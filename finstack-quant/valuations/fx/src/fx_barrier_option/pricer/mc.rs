//! Monte Carlo pricer for continuous and discrete FX barrier options.

use super::{
    BarrierState, barrier_pricing_context, barrier_state, notional_value, validate_monitoring_state,
};
use crate::fx_barrier_option::types::FxBarrierOption;
use crate::market_inputs::FxOptionInputs;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::PayoutTiming;
use finstack_quant_models::monte_carlo::payoff::barrier::{
    BarrierOptionParams, BarrierOptionPayoff,
};
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

struct FxBarrierPricingOutcome {
    value: Money,
    diagnostics: Option<finstack_quant_valuations_core::results::MonteCarloValuationDetails>,
}

impl FxBarrierPricingOutcome {
    fn deterministic(value: Money) -> Self {
        Self {
            value,
            diagnostics: None,
        }
    }
}

/// FX barrier option Monte Carlo pricer.
pub struct FxBarrierOptionMcPricer {
    pub(super) config: PathDependentPricerConfig,
}

impl FxBarrierOptionMcPricer {
    /// Create a new FX barrier option MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    /// Price an FX barrier option using Monte Carlo.
    fn price_internal(
        &self,
        inst: &FxBarrierOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<FxBarrierPricingOutcome> {
        if as_of > inst.expiry {
            return Ok(FxBarrierPricingOutcome::deterministic(Money::from((
                0_i64,
                inst.quote_currency,
            ))));
        }

        let FxOptionInputs {
            spot: fx_spot,
            r_domestic: r_dom,
            r_foreign: r_for,
            sigma,
            t,
            df_domestic: discount_factor,
            ..
        } = match barrier_state(inst, curves, as_of)? {
            BarrierState::Fixed(per_unit) => {
                return Ok(FxBarrierPricingOutcome::deterministic(notional_value(
                    inst, per_unit,
                )?));
            }
            BarrierState::Live(inputs) => inputs,
        };

        // For FX, drift is r_dom - r_for.
        // In GBM process param 'q' is subtracted from r to get drift (r-q).
        // So q should be r_for.
        let q = r_for;
        let gbm_params = GbmParams::new(r_dom, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let mut config =
            finstack_quant_valuations_core::instruments::pricing::monte_carlo::merged_path_config(
                &self.config,
                &inst.instrument_pricing_overrides,
            )?;

        // Standard FX barrier: the GBM drift `r_dom - r_for` (set above via
        // `GbmParams`) fully describes the dynamics. Quanto barriers are not
        // supported by this 1D MC payoff — see `BarrierOptionPayoff` docs.
        let seed = inst
            .instrument_pricing_overrides
            .model_config
            .mc_seed(&inst.id);
        config.seed = seed;

        let (time_grid, monitoring) = inst.monitoring.time_grid(
            as_of,
            inst.day_count,
            inst.monitoring_start_date,
            t,
            &config,
        )?;
        let mut payoff = BarrierOptionPayoff::new(BarrierOptionParams {
            strike: inst.strike,
            barrier: inst.barrier,
            barrier_type: inst.barrier_type,
            option_type: inst.option_type,
            rebate: inst.rebate_per_unit(),
            notional: inst.notional.amount(),
            maturity_step: time_grid.num_steps(),
            sigma,
            time_grid: &time_grid,
            monitoring,
        });
        // Exact at-hit rebate timing: compound the rebate forward from the
        // hit time at the domestic rate so DF(T) nets to DF(τ).
        if inst.rebate.is_some() && inst.rebate_timing == Some(PayoutTiming::AtHit) {
            payoff = payoff.with_rebate_at_hit(r_dom);
        }

        let time_grid_values = time_grid.times().to_vec();
        let antithetic = config.antithetic;
        let sobol = config.use_sobol;
        let brownian_bridge = config.use_brownian_bridge;
        let pricer = PathDependentPricer::new(config);
        let result = pricer.price(
            &process,
            fx_spot,
            time_grid,
            &payoff,
            inst.quote_currency,
            discount_factor,
        )?;

        Ok(FxBarrierPricingOutcome {
            value: result.mean,
            diagnostics: Some(
                finstack_quant_valuations_core::results::MonteCarloValuationDetails {
                    model_key: ModelKey::MonteCarloGBM,
                    standard_error: result.stderr,
                    training_paths: 0,
                    training_simulated_paths: 0,
                    make_whole_training_paths: 0,
                    make_whole_training_simulated_paths: 0,
                    estimator_paths: result.num_paths,
                    simulated_paths: result.num_simulated_paths,
                    seed,
                    time_grid: time_grid_values,
                    antithetic,
                    sobol,
                    brownian_bridge,
                },
            ),
        })
    }
}

impl Default for FxBarrierOptionMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for FxBarrierOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::FxBarrierOption, ModelKey::MonteCarloGBM)
    }

    #[tracing::instrument(
        name = "fx_barrier_option.mc.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn finstack_quant_valuations_core::instruments::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let fx_barrier =
            expect_inst::<FxBarrierOption>(instrument, InstrumentType::FxBarrierOption)?;

        let context = barrier_pricing_context(fx_barrier, ModelKey::MonteCarloGBM);
        validate_monitoring_state(fx_barrier, as_of)
            .map_err(|error| PricingError::from_core(error, context.clone()))?;

        let outcome = self
            .price_internal(fx_barrier, market, as_of)
            .map_err(|error| PricingError::from_core(error, context))?;
        let mut result = ValuationResult::stamped(fx_barrier.id(), as_of, outcome.value);
        if let Some(diagnostics) = outcome.diagnostics {
            result = result.with_details(
                finstack_quant_valuations_core::results::ValuationDetails::MonteCarlo(diagnostics),
            );
        } else if let Some(details) = fx_barrier.valuation_details(market, as_of) {
            result = result.with_details(details);
        }
        Ok(result)
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    validate_monitoring_state(inst, as_of)?;
    let pricer = FxBarrierOptionMcPricer::new();
    pricer
        .price_internal(inst, curves, as_of)
        .map(|outcome| outcome.value)
}
