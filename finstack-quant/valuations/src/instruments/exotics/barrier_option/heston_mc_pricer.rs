//! Barrier option Heston Monte Carlo pricer.
//!
//! Prices barrier options under the Heston stochastic volatility model
//! using Monte Carlo simulation with QE discretization and Brownian bridge
//! barrier correction.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::barrier_option::types::BarrierOption;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

use finstack_quant_models::monte_carlo::discretization::qe_heston::QeHeston;
use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_quant_models::monte_carlo::payoff::barrier::{
    BarrierMonitoring, BarrierOptionPayoff, OptionKind,
};
use finstack_quant_models::monte_carlo::process::heston::HestonProcess;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::seed;
use finstack_quant_models::monte_carlo::TimeGrid;

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

    fn convert_option_kind(option_type: crate::instruments::OptionType) -> OptionKind {
        match option_type {
            crate::instruments::OptionType::Call => OptionKind::Call,
            crate::instruments::OptionType::Put => OptionKind::Put,
        }
    }

    /// Price a barrier option using Heston Monte Carlo.
    fn price_internal(
        &self,
        inst: &BarrierOption,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(Money, f64)> {
        // Time to maturity
        let t = inst
            .day_count
            .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

        if t <= 0.0 {
            return price_expired_barrier(inst, market).map(|m| (m, 0.0));
        }

        let disc_curve = market.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        let r = crate::instruments::common_impl::helpers::zero_rate_from_df(
            discount_factor,
            t,
            "BarrierOption Heston MC discount curve",
        )?;
        if inst.observed_barrier_breached == Some(true)
            && inst.barrier_type.is_knock_out()
            && inst.rebate_timing
                == finstack_quant_models::closed_form::barrier::RebateTiming::AtHit
        {
            return Ok((Money::from((0_i64, inst.notional.currency())), 0.0));
        }

        let spot_scalar = market.get_price(&inst.spot_id)?;
        let spot = match spot_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            market,
            inst.div_yield_id.as_ref(),
        )?;

        // Get volatility (used for barrier bridge correction sigma)
        let vol_surface = market.get_surface(inst.vol_surface_id.as_str())?;
        let sigma = finstack_quant_models::volatility::get_surface_vol_clamped(
            &vol_surface,
            t,
            inst.strike,
        );

        let heston_params =
            crate::instruments::equity::equity_option::heston_market::heston_params_from_market_strict(
                market, r, q,
            )?;
        let v0 = heston_params.v0;
        let process = HestonProcess::new(heston_params);
        let discretization = QeHeston::new();

        let num_steps = ((t * self.steps_per_year).round() as usize).max(10);
        let time_grid = TimeGrid::uniform(t, num_steps)?;
        let maturity_step = time_grid.num_steps();

        // Create barrier payoff (uses vol-surface sigma for bridge correction)
        let mut payoff = BarrierOptionPayoff::new(
            inst.strike,
            inst.barrier.amount(),
            inst.barrier_type,
            Self::convert_option_kind(inst.option_type),
            inst.rebate.map(|m| m.amount()),
            inst.notional.amount(),
            maturity_step,
            sigma,
            &time_grid,
            BarrierMonitoring::Continuous { start_step: 0 },
        )
        .with_observed_barrier_breached(inst.observed_barrier_breached.unwrap_or(false));
        if super::pricer::wants_at_hit_rebate(inst) {
            payoff = payoff.with_rebate_at_hit(r);
        }

        let num_paths = inst
            .instrument_pricing_overrides
            .model_config
            .mc_paths
            .filter(|&n| n > 0)
            .unwrap_or(self.num_paths);

        // Derive deterministic seed
        let seed_val = if let Some(ref scenario) = inst.metric_pricing_overrides.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

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
            inst.notional.currency(),
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

        let (pv, stderr) = self.price_internal(barrier, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let mut result = ValuationResult::stamped(barrier.id(), as_of, pv);
        if stderr > 0.0 {
            result
                .measures
                .insert(crate::metrics::MetricId::custom("mc_stderr"), stderr);
        }
        Ok(result)
    }
}

/// Price an expired barrier option using explicit observed barrier state.
fn price_expired_barrier(
    inst: &BarrierOption,
    curves: &MarketContext,
) -> finstack_quant_core::Result<Money> {
    let spot_scalar = curves.get_price(&inst.spot_id)?;
    let spot = match spot_scalar {
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
        finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
    };

    let ccy = inst.notional.currency();
    let notional = inst.notional.amount();
    let is_knock_out = inst.barrier_type.is_knock_out();

    let barrier_breached = inst.observed_barrier_breached.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Expired barrier option requires `observed_barrier_breached` to determine realized payoff"
                .to_string(),
        )
    })?;

    let intrinsic = match inst.option_type {
        crate::instruments::OptionType::Call => (spot - inst.strike).max(0.0) * notional,
        crate::instruments::OptionType::Put => (inst.strike - spot).max(0.0) * notional,
    };
    let rebate = inst.rebate.map(|m| m.amount()).unwrap_or(0.0);

    let pv = if is_knock_out {
        if barrier_breached {
            rebate
        } else {
            intrinsic
        }
    } else {
        // Knock-in
        if barrier_breached {
            intrinsic
        } else {
            rebate
        }
    };

    Money::new(pv, ccy)
}
