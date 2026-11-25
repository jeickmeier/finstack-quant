//! Analytical (Kemna-Vorst / seasoned geometric) and semi-analytical
//! (Turnbull-Wakeman) Asian option pricers.

use super::inputs::{
    analytic_inputs, asian_drift_and_fixing_multipliers, expired_valuation,
    expiry_year_fraction_or_zero, fixing_year_fractions,
};
use super::moments::{price_arithmetic_tw_asian, price_geometric_asian};
use crate::exotics::asian_option::types::{AsianOption, average_intrinsic};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::OptionMarketParams;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

fn future_equity_forwards(
    asian: &AsianOption,
    market: &MarketContext,
    as_of: Date,
    spot: f64,
    q: f64,
) -> finstack_quant_core::Result<Vec<(f64, f64)>> {
    let disc_curve = market.get_discount(asian.discount_curve_id.as_str())?;
    let mut forwards = Vec::with_capacity(asian.fixing_dates.len());
    for &date in &asian.fixing_dates {
        if date > as_of {
            let t_i = asian
                .day_count
                .year_fraction(as_of, date, DayCountContext::default())?;
            let df_i = disc_curve.df_between_dates(as_of, date)?;
            forwards.push((t_i, spot * (-q * t_i).exp() / df_i));
        }
    }
    Ok(forwards)
}

pub(super) fn seasoned_geometric_asian_control(
    market: &OptionMarketParams,
    df: f64,
    hist_prod_log: f64,
    hist_count: usize,
    future_times: &[f64],
    drift_schedule: Option<&finstack_quant_models::monte_carlo::process::gbm::DriftSchedule>,
    fixing_multipliers: Option<&[f64]>,
) -> finstack_quant_core::Result<f64> {
    let &OptionMarketParams {
        spot,
        strike,
        rate: r,
        volatility: sigma,
        dividend_yield: q,
        option_type,
        ..
    } = market;
    let total_fixings = hist_count + future_times.len();
    if total_fixings == 0 {
        return Ok(0.0);
    }
    let future_forwards: Vec<(f64, f64)> = future_times
        .iter()
        .enumerate()
        .map(|(index, &t_i)| {
            let drift_to_t = match drift_schedule {
                Some(schedule) => schedule.cumulative(t_i),
                None => (r - q) * t_i,
            };
            let mult = fixing_multipliers.map_or(1.0, |scales| scales[index]);
            (t_i, spot * drift_to_t.exp() * mult)
        })
        .collect();
    price_geometric_asian(
        &future_forwards,
        strike,
        sigma,
        df,
        option_type,
        hist_prod_log,
        total_fixings,
    )
}

/// Geometric Asian option analytical pricer.
pub struct AsianOptionAnalyticalGeometricPricer;

impl Pricer for AsianOptionAnalyticalGeometricPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::AsianOption, ModelKey::AsianGeometricBS)
    }

    #[tracing::instrument(
        name = "asian_option.geometric_bs.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let asian = expect_inst::<AsianOption>(instrument, InstrumentType::AsianOption)?;
        asian.validate_past_fixings(as_of)?;

        let (sum, log_prod, count) = asian.accumulated_state(as_of);
        if expiry_year_fraction_or_zero(asian, as_of)? <= 0.0 {
            return expired_valuation(
                asian,
                market,
                as_of,
                asian.averaging_method,
                sum,
                log_prod,
                count,
            );
        }

        let inputs = analytic_inputs(asian, market, as_of)?;
        let r = inputs.r_eff().map_err(|error| {
            PricingError::from_core(error, PricingErrorContext::from_instrument(asian))
        })?;
        let (spot, q, sigma, t) = (inputs.spot, inputs.q, inputs.sigma, inputs.t);

        // Price on the actual fixing schedule. Falling back to the
        // equal-spacing formula would misprice contracts whose fixings are
        // not uniformly spaced over [as_of, expiry] (e.g. averaging windows
        // concentrated near expiry).
        let fixing_times = fixing_year_fractions(asian, as_of)?;
        let df = inputs.df;
        let (drift, multipliers) = asian_drift_and_fixing_multipliers(asian, market, as_of, q)
            .map_err(|error| {
                PricingError::from_core(error, PricingErrorContext::from_instrument(asian))
            })?;
        let price = seasoned_geometric_asian_control(
            &OptionMarketParams {
                spot,
                strike: asian.strike,
                rate: r,
                volatility: sigma,
                time_to_expiry: t,
                dividend_yield: q,
                option_type: asian.option_type,
            },
            df,
            log_prod,
            count,
            &fixing_times,
            Some(&drift),
            Some(&multipliers),
        )
        .map_err(|error| {
            PricingError::from_core(error, PricingErrorContext::from_instrument(asian))
        })?;

        let pv = Money::new(price * asian.quantity, asian.currency).map_err(|error| {
            finstack_quant_valuations_core::pricer::PricingError::from_core(
                error,
                finstack_quant_valuations_core::pricer::PricingErrorContext::from_instrument(asian),
            )
        })?;
        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}

/// Arithmetic Asian option semi-analytical pricer (Turnbull-Wakeman).
pub struct AsianOptionSemiAnalyticalTwPricer;

impl Pricer for AsianOptionSemiAnalyticalTwPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::AsianOption, ModelKey::AsianTurnbullWakeman)
    }

    #[tracing::instrument(
        name = "asian_option.turnbull_wakeman.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let asian = expect_inst::<AsianOption>(instrument, InstrumentType::AsianOption)?;
        asian.validate_past_fixings(as_of)?;

        let (sum, _, count) = asian.accumulated_state(as_of);
        let total_fixings = asian.fixing_dates.len();

        if expiry_year_fraction_or_zero(asian, as_of)? <= 0.0 {
            // Turnbull-Wakeman prices the arithmetic average.
            return expired_valuation(
                asian,
                market,
                as_of,
                AveragingMethod::Arithmetic,
                sum,
                0.0,
                count,
            );
        }

        // Date-based DF for time-consistent discounting
        let bs_inputs = analytic_inputs(asian, market, as_of)?;

        let df_expiry = bs_inputs.df;
        let q = bs_inputs.q;
        let sigma = bs_inputs.sigma;

        let future_fixings = total_fixings.saturating_sub(count);
        if future_fixings == 0 {
            // Deterministic case (all fixings past, but not expired?)
            let payoff =
                average_intrinsic(asian.option_type, sum / total_fixings as f64, asian.strike);
            // Use the date-based DF from inputs
            return Ok(ValuationResult::stamped(
                asian.id(),
                as_of,
                Money::new(payoff * df_expiry * asian.quantity, asian.currency).map_err(
                    |error| {
                        finstack_quant_valuations_core::pricer::PricingError::from_core(
                            error,
                            finstack_quant_valuations_core::pricer::PricingErrorContext::from_instrument(asian),
                        )
                    },
                )?,
            ));
        }

        let future_forwards = future_equity_forwards(asian, market, as_of, bs_inputs.spot, q)?;
        let price = price_arithmetic_tw_asian(
            &future_forwards,
            asian.strike,
            sigma,
            df_expiry,
            asian.option_type,
            sum,
            total_fixings,
        )
        .map_err(|error| {
            PricingError::from_core(error, PricingErrorContext::from_instrument(asian))
        })?;

        let pv = Money::new(price * asian.quantity, asian.currency).map_err(|error| {
            finstack_quant_valuations_core::pricer::PricingError::from_core(
                error,
                finstack_quant_valuations_core::pricer::PricingErrorContext::from_instrument(asian),
            )
        })?;
        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}
