//! Monte Carlo GBM pricer for Asian options with geometric control-variate
//! variance reduction.

use super::analytic::seasoned_geometric_asian_control;
use super::common::{
    asian_drift_and_fixing_multipliers, expired_value, map_fixings_to_steps, resolve_spot,
};
use crate::exotics::asian_option::types::AsianOption;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::control_variate::apply_control_variate;
use finstack_quant_models::monte_carlo::payoff::asian::{Asian, AsianFixingHistory};
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess};
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::types::OptionMarketParams;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

/// Asian option Monte Carlo pricer.
pub struct AsianOptionMcPricer {
    pub(super) config: PathDependentPricerConfig,
}

impl AsianOptionMcPricer {
    /// Create a new Asian option MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    /// Price an Asian option using Monte Carlo.
    pub(crate) fn price_internal(
        &self,
        inst: &AsianOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        inst.validate_past_fixings(as_of)?;
        // Day counts reject reversed dates; expired contracts use their
        // realized payoff without reading live market inputs.
        let t = if as_of >= inst.expiry {
            0.0
        } else {
            inst.day_count
                .year_fraction(as_of, inst.expiry, DayCountContext::default())?
        };

        let (hist_sum, hist_prod_log, hist_count) = inst.accumulated_state(as_of);

        if t <= 0.0 {
            return expired_value(inst, curves, hist_sum, hist_prod_log, hist_count);
        }

        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        // Keep drift consistent with date-based discounting: exp(-r * t) == DF(as_of, maturity).
        // A non-positive/non-finite df means a corrupted curve; error rather
        // than silently pricing at a zero rate.
        let r =
            finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
                discount_factor,
                t,
                "AsianOption discount curve",
            )?;

        let spot = resolve_spot(inst, curves)?;

        let q = finstack_quant_valuations_core::instruments::pricing::market_inputs::resolve_optional_dividend_yield(
            curves,
            inst.div_yield_id.as_ref(),
        )?;

        // Get volatility (override → surface)
        let sigma =
            finstack_quant_valuations_core::instruments::pricing::volatility::resolve_sigma_at(
                &inst.instrument_pricing_overrides.market_quotes,
                curves,
                inst.vol_surface_id.as_str(),
                t,
                inst.strike,
            )?;

        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let base_cfg =
            finstack_quant_valuations_core::instruments::pricing::monte_carlo::merged_path_config(
                &self.config,
                &inst.instrument_pricing_overrides,
            )?;

        // Exact event times prevent time-grid resolution from changing the
        // average. Equal model times share a simulated state but retain weight.
        let base_num_steps =
            ((t * base_cfg.steps_per_year).round() as usize).max(base_cfg.min_steps);
        let fixing_grid =
            map_fixings_to_steps(&inst.fixing_dates, inst.day_count, as_of, t, base_num_steps)?;
        let (drift, fixing_multipliers) =
            asian_drift_and_fixing_multipliers(inst, curves, as_of, q)?;
        let drift_schedule = std::sync::Arc::new(drift);
        let process = process.with_drift_schedule(std::sync::Arc::clone(&drift_schedule));
        let future_fixing_times = fixing_grid.future_times();
        let time_grid = fixing_grid.time_grid;
        let fixing_steps = fixing_grid.fixing_steps;

        let averaging = inst.averaging_method;

        // Derive deterministic seed from instrument ID and scenario

        let seed = inst
            .instrument_pricing_overrides
            .model_config
            .mc_seed(&inst.id);

        let mut config = base_cfg;
        config.seed = seed;

        // If arithmetic averaging, apply geometric-Asian control variate for variance reduction
        let result_money = if matches!(
            inst.averaging_method,
            finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod::Arithmetic
        ) {
            let pricer = PathDependentPricer::new(config);
            let arith_payoff = Asian::with_history(
                inst.option_type,
                inst.strike,
                inst.quantity,
                finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod::Arithmetic,
                fixing_steps,
                AsianFixingHistory {
                    sum: hist_sum,
                    log_sum: hist_prod_log,
                    count: hist_count,
                },
            )?
            .with_fixing_multipliers(&fixing_multipliers)?;
            // Both existing payoffs observe one path; the collector retains
            // discounted paired moments instead of full simulated paths.
            let moments = pricer.price_asian_pair(
                &process,
                spot,
                time_grid,
                &arith_payoff,
                inst.currency,
                discount_factor,
            )?;

            let n = moments.get_count();
            let (mean_x, mean_y) = (
                moments.get_mean_x()?.unwrap_or(0.0),
                moments.get_mean_y()?.unwrap_or(0.0),
            );
            let (var_x, var_y) = (
                moments.get_variance_x()?.unwrap_or(0.0),
                moments.get_variance_y()?.unwrap_or(0.0),
            );
            let cov_xy = moments.get_covariance()?.unwrap_or(0.0);

            // Analytical value of geometric Asian (control).
            //
            // The standard geometric-Asian closed form has no seasoning
            // adjustment, so it is only a valid control when the option is
            // unseasoned. For a seasoned arithmetic Asian the seasoning-
            // aware analytic control variate is used instead (see below).

            // Seasoning-aware analytic control variate (W-07). The
            // geometric Asian is priced on the *exact* future fixing times
            // the MC samples, with the past fixings' fixed log-product
            // folded in, so it is the true mean of the simulated geometric
            // payoff for both seasoned and unseasoned options. The seasoned
            // path therefore keeps the variance reduction instead of
            // discarding the geometric control.
            // The paired MC payoffs are notional-scaled while the
            // closed form is per unit notional, so the control mean must
            // be scaled to the same units before the CV adjustment.
            let control_analytical = inst.quantity
                * seasoned_geometric_asian_control(
                    &OptionMarketParams {
                        spot,
                        strike: inst.strike,
                        rate: r,
                        volatility: sigma,
                        time_to_expiry: t,
                        dividend_yield: q,
                        option_type: inst.option_type,
                    },
                    discount_factor,
                    hist_prod_log,
                    hist_count,
                    &future_fixing_times,
                    Some(drift_schedule.as_ref()),
                    Some(&fixing_multipliers),
                )?;
            let adj =
                apply_control_variate(mean_x, var_x, mean_y, var_y, cov_xy, control_analytical, n);
            MoneyEstimate::from_estimate(adj, inst.currency)?.mean
        } else {
            let pricer = PathDependentPricer::new(config);
            let payoff = Asian::with_history(
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
            )?
            .with_fixing_multipliers(&fixing_multipliers)?;
            pricer
                .price(
                    &process,
                    spot,
                    time_grid,
                    &payoff,
                    inst.currency,
                    discount_factor,
                )?
                .mean
        };

        Ok(result_money)
    }
}

impl Default for AsianOptionMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for AsianOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::AsianOption, ModelKey::MonteCarloGBM)
    }

    #[tracing::instrument(
        name = "asian_option.mc.price_dyn",
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

        let pv = self.price_internal(asian, market, as_of)?;

        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}
