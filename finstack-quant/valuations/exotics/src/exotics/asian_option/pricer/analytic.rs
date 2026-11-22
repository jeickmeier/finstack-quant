//! Analytical (Kemna-Vorst / seasoned geometric) and semi-analytical
//! (Turnbull-Wakeman) Asian option pricers.

use super::common::{
    analytic_inputs, asian_drift_and_fixing_multipliers, expired_valuation,
    expiry_year_fraction_or_zero, fixing_year_fractions,
};
use crate::exotics::asian_option::types::{AsianOption, average_intrinsic};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_models::closed_form::{AsianFixingSchedule, arithmetic_asian_tw_price_times};
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::{OptionMarketParams, OptionType};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

/// Closed-form value of a **seasoned** geometric-average Asian option, used as
/// the analytic control variate for the seasoned arithmetic Asian MC (W-07).
///
/// The Monte Carlo geometric payoff computes `G = exp((Σ ln S_i) / N)` where
/// the sum runs over all `N` fixings — `m` already observed (contributing the
/// fixed quantity `hist_prod_log = Σ ln S_past`) and `k` future fixings at
/// times `future_times` (under the simulated GBM). `ln G` is therefore normal,
/// so `G` is lognormal and the option has a Black-style closed form.
///
/// With `ln S_{t_i} = ln S_0 + (r - q - σ²/2) t_i + σ W_{t_i}`:
/// * `μ = [hist_prod_log + Σ_i (ln S_0 + (r-q-σ²/2) t_i)] / N`
/// * `v = σ² · ΣᵢΣⱼ min(t_i, t_j) / N²`   (variance of `ln G`)
/// * `E[G] = exp(μ + v/2)`
///
/// The fallback drift uses `r` and discounting uses `df` so the value is consistent
/// with how the MC simulates and discounts (the unseasoned case reduces to the
/// standard Kemna-Vorst control). When the MC attaches a time-varying drift
/// schedule (curve-implied forwards on a non-flat curve), the same schedule
/// MUST be supplied here: the control variate is only unbiased when the
/// analytic control equals the true mean of the simulated geometric payoff,
/// so `E[ln S_{tᵢ}]` has to use the schedule's cumulative drift, not the
/// maturity-averaged constant `r − q`. Returns `df · max(forward_intrinsic,
/// 0)` in the degenerate zero-variance case.
///
/// `market` carries spot, strike, `rate` (`r`), `dividend_yield` (`q`),
/// `volatility` (`σ`) and the call/put direction; `market.time_to_expiry` is
/// not read, because discounting uses `df` and the fixings use `future_times`.
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
    let is_call = matches!(option_type, OptionType::Call);
    let k = future_times.len();
    let n_total = hist_count + k;
    if n_total == 0 {
        return Ok(0.0);
    }
    let n = n_total as f64;
    let ln_spot = spot.ln();
    let constant_drift = r - q - 0.5 * sigma * sigma;

    // Mean of ln(G): fixed past contribution + future drift contribution.
    // With a drift schedule, E[ln S_{tᵢ}] = ln S + M(tᵢ) − σ²tᵢ/2 where
    // M(t) is the schedule's cumulative log-drift — identical to what the MC
    // engine applies step by step.
    let mut mean_acc = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    mean_acc.add(hist_prod_log)?;
    for (index, &t_i) in future_times.iter().enumerate() {
        let drift_to_t = match drift_schedule {
            Some(schedule) => schedule.cumulative(t_i) - 0.5 * sigma * sigma * t_i,
            None => constant_drift * t_i,
        };
        let log_multiplier = fixing_multipliers.map_or(0.0, |scales| scales[index].ln());
        mean_acc.add(ln_spot + drift_to_t + log_multiplier)?;
    }
    let mu = mean_acc.get_total()? / n;

    // Variance of ln(G): σ² ΣᵢΣⱼ min(t_i,t_j) / N².
    let mut cov_acc = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    for &t_i in future_times {
        for &t_j in future_times {
            cov_acc.add(t_i.min(t_j))?;
        }
    }
    let var_ln_g = sigma * sigma * cov_acc.get_total()? / (n * n);

    // Degenerate (no future fixings or zero vol): G is deterministic.
    if var_ln_g <= 0.0 || k == 0 {
        let g = mu.exp();
        let intrinsic = if is_call {
            (g - strike).max(0.0)
        } else {
            (strike - g).max(0.0)
        };
        return Ok(df * intrinsic);
    }

    let std = var_ln_g.sqrt();
    let expected_g = (mu + 0.5 * var_ln_g).exp();
    let d1 = (mu + var_ln_g - strike.ln()) / std;
    let d2 = d1 - std;
    let norm_cdf = finstack_quant_core::math::special_functions::norm_cdf;
    let price = if is_call {
        expected_g * norm_cdf(d1)? - strike * norm_cdf(d2)?
    } else {
        strike * norm_cdf(-d2)? - expected_g * norm_cdf(-d1)?
    };
    Ok(df * price.max(0.0))
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

        let spot = bs_inputs.spot;
        let df_expiry = bs_inputs.df;
        let q = bs_inputs.q;
        let sigma = bs_inputs.sigma;
        let t = bs_inputs.t;

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

        let n = total_fixings as f64;
        let m = future_fixings as f64;
        let k = asian.strike;

        let numerator = n * k - sum;
        let k_eff = numerator / m;
        let scale = m / n;

        let price = if k_eff <= 0.0 {
            // Deep ITM (k_eff <= 0): past fixings alone settle the moneyness, so
            // PV = DF_T · max(±(E[A] − K), 0) with E[A] = (Σ past + Σ F_i) / N
            // and each GK forward F_i = S · exp(−q·t_i) / DF(t_i) on date-based DFs.
            let disc_curve = market.get_discount(asian.discount_curve_id.as_str())?;
            let mut sum_fwd = 0.0;
            for date in &asian.fixing_dates {
                if *date > as_of {
                    let t_i =
                        asian
                            .day_count
                            .year_fraction(as_of, *date, DayCountContext::default())?;
                    let df_i = disc_curve.df_between_dates(as_of, *date)?;
                    let forward_i = spot * (-q * t_i).exp() / df_i;
                    sum_fwd += forward_i;
                }
            }
            let expected_avg = (sum + sum_fwd) / n;
            average_intrinsic(asian.option_type, expected_avg, k) * df_expiry
        } else {
            // Price the remaining-fixings option on the ACTUAL future fixing
            // times: seasoned schedules are concentrated near expiry, so the
            // equal-spacing-over-[0, t] assumption misstates the average's
            // variance.
            let future_times = fixing_year_fractions(asian, as_of)?;
            let unscaled = arithmetic_asian_tw_price_times(
                spot,
                k_eff,
                q,
                sigma,
                &AsianFixingSchedule {
                    t_expiry: t,
                    df: df_expiry,
                    fixing_times: &future_times,
                },
                asian.option_type,
            )?;
            unscaled * scale
        };

        let pv = Money::new(price * asian.quantity, asian.currency).map_err(|error| {
            finstack_quant_valuations_core::pricer::PricingError::from_core(
                error,
                finstack_quant_valuations_core::pricer::PricingErrorContext::from_instrument(asian),
            )
        })?;
        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}
