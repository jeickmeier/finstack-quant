//! Autocallable Monte Carlo pricer.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::autocallable::monte_carlo::{
    AutocallablePayoff, FinalPayoffType as McFinalPayoffType,
};
use crate::instruments::equity::autocallable::types::{Autocallable, FinalPayoffType};
use crate::instruments::equity::piecewise_gbm::{bootstrap_forward_gbm, PiecewiseExactGbm};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;
use finstack_monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::time_grid::TimeGrid;

/// Autocallable Monte Carlo pricer.
pub struct AutocallableMcPricer {
    config: PathDependentPricerConfig,
}

impl AutocallableMcPricer {
    /// Create a new autocallable MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    fn convert_final_payoff_type(ft: FinalPayoffType) -> McFinalPayoffType {
        match ft {
            FinalPayoffType::CapitalProtection { floor } => {
                McFinalPayoffType::CapitalProtection { floor }
            }
            FinalPayoffType::Participation { rate } => McFinalPayoffType::Participation { rate },
            FinalPayoffType::KnockInPut { strike } => McFinalPayoffType::KnockInPut { strike },
        }
    }

    /// Price an autocallable using Monte Carlo.
    fn price_internal(
        &self,
        inst: &Autocallable,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<finstack_core::money::Money> {
        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let initial_spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;

        // Use explicit expiry as the contractual settlement/maturity date.
        let final_date = inst.expiry;
        let disc_dc = disc_curve.day_count();
        let t = disc_dc.year_fraction(as_of, final_date, DayCountContext::default())?;
        if t <= 0.0 {
            return Ok(Money::new(0.0, inst.notional.currency()));
        }

        let discount_factor = disc_curve.df_between_dates(as_of, final_date)?;

        // Dividend yield from scalar id if provided
        //
        // When a dividend yield ID is explicitly provided, we require the lookup to succeed
        // and return a unitless scalar. Silent fallback to 0.0 would mask market data
        // configuration errors.
        let q = if let Some(div_id) = &inst.div_yield_id {
            let ms = curves.get_price(div_id.as_str()).map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "Failed to fetch dividend yield '{}': {}",
                    div_id, e
                ))
            })?;
            match ms {
                finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_core::market_data::scalars::MarketScalar::Price(m) => {
                    return Err(finstack_core::Error::Validation(format!(
                        "Dividend yield '{}' should be a unitless scalar, got Price({})",
                        div_id,
                        m.currency()
                    )));
                }
            }
        } else {
            0.0
        };

        // Map observation dates to times.
        let observation_times: Vec<f64> = inst
            .observation_dates
            .iter()
            .map(|&date| disc_dc.year_fraction(as_of, date, DayCountContext::default()))
            .collect::<finstack_core::Result<Vec<_>>>()?;

        // Bootstrap a piecewise-constant forward GBM over the autocall observation
        // schedule (plus the final maturity) so the simulation carries the term
        // structure of volatility and rates. A single flat-vol GBM misprices the
        // knock-in put and the path-dependent autocall probabilities when the
        // surface/curve is not flat; for a flat surface this reduces exactly to the
        // previous constant-GBM process.
        //
        // NOTE: vol-surface expiries are assumed to share the discount curve's day
        // count (both typically ACT/365F for equity vol).
        let mut check_points = observation_times.clone();
        check_points.retain(|&ct| ct > 0.0);
        check_points.push(t);
        check_points.sort_by(|a, b| a.total_cmp(b));
        check_points.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        let process = bootstrap_forward_gbm(
            disc_curve.as_ref(),
            curves,
            &inst.pricing_overrides.market_quotes,
            inst.vol_surface_id.as_str(),
            as_of,
            initial_spot,
            q,
            &check_points,
            &format!("Autocallable {}", inst.id),
        )?;

        let mc_final_payoff = Self::convert_final_payoff_type(inst.final_payoff_type);

        // Calculate discount factor ratios for each observation date
        // Ratio = DF(T_obs) / DF(T_mat)
        // This corrects for the engine applying DF(T_mat) to early cashflows
        //
        // IMPORTANT: Use discount curve's day count (disc_dc) consistently for all
        // time calculations. Mixing inst.day_count with disc_dc would distort timing
        // and coupon PVs. The observation_times above already use disc_dc, so the
        // discount factor lookups must match to ensure consistent discounting.
        let df_ratios: Vec<f64> = observation_times
            .iter()
            .map(|&t_obs| {
                let df_obs = disc_curve.df(t_obs.max(0.0));
                if discount_factor > 0.0 {
                    df_obs / discount_factor
                } else {
                    1.0
                }
            })
            .collect();

        let payoff = AutocallablePayoff::new(
            observation_times.clone(),
            inst.autocall_barriers.clone(),
            inst.coupons.clone(),
            inst.memory_coupons,
            inst.final_barrier,
            mc_final_payoff,
            inst.participation_rate,
            inst.cap_level,
            inst.notional.amount(),
            inst.notional.currency(),
            initial_spot,
            df_ratios,
        )?;

        // Derive deterministic seed from instrument ID and scenario.
        use finstack_monte_carlo::seed;
        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        let merged_cfg = crate::instruments::common_impl::helpers::merged_path_config(
            &self.config,
            &inst.pricing_overrides,
        )?;

        // Identical grid to the previous constant-GBM path (uniform steps plus the
        // observation dates as required times), so a flat surface reproduces the
        // prior prices bit-for-bit.
        let time_grid = TimeGrid::uniform_with_required_times(
            t,
            merged_cfg.steps_per_year,
            merged_cfg.min_steps,
            &observation_times,
        )?;

        // Mirror `PathDependentPricer::price_with_grid` (non-Sobol path) but drive
        // the piecewise process with its exact discretization.
        let engine_config = McEngineConfig {
            num_paths: merged_cfg.num_paths,
            time_grid,
            target_ci_half_width: None,
            use_parallel: merged_cfg.use_parallel,
            chunk_size: Some(merged_cfg.chunk_size),
            path_capture: merged_cfg.path_capture.clone(),
            antithetic: merged_cfg.antithetic,
        };
        let engine = McEngine::new(engine_config);
        let rng = PhiloxRng::new(seed);
        let disc = PiecewiseExactGbm::new();
        let initial_state = vec![initial_spot];

        let result = engine.price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            inst.notional.currency(),
            discount_factor,
        )?;

        Ok(result.mean)
    }
}

impl Default for AutocallableMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for AutocallableMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Autocallable, ModelKey::MonteCarloGBM)
    }

    #[tracing::instrument(
        name = "autocallable.mc.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let autocallable = instrument
            .as_any()
            .downcast_ref::<Autocallable>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::Autocallable, instrument.key())
            })?;

        let pv = self
            .price_internal(autocallable, market, as_of)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(autocallable)
                        .model(ModelKey::MonteCarloGBM),
                )
            })?;

        Ok(ValuationResult::stamped(autocallable.id(), as_of, pv))
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &Autocallable,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let pricer = AutocallableMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}
