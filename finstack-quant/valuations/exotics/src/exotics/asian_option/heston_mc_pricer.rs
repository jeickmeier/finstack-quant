//! Asian option Heston Monte Carlo pricer.
//!
//! Prices Asian options under the Heston stochastic volatility model
//! using Monte Carlo simulation with QE discretization. Averaging is
//! performed on the spot component (`state[0]`) of the Heston path.

use crate::exotics::asian_option::types::AsianOption;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::payoff::asian::AsianFixingHistory;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::pricing::heston_market::{
    DEFAULT_HESTON_MC_PATHS, DEFAULT_HESTON_STEPS_PER_YEAR, HestonMcRequest, price_heston_mc,
};
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

use finstack_quant_models::monte_carlo::payoff::asian::Asian;

/// Asian option Heston Monte Carlo pricer.
///
/// Prices Asian options under the Heston stochastic volatility model using
/// QE discretization. Averaging is performed on the spot component of the
/// Heston two-factor path. Heston parameters are required market scalars
/// (`HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`, `HESTON_RHO`, `HESTON_V0`).
pub(crate) struct AsianOptionHestonMcPricer {
    num_paths: usize,
    steps_per_year: f64,
    min_steps: usize,
}

impl AsianOptionHestonMcPricer {
    /// Create a new Asian option Heston MC pricer with default configuration.
    pub(crate) fn new() -> Self {
        Self {
            num_paths: DEFAULT_HESTON_MC_PATHS,
            steps_per_year: DEFAULT_HESTON_STEPS_PER_YEAR,
            min_steps: 10,
        }
    }

    /// Price an Asian option using Heston Monte Carlo.
    fn price_internal(
        &self,
        inst: &AsianOption,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(Money, f64)> {
        inst.validate_past_fixings(as_of)?;
        let (hist_sum, hist_prod_log, hist_count) = inst.accumulated_state(as_of);

        if as_of >= inst.expiry {
            let pv =
                super::pricer::expired_value(inst, market, hist_sum, hist_prod_log, hist_count)?;
            return Ok((pv, 0.0));
        }

        // Time to maturity
        let t = inst
            .day_count
            .year_fraction(as_of, inst.expiry, DayCountContext::default())?;
        if t <= 0.0 {
            let pv =
                super::pricer::expired_value(inst, market, hist_sum, hist_prod_log, hist_count)?;
            return Ok((pv, 0.0));
        }

        let disc_curve = market.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        if inst.fixing_dates.iter().all(|&d| d <= as_of) {
            let pv =
                super::pricer::expired_value(inst, market, hist_sum, hist_prod_log, hist_count)?;
            return Ok((
                Money::from_model_value(pv.amount() * discount_factor, inst.currency)?,
                0.0,
            ));
        }
        let r =
            finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
                discount_factor,
                t,
                "AsianOption Heston MC discount curve",
            )?;

        let spot = crate::exotics::bs_gbm::resolve_positive_spot(
            market,
            &inst.spot_id,
            inst.currency,
            "AsianOption",
        )?;

        let q = finstack_quant_valuations_core::instruments::pricing::market_inputs::resolve_optional_dividend_yield(
            market,
            inst.div_yield_id.as_ref(),
        )?;

        // Insert each fixing time exactly and retain repeated observations at
        // shared diffusion times.
        let base_num_steps = ((t * self.steps_per_year).round() as usize).max(self.min_steps);
        let fixing_grid = crate::exotics::asian_option::pricer::map_fixings_to_steps(
            &inst.fixing_dates,
            inst.day_count,
            as_of,
            t,
            base_num_steps,
        )?;
        let time_grid = fixing_grid.time_grid;
        let fixing_steps = fixing_grid.fixing_steps;

        // The Heston engine has constant carry r-q. Deterministic observation
        // scaling gives every fixing its dated forward without changing the
        // common stochastic variance path or the declared diffusion clock.
        let fixing_multipliers = inst
            .fixing_dates
            .iter()
            .filter(|&&date| date > as_of)
            .map(|&date| {
                let time = inst
                    .day_count
                    .year_fraction(as_of, date, DayCountContext::default())?;
                Ok((-disc_curve.df_between_dates(as_of, date)?.ln() - r * time).exp())
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        let averaging = inst.averaging_method;

        price_heston_mc(
            market,
            HestonMcRequest {
                instrument_id: &inst.id,
                model_config: &inst.instrument_pricing_overrides.model_config,
                default_num_paths: self.num_paths,
                spot,
                r,
                q,
                discount_factor,
                currency: inst.currency,
                time_grid,
                drift_schedule: None,
            },
            |_grid, _heston| {
                if hist_count > 0 {
                    Asian::with_history(
                        inst.option_type,
                        inst.strike,
                        inst.quantity,
                        averaging,
                        fixing_steps,
                        AsianFixingHistory {
                            sum: hist_sum,
                            log_sum: hist_prod_log,
                            count: hist_count,
                        },
                    )
                } else {
                    Asian::new(
                        inst.option_type,
                        inst.strike,
                        inst.quantity,
                        averaging,
                        fixing_steps,
                    )
                }?
                .with_fixing_multipliers(&fixing_multipliers)
            },
        )
    }
}

impl Default for AsianOptionHestonMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for AsianOptionHestonMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::AsianOption, ModelKey::MonteCarloHeston)
    }

    #[tracing::instrument(
        name = "asian_option.heston_mc.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(
            inst_id = %instrument.id(),
            as_of = %as_of,
            num_paths = self.num_paths,
        ),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let asian = expect_inst::<AsianOption>(instrument, InstrumentType::AsianOption)?;

        let (pv, stderr) = self.price_internal(asian, market, as_of)?;

        let mut result = ValuationResult::stamped(asian.id(), as_of, pv);
        if stderr > 0.0 {
            result.measures.insert(
                finstack_quant_valuations_core::metrics::MetricId::custom("mc_stderr"),
                stderr,
            );
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_models::types::OptionType;
    use time::Month;

    #[test]
    fn post_expiry_heston_mc_settles_realized_average() {
        let mut asian = AsianOption::example().expect("example");
        asian.option_type = OptionType::Call;
        asian.strike = 100.0;
        asian.quantity = 10.0;
        asian.fixing_dates = vec![
            Date::from_calendar_date(2024, Month::June, 3).expect("date"),
            Date::from_calendar_date(2024, Month::December, 2).expect("date"),
        ];
        asian.expiry = Date::from_calendar_date(2024, Month::December, 2).expect("date");
        asian.past_fixings = vec![
            (asian.fixing_dates[0], 110.0),
            (asian.fixing_dates[1], 130.0),
        ];
        let as_of = Date::from_calendar_date(2025, Month::January, 10).expect("date");
        let market = MarketContext::new().insert_price(
            "SPX-SPOT",
            MarketScalar::price(Money::new(90.0, Currency::USD).expect("money")),
        );
        let (pv, stderr) = AsianOptionHestonMcPricer::new()
            .price_internal(&asian, &market, as_of)
            .expect("post-expiry settlement");
        assert_eq!(stderr, 0.0);
        // Average = (110 + 130)/2 = 120; payoff = (120 - 100) * 10 = 200.
        assert!((pv.amount() - 200.0).abs() < 1e-10, "pv={}", pv.amount());
    }
}
