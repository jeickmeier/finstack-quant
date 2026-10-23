//! Barrier option Heston Monte Carlo pricer.
//!
//! Prices barrier options under the Heston stochastic volatility model
//! using Monte Carlo simulation with QE discretization and Brownian bridge
//! barrier correction.

use crate::exotics::barrier_option::types::BarrierOption;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError,
};
use finstack_quant_valuations_core::results::ValuationResult;

use finstack_quant_models::monte_carlo::discretization::qe_heston::QeHeston;
use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_quant_models::monte_carlo::process::heston::HestonProcess;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;

/// Barrier option Heston Monte Carlo pricer.
///
/// Prices barrier options under the Heston stochastic volatility model using
/// QE discretization. The barrier is monitored on the spot component (`state[0]`)
/// of the Heston path. Heston parameters are required market scalars
/// (`HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`, `HESTON_RHO`, `HESTON_V0`).
pub(crate) struct BarrierOptionHestonMcPricer {
    num_paths: usize,
    steps_per_year: f64,
}

impl BarrierOptionHestonMcPricer {
    /// Create a new barrier option Heston MC pricer with default configuration.
    pub(crate) fn new() -> Self {
        Self {
            num_paths: 100_000,
            steps_per_year: 252.0,
        }
    }

    /// Price a barrier option using Heston Monte Carlo.
    fn price_internal(
        &self,
        inst: &BarrierOption,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(Money, f64)> {
        if let Some(value) = super::pricer::settled_barrier_value(inst, market, as_of)? {
            return Ok((value, 0.0));
        }
        // Time to maturity
        let t = inst
            .day_count
            .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

        let disc_curve = market.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        let r =
            finstack_quant_valuations_core::instruments::common_impl::helpers::zero_rate_from_df(
                discount_factor,
                t,
                "BarrierOption Heston MC discount curve",
            )?;
        let spot = super::pricer::resolve_spot(inst, market)?;
        let q = finstack_quant_valuations_core::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            market,
            inst.div_yield_id.as_ref(),
        )?;

        let heston_params =
            finstack_quant_valuations_core::instruments::pricing::heston_market::heston_params_from_market_strict(
                market, r, q,
            )?;
        let v0 = heston_params.model.v0;
        let sigma = v0.sqrt();
        let process = HestonProcess::new(heston_params).with_drift_schedule(std::sync::Arc::new(
            super::pricer::barrier_drift(inst, market, as_of, q)?,
        ));
        let discretization = QeHeston::new();

        let grid_config = PathDependentPricerConfig {
            steps_per_year: self.steps_per_year,
            min_steps: 10,
            ..PathDependentPricerConfig::default()
        };
        let (time_grid, monitoring) =
            inst.monitoring
                .time_grid(as_of, inst.day_count, None, t, &grid_config)?;
        // The Heston path variance supplies the local bridge volatility.
        let payoff = super::pricer::barrier_payoff(inst, sigma, &time_grid, monitoring, r)
            .with_observed_barrier_breached(inst.observed_barrier_breached.unwrap_or(false));

        let num_paths =
            finstack_quant_valuations_core::instruments::common_impl::helpers::resolve_mc_paths(
                inst.instrument_pricing_overrides.model_config.mc_paths,
                self.num_paths,
            )?;

        // Derive deterministic seed
        let seed_val = inst
            .instrument_pricing_overrides
            .model_config
            .mc_seed(&inst.id);

        let engine = McEngine::new(McEngineConfig::new(num_paths, time_grid));

        let rng = PhiloxRng::new(seed_val);

        // Initial state: [spot, v0]
        let initial_state = [spot, v0];

        let result = engine.price(
            &rng,
            &process,
            &discretization,
            &initial_state,
            &payoff,
            inst.currency,
            discount_factor,
        )?;

        Ok((result.mean, result.stderr))
    }
}

impl Default for BarrierOptionHestonMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for BarrierOptionHestonMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BarrierOption, ModelKey::MonteCarloHeston)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let barrier = expect_inst::<BarrierOption>(instrument, InstrumentType::BarrierOption)?;

        let (pv, stderr) = self.price_internal(barrier, market, as_of)?;

        let mut result = ValuationResult::stamped(barrier.id(), as_of, pv);
        if stderr > 0.0 {
            result.measures.insert(
                finstack_quant_valuations_core::metrics::MetricId::custom("mc_stderr"),
                stderr,
            );
        }
        Ok(result)
    }
}
