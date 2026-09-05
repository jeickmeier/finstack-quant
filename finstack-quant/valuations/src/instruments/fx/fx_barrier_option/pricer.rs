//! FX barrier option pricers (Monte Carlo and analytical).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::fx_barrier_option::types::{FxBarrierOption, Monitoring};
use crate::instruments::fx::shared::{
    collect_fx_option_inputs, resolve_fx_spot as resolve_shared_fx_spot, FxOptionInputRequest,
    FxSpotSource,
};
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

// MC-specific imports
use finstack_quant_models::monte_carlo::payoff::barrier::{
    BarrierMonitoring as McBarrierMonitoring, BarrierOptionPayoff, OptionKind as McOptionKind,
};
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess};
use finstack_quant_models::monte_carlo::TimeGrid;

struct FxBarrierPricingOutcome {
    value: Money,
    diagnostics: Option<crate::results::MonteCarloValuationDetails>,
}

impl FxBarrierPricingOutcome {
    fn deterministic(value: Money) -> Self {
        Self {
            value,
            diagnostics: None,
        }
    }
}

fn barrier_pricing_context(inst: &FxBarrierOption, model: ModelKey) -> PricingErrorContext {
    let mut context = PricingErrorContext::from_instrument(inst)
        .model(model)
        .curve_ids([
            inst.domestic_discount_curve_id.as_str(),
            inst.foreign_discount_curve_id.as_str(),
            inst.vol_surface_id.as_str(),
        ]);
    if let Some(spot_id) = &inst.fx_spot_id {
        context = context.curve_id(spot_id.as_str());
    }
    context
}

/// FX barrier option Monte Carlo pricer.
pub struct FxBarrierOptionMcPricer {
    config: PathDependentPricerConfig,
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
        inst.validate()?;
        if as_of > inst.expiry {
            return Ok(FxBarrierPricingOutcome::deterministic(Money::from((
                0_i64,
                inst.quote_currency,
            ))));
        }

        let (fx_spot, t) = collect_fx_barrier_expiry_state(inst, curves, as_of)?;
        if t <= 0.0 {
            let per_unit = expired_barrier_value_per_unit(inst, fx_spot)?;
            return Ok(FxBarrierPricingOutcome::deterministic(Money::new(
                per_unit * inst.notional.amount(),
                inst.quote_currency,
            )?));
        }

        let (_, r_dom, r_for, sigma, discount_factor) =
            collect_fx_barrier_inputs(inst, curves, as_of)?;

        if inst.observed_barrier_breached == Some(true) {
            let per_unit = seasoned_breached_value_per_unit(
                inst,
                fx_spot,
                r_dom,
                r_for,
                sigma,
                t,
                discount_factor,
            );
            return Ok(FxBarrierPricingOutcome::deterministic(Money::new(
                per_unit * inst.notional.amount(),
                inst.quote_currency,
            )?));
        }

        // For FX, drift is r_dom - r_for.
        // In GBM process param 'q' is subtracted from r to get drift (r-q).
        // So q should be r_for.
        let q = r_for;
        let gbm_params = GbmParams::new(r_dom, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let mut config = crate::instruments::common_impl::helpers::merged_path_config(
            &self.config,
            &inst.instrument_pricing_overrides,
        )?;

        // Standard FX barrier: the GBM drift `r_dom - r_for` (set above via
        // `GbmParams`) fully describes the dynamics. Quanto barriers are not
        // supported by this 1D MC payoff — see `BarrierOptionPayoff` docs.
        let mc_option_kind = match inst.option_type {
            crate::instruments::OptionType::Call => McOptionKind::Call,
            crate::instruments::OptionType::Put => McOptionKind::Put,
        };
        use finstack_quant_models::monte_carlo::seed;

        let seed = if let Some(scenario) = &inst.metric_pricing_overrides.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };
        config.seed = seed;

        let (time_grid, monitoring) = barrier_time_grid(inst, as_of, t, &config)?;
        let mut payoff = BarrierOptionPayoff::new(
            inst.strike,
            inst.barrier,
            inst.barrier_type,
            mc_option_kind,
            inst.rebate,
            inst.notional.amount(),
            time_grid.num_steps(),
            sigma,
            &time_grid,
            monitoring,
        );
        // Exact at-hit rebate timing: compound the rebate forward from the
        // hit time at the domestic rate so DF(T) nets to DF(τ).
        {
            use finstack_quant_models::closed_form::barrier::RebateTiming;
            if inst.rebate.is_some() && inst.rebate_timing == RebateTiming::AtHit {
                payoff = payoff.with_rebate_at_hit(r_dom);
            }
        }

        let time_grid_values = time_grid.times().to_vec();
        let antithetic = config.antithetic;
        let sobol = config.use_sobol;
        let brownian_bridge = config.use_brownian_bridge;
        let pricer = PathDependentPricer::new(config);
        let result = pricer.price_with_grid(
            &process,
            fx_spot,
            time_grid,
            &payoff,
            inst.quote_currency,
            discount_factor,
        )?;

        Ok(FxBarrierPricingOutcome {
            value: result.mean,
            diagnostics: Some(crate::results::MonteCarloValuationDetails {
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
            }),
        })
    }
}
fn barrier_time_grid(
    inst: &FxBarrierOption,
    as_of: Date,
    time_to_maturity: f64,
    config: &PathDependentPricerConfig,
) -> finstack_quant_core::Result<(TimeGrid, McBarrierMonitoring)> {
    let date_time = |date: Date| {
        inst.day_count
            .year_fraction(as_of, date, DayCountContext::default())
    };

    let required_times = match &inst.monitoring {
        Monitoring::Continuous => {
            let start = inst.monitoring_start_date.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "FxBarrierOption requires monitoring_start_date".to_string(),
                )
            })?;
            if start <= as_of {
                vec![0.0]
            } else {
                vec![date_time(start)?]
            }
        }
        Monitoring::Discrete { observation_dates } => observation_dates
            .iter()
            .copied()
            .filter(|date| *date >= as_of)
            .map(date_time)
            .collect::<finstack_quant_core::Result<Vec<_>>>()?,
    };
    let time_grid = config.build_time_grid(time_to_maturity, &required_times)?;
    let step_for_time = |required: f64| {
        let tolerance = 1.0e-12 * required.abs().max(1.0);
        time_grid
            .times()
            .iter()
            .position(|time| (*time - required).abs() <= tolerance)
            .ok_or_else(|| {
                finstack_quant_core::Error::Internal(format!(
                    "FX barrier required monitoring time {required} is missing from the simulation grid"
                ))
            })
    };
    let monitoring = match &inst.monitoring {
        Monitoring::Continuous => McBarrierMonitoring::Continuous {
            start_step: step_for_time(required_times[0])?,
        },
        Monitoring::Discrete { .. } => McBarrierMonitoring::Discrete {
            observation_steps: required_times
                .iter()
                .copied()
                .map(step_for_time)
                .collect::<finstack_quant_core::Result<Vec<_>>>()?,
        },
    };
    Ok((time_grid, monitoring))
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

    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
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
            result = result.with_details(crate::results::ValuationDetails::MonteCarlo(diagnostics));
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
    if as_of > inst.expiry {
        return Ok(Money::from((0_i64, inst.quote_currency)));
    }
    let pricer = FxBarrierOptionMcPricer::new();
    pricer
        .price_internal(inst, curves, as_of)
        .map(|outcome| outcome.value)
}

fn validate_monitoring_state(
    inst: &FxBarrierOption,
    as_of: Date,
) -> finstack_quant_core::Result<()> {
    let has_past_monitoring = match &inst.monitoring {
        Monitoring::Continuous => {
            let start = inst.monitoring_start_date.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "FxBarrierOption requires monitoring_start_date".to_string(),
                )
            })?;
            as_of > start
        }
        Monitoring::Discrete { observation_dates } => {
            observation_dates.iter().any(|date| *date < as_of)
        }
    };
    if has_past_monitoring && as_of <= inst.expiry && inst.observed_barrier_breached.is_none() {
        return Err(finstack_quant_core::Error::Validation(
            "Seasoned FX barrier option requires observed_barrier_breached after monitoring starts"
                .to_string(),
        ));
    }
    Ok(())
}

use finstack_quant_core::types::BarrierType as AnalyticalBarrierType;
use finstack_quant_models::closed_form::barrier::{
    barrier_call_continuous, barrier_put_continuous, barrier_rebate, BarrierParams,
};

fn expired_barrier_value_per_unit(
    inst: &FxBarrierOption,
    spot: f64,
) -> finstack_quant_core::Result<f64> {
    let strike = inst.strike;
    let is_knock_in = inst.barrier_type.is_knock_in();
    let barrier_hit = inst.observed_barrier_breached.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Expired FX barrier option requires `observed_barrier_breached` to determine realized payoff"
                .to_string(),
        )
    })?;
    let activated = if is_knock_in {
        barrier_hit
    } else {
        !barrier_hit
    };

    let intrinsic = if activated {
        match inst.option_type {
            crate::instruments::OptionType::Call => (spot - strike).max(0.0),
            crate::instruments::OptionType::Put => (strike - spot).max(0.0),
        }
    } else {
        0.0
    };

    let rebate_due = if is_knock_in {
        !barrier_hit
    } else {
        barrier_hit
    };
    let rebate = if rebate_due {
        inst.rebate.unwrap_or(0.0)
    } else {
        0.0
    };

    Ok(intrinsic + rebate)
}

#[allow(clippy::too_many_arguments)]
fn seasoned_breached_value_per_unit(
    inst: &FxBarrierOption,
    spot: f64,
    r_dom: f64,
    r_for: f64,
    sigma: f64,
    t: f64,
    discount_factor: f64,
) -> f64 {
    if inst.barrier_type.is_knock_in() {
        finstack_quant_models::closed_form::vanilla::bs_price_unchecked(
            spot,
            inst.strike,
            r_dom,
            r_for,
            sigma,
            t,
            inst.option_type,
        )
    } else {
        match inst.rebate_timing {
            finstack_quant_models::closed_form::barrier::RebateTiming::AtHit => 0.0,
            finstack_quant_models::closed_form::barrier::RebateTiming::AtExpiry => {
                inst.rebate.unwrap_or(0.0) * discount_factor
            }
        }
    }
}

fn resolve_fx_spot(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    resolve_shared_fx_spot(FxOptionInputRequest {
        market: curves,
        as_of,
        base_currency: inst.base_currency,
        quote_currency: inst.quote_currency,
        expiry: inst.expiry,
        day_count: inst.day_count,
        domestic_discount_curve_id: &inst.domestic_discount_curve_id,
        foreign_discount_curve_id: &inst.foreign_discount_curve_id,
        vol_surface_id: inst.vol_surface_id.as_str(),
        strike: inst.strike,
        instrument_pricing_overrides: &inst.instrument_pricing_overrides,
        spot_source: FxSpotSource::ScalarId(inst.fx_spot_id.as_ref()),
        rate_context: "FxBarrierOption",
    })
}

fn collect_fx_barrier_expiry_state(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<(f64, f64)> {
    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;
    let fx_spot = resolve_fx_spot(inst, curves, as_of)?;
    Ok((fx_spot, t))
}

/// Helper to collect inputs for FX barrier option pricing.
fn collect_fx_barrier_inputs(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<(f64, f64, f64, f64, f64)> {
    let inputs = collect_fx_option_inputs(FxOptionInputRequest {
        market: curves,
        as_of,
        base_currency: inst.base_currency,
        quote_currency: inst.quote_currency,
        expiry: inst.expiry,
        day_count: inst.day_count,
        domestic_discount_curve_id: &inst.domestic_discount_curve_id,
        foreign_discount_curve_id: &inst.foreign_discount_curve_id,
        vol_surface_id: inst.vol_surface_id.as_str(),
        strike: inst.strike,
        instrument_pricing_overrides: &inst.instrument_pricing_overrides,
        spot_source: FxSpotSource::ScalarId(inst.fx_spot_id.as_ref()),
        rate_context: "FxBarrierOption",
    })?;
    let sigma = inputs.sigma;
    if !sigma.is_finite() || sigma < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxBarrierOption volatility must be finite and non-negative, got {}",
            sigma
        )));
    }

    let domestic_df = curves
        .get_discount(inst.domestic_discount_curve_id.as_str())?
        .df_between_dates(as_of, inst.expiry)?;

    Ok((
        inputs.spot,
        inputs.r_domestic,
        inputs.r_foreign,
        sigma,
        domestic_df,
    ))
}

/// FX Barrier option analytical pricer (continuous monitoring).
pub(crate) struct FxBarrierOptionAnalyticalPricer;

impl FxBarrierOptionAnalyticalPricer {
    /// Create a new analytical FX barrier option pricer
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for FxBarrierOptionAnalyticalPricer {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the BS barrier price + optional rebate (without notional scaling).
fn bs_barrier_price_per_unit(
    fx_barrier: &FxBarrierOption,
    fx_spot: f64,
    r_dom: f64,
    r_for: f64,
    sigma: f64,
    t: f64,
    analytical_barrier_type: AnalyticalBarrierType,
) -> f64 {
    let params = BarrierParams::new(
        fx_spot,
        fx_barrier.strike,
        fx_barrier.barrier,
        t,
        r_dom,
        r_for,
        sigma,
    );
    let price = match fx_barrier.option_type {
        crate::instruments::OptionType::Call => {
            barrier_call_continuous(&params, analytical_barrier_type)
        }
        crate::instruments::OptionType::Put => {
            barrier_put_continuous(&params, analytical_barrier_type)
        }
    };

    let rebate_val = if let Some(rebate) = fx_barrier.rebate {
        barrier_rebate(
            &params,
            rebate,
            analytical_barrier_type,
            fx_barrier.rebate_timing,
        )
    } else {
        0.0
    };

    price + rebate_val
}

impl Pricer for FxBarrierOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::FxBarrierOption,
            ModelKey::FxBarrierBSContinuous,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let fx_barrier =
            expect_inst::<FxBarrierOption>(instrument, InstrumentType::FxBarrierOption)?;

        let context = barrier_pricing_context(fx_barrier, ModelKey::FxBarrierBSContinuous);
        fx_barrier
            .validate()
            .map_err(|error| PricingError::from_core(error, context.clone()))?;
        validate_monitoring_state(fx_barrier, as_of)
            .map_err(|error| PricingError::from_core(error, context.clone()))?;

        if as_of > fx_barrier.expiry {
            return Ok(ValuationResult::stamped(
                fx_barrier.id(),
                as_of,
                Money::from((0_i64, fx_barrier.quote_currency)),
            ));
        }

        if matches!(fx_barrier.monitoring, Monitoring::Discrete { .. }) {
            return Err(PricingError::invalid_input_with_context(
                "Discrete FX barrier monitoring requires the Monte Carlo pricer; the analytical \
                 Reiner-Rubinstein pricer assumes continuous monitoring.",
                context,
            ));
        }

        let (fx_spot, t) = collect_fx_barrier_expiry_state(fx_barrier, market, as_of)
            .map_err(|error| PricingError::from_core(error, context.clone()))?;

        if t <= 0.0 {
            let per_unit = expired_barrier_value_per_unit(fx_barrier, fx_spot)
                .map_err(|error| PricingError::from_core(error, context.clone()))?;
            return Ok(ValuationResult::stamped(
                fx_barrier.id(),
                as_of,
                Money::new(
                    per_unit * fx_barrier.notional.amount(),
                    fx_barrier.quote_currency,
                )
                .map_err(|error| {
                    crate::pricer::PricingError::from_core(
                        error,
                        crate::pricer::PricingErrorContext::from_instrument(fx_barrier),
                    )
                })?,
            ));
        }

        let (_, r_dom, r_for, sigma, discount_factor) =
            collect_fx_barrier_inputs(fx_barrier, market, as_of)
                .map_err(|error| PricingError::from_core(error, context))?;

        if fx_barrier.observed_barrier_breached == Some(true) {
            let per_unit = seasoned_breached_value_per_unit(
                fx_barrier,
                fx_spot,
                r_dom,
                r_for,
                sigma,
                t,
                discount_factor,
            );
            return Ok(ValuationResult::stamped(
                fx_barrier.id(),
                as_of,
                Money::new(
                    per_unit * fx_barrier.notional.amount(),
                    fx_barrier.quote_currency,
                )
                .map_err(|error| {
                    crate::pricer::PricingError::from_core(
                        error,
                        crate::pricer::PricingErrorContext::from_instrument(fx_barrier),
                    )
                })?,
            ));
        }

        let analytical_barrier_type = fx_barrier.barrier_type;

        let price_per_unit = bs_barrier_price_per_unit(
            fx_barrier,
            fx_spot,
            r_dom,
            r_for,
            sigma,
            t,
            analytical_barrier_type,
        );

        let pv = Money::new(
            price_per_unit * fx_barrier.notional.amount(),
            fx_barrier.quote_currency,
        )
        .map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(fx_barrier),
            )
        })?;
        Ok(ValuationResult::stamped(fx_barrier.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::Instrument;
    use crate::instruments::OptionType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::BarrierType;
    use finstack_quant_models::closed_form::barrier::{barrier_rebate, RebateTiming};
    use std::sync::Arc;
    use time::Month;

    #[test]
    fn analytical_barrier_error_preserves_category_and_context() {
        let mut inst = FxBarrierOption::example();
        let start = inst.monitoring_start_date.expect("example start");
        inst.monitoring = Monitoring::Discrete {
            observation_dates: vec![start, inst.expiry],
        };

        let error = FxBarrierOptionAnalyticalPricer::new()
            .price_dyn(&inst, &MarketContext::new(), start)
            .expect_err("discrete contract must reject analytical model");
        let PricingError::InvalidInput { context, .. } = error else {
            panic!("monitoring/model mismatch must remain an invalid-input error");
        };
        assert_eq!(context.instrument_id.as_deref(), Some(inst.id.as_str()));
        assert_eq!(
            context.instrument_type,
            Some(InstrumentType::FxBarrierOption)
        );
        assert_eq!(context.model, Some(ModelKey::FxBarrierBSContinuous));
        assert!(context
            .curve_ids
            .contains(&inst.domestic_discount_curve_id.to_string()));
        assert!(context.curve_ids.contains(&inst.vol_surface_id.to_string()));
    }

    #[test]
    fn expired_up_and_in_call_returns_intrinsic_when_hit() {
        let mut inst = FxBarrierOption::example();
        inst.option_type = OptionType::Call;
        inst.barrier_type = BarrierType::UpAndIn;
        inst.strike = 1.10;
        inst.barrier = 1.20;
        inst.rebate = None;
        inst.observed_barrier_breached = Some(true);

        let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
        assert!((per_unit - 0.15).abs() < 1e-12);
    }

    #[test]
    fn expired_down_and_out_put_returns_intrinsic_when_not_hit() {
        let mut inst = FxBarrierOption::example();
        inst.option_type = OptionType::Put;
        inst.barrier_type = BarrierType::DownAndOut;
        inst.strike = 1.10;
        inst.barrier = 0.90;
        inst.rebate = None;
        inst.observed_barrier_breached = Some(false);

        // Barrier not hit at expiry => down-and-out stays active => intrinsic applies.
        let per_unit = expired_barrier_value_per_unit(&inst, 1.00).expect("expired value");
        assert!((per_unit - 0.10).abs() < 1e-12);
    }

    #[test]
    fn expired_up_and_out_with_hit_pays_rebate_only() {
        let mut inst = FxBarrierOption::example();
        inst.option_type = OptionType::Call;
        inst.barrier_type = BarrierType::UpAndOut;
        inst.strike = 1.10;
        inst.barrier = 1.20;
        inst.rebate = Some(0.02);
        inst.observed_barrier_breached = Some(true);

        // Barrier hit at expiry => knocked out. With rebate, no intrinsic and rebate paid.
        let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
        assert!((per_unit - 0.02).abs() < 1e-12);
    }

    #[test]
    fn expired_up_and_in_with_no_hit_pays_rebate_only() {
        let mut inst = FxBarrierOption::example();
        inst.option_type = OptionType::Call;
        inst.barrier_type = BarrierType::UpAndIn;
        inst.strike = 1.10;
        inst.barrier = 1.20;
        inst.rebate = Some(0.02);
        inst.observed_barrier_breached = Some(false);

        let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
        assert!((per_unit - 0.02).abs() < 1e-12);
    }

    #[test]
    fn expired_fx_barrier_requires_observed_state() {
        let mut inst = FxBarrierOption::example();
        inst.observed_barrier_breached = None;

        let err = expired_barrier_value_per_unit(&inst, 1.25).expect_err("missing observed state");
        assert!(
            err.to_string().contains("observed_barrier_breached"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validation_allows_barrier_equal_to_strike() {
        let mut inst = FxBarrierOption::example();
        inst.strike = 1.10;
        inst.barrier = 1.10;

        inst.validate()
            .expect("equal strike/barrier should remain valid");
    }

    #[test]
    fn expired_analytical_value_only_requires_observed_state_and_spot() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let mut option = FxBarrierOption::example();
        option.expiry = as_of;
        option.monitoring = Monitoring::Continuous;
        option.option_type = OptionType::Call;
        option.barrier_type = BarrierType::UpAndIn;
        option.rebate = Some(0.02);
        option.observed_barrier_breached = Some(false);

        let market = MarketContext::new().insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.25));

        let pv = option
            .value(&market, as_of)
            .expect("expired analytical value");
        assert!(
            (pv.amount() - 20_000.0).abs() < 1e-8,
            "expired FX barrier should settle from observed state and spot only, got {}",
            pv.amount()
        );
    }

    #[test]
    fn analytical_pricer_handles_zero_vol_knock_in_rebate_end_to_end() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let option = FxBarrierOption::builder()
            .id("FXBAR-ZERO-VOL-UPIN".into())
            .strike(1.10)
            .barrier(1.20)
            .rebate(0.02)
            .option_type(OptionType::Call)
            .barrier_type(BarrierType::UpAndIn)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .monitoring(Monitoring::Continuous)
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .attributes(crate::instruments::Attributes::new())
            .build()
            .expect("fx barrier option");

        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .build()
                    .expect("dom curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .build()
                    .expect("for curve"),
            )
            .insert_surface(
                VolSurface::builder("EURUSD-VOL")
                    .expiries(&[0.25, 0.5, 1.0])
                    .strikes(&[1.0, 1.1, 1.2])
                    .row(&[0.0, 0.0, 0.0])
                    .row(&[0.0, 0.0, 0.0])
                    .row(&[0.0, 0.0, 0.0])
                    .build()
                    .expect("vol surface"),
            )
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));

        let pv = option.value(&market, as_of).expect("fx barrier pv");
        assert!(
            (pv.amount() - 20_000.0).abs() < 1e-8,
            "zero-vol no-hit knock-in rebate should settle at rebate * notional, got {}",
            pv.amount()
        );
        assert_eq!(pv.currency(), Currency::USD);
    }

    #[test]
    fn analytical_pricer_finite_vol_rebate_delta_matches_closed_form_and_notional() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let notional = 1_000_000.0;
        let rebate = 0.02;

        let build_option = |id: &str, rebate: Option<f64>| {
            FxBarrierOption::builder()
                .id(id.into())
                .strike(1.10)
                .barrier(1.20)
                .rebate_opt(rebate)
                .rebate_timing(RebateTiming::AtExpiry)
                .option_type(OptionType::Call)
                .barrier_type(BarrierType::UpAndOut)
                .monitoring_start_date(as_of)
                .expiry(expiry)
                .notional(Money::new(notional, Currency::EUR).expect("valid money fixture"))
                .base_currency(Currency::EUR)
                .quote_currency(Currency::USD)
                .day_count(DayCount::Act365F)
                .monitoring(Monitoring::Continuous)
                .domestic_discount_curve_id("USD-OIS".into())
                .foreign_discount_curve_id("EUR-OIS".into())
                .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
                .vol_surface_id("EURUSD-VOL".into())
                .attributes(crate::instruments::Attributes::new())
                .build()
                .expect("finite-vol FX barrier option")
        };

        let base_option = build_option("FXBAR-FINITE-VOL-NO-REBATE", None);
        let rebate_option = build_option("FXBAR-FINITE-VOL-REBATE", Some(rebate));
        let domestic_rate: f64 = 0.03;
        let foreign_rate: f64 = 0.01;
        let volatility: f64 = 0.15;
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .day_count(DayCount::Act365F)
                    .knots([(0.0, 1.0), (5.0, (-domestic_rate * 5.0).exp())])
                    .interp(InterpStyle::LogLinear)
                    .build()
                    .expect("domestic curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .day_count(DayCount::Act365F)
                    .knots([(0.0, 1.0), (5.0, (-foreign_rate * 5.0).exp())])
                    .interp(InterpStyle::LogLinear)
                    .build()
                    .expect("foreign curve"),
            )
            .insert_surface(
                VolSurface::builder("EURUSD-VOL")
                    .expiries(&[0.5, 1.0, 2.0])
                    .strikes(&[1.0, 1.1, 1.2])
                    .row(&[volatility, volatility, volatility])
                    .row(&[volatility, volatility, volatility])
                    .row(&[volatility, volatility, volatility])
                    .build()
                    .expect("vol surface"),
            )
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));

        let base_pv = base_option
            .value(&market, as_of)
            .expect("finite-vol FX barrier price without rebate");
        let rebate_pv = rebate_option
            .value(&market, as_of)
            .expect("finite-vol FX barrier price with rebate");

        let (spot, r_dom, r_for, sigma, _) =
            collect_fx_barrier_inputs(&rebate_option, &market, as_of)
                .expect("finite-vol FX barrier inputs");
        let t = rebate_option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let params = BarrierParams::new(
            spot,
            rebate_option.strike,
            rebate_option.barrier,
            t,
            r_dom,
            r_for,
            sigma,
        );
        let expected_per_unit = barrier_rebate(
            &params,
            rebate,
            BarrierType::UpAndOut,
            RebateTiming::AtExpiry,
        );
        let continuous_per_unit = barrier_rebate(
            &params,
            rebate,
            BarrierType::UpAndOut,
            RebateTiming::AtExpiry,
        );
        let actual_delta = rebate_pv.amount() - base_pv.amount();
        let expected_delta = expected_per_unit * notional;

        assert!(
            sigma.is_finite() && sigma > 0.0,
            "test must exercise a finite positive volatility, got {sigma}"
        );
        assert!(
            (expected_per_unit - continuous_per_unit).abs() < 1e-12,
            "at-expiry rebate must match the continuous legacy formula"
        );
        assert!(
            (actual_delta - expected_delta).abs() < 0.02,
            "instrument rebate delta {actual_delta} should equal per-unit rebate {expected_per_unit} scaled by notional {notional}"
        );
        assert_eq!(base_pv.currency(), Currency::USD);
        assert_eq!(rebate_pv.currency(), Currency::USD);
    }

    #[test]
    fn validation_rejects_currency_mismatch_and_invalid_numeric_fields() {
        let mut mismatched = FxBarrierOption::example();
        mismatched.notional = Money::from((1_000_000_i64, Currency::USD));
        let err = mismatched.validate().expect_err("currency mismatch");
        assert!(err.to_string().contains("Currency mismatch"));

        let mut bad_strike = FxBarrierOption::example();
        bad_strike.strike = 0.0;
        assert!(bad_strike
            .validate()
            .expect_err("bad strike")
            .to_string()
            .contains("strike"));

        let mut bad_barrier = FxBarrierOption::example();
        bad_barrier.barrier = f64::NAN;
        assert!(bad_barrier
            .validate()
            .expect_err("bad barrier")
            .to_string()
            .contains("barrier"));

        let mut bad_notional = FxBarrierOption::example();
        bad_notional.notional = Money::from((0_i64, Currency::EUR));
        assert!(bad_notional
            .validate()
            .expect_err("bad notional")
            .to_string()
            .contains("notional"));
    }

    #[test]
    fn resolve_fx_spot_uses_fx_matrix_when_spot_id_is_absent() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut inst = FxBarrierOption::example();
        inst.fx_spot_id = None;

        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.15)
            .expect("valid quote");
        let market = MarketContext::new().insert_fx(FxMatrix::new(provider));

        let spot = resolve_fx_spot(&inst, &market, as_of).expect("fx matrix spot");
        assert!((spot - 1.15).abs() < 1e-12);
    }

    #[test]
    fn resolve_fx_spot_requires_valid_spot_source() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut no_matrix = FxBarrierOption::example();
        no_matrix.fx_spot_id = None;
        let err =
            resolve_fx_spot(&no_matrix, &MarketContext::new(), as_of).expect_err("missing matrix");
        assert!(err.to_string().contains("fx_matrix"));

        let mut price_scalar = FxBarrierOption::example();
        price_scalar.fx_spot_id = Some("EURUSD-SPOT".into());
        let price_market = MarketContext::new().insert_price(
            "EURUSD-SPOT",
            MarketScalar::Price(Money::new(1.10, Currency::USD).expect("valid money fixture")),
        );
        let spot = resolve_fx_spot(&price_scalar, &price_market, as_of).expect("price scalar spot");
        assert!((spot - 1.10).abs() < 1e-12);

        let bad_market =
            MarketContext::new().insert_price("EURUSD-SPOT", MarketScalar::Unitless(0.0));
        let err = resolve_fx_spot(&price_scalar, &bad_market, as_of).expect_err("bad scalar");
        assert!(err.to_string().contains("spot must be finite and > 0"));
    }

    /// Regression: MC pricer must honour option_type for puts.
    ///
    /// A deep-ITM down-and-out put with a far barrier priced through the
    /// discrete-monitoring MC path must remain close to the continuous
    /// analytical benchmark.
    /// This catches regressions that evaluate the terminal payoff as a call
    /// instead of honoring `option_type`.
    #[test]
    fn mc_barrier_put_honours_option_type() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Deep-ITM down-and-out put: spot=1.10, strike=1.30, barrier=0.80
        // spot is well below strike => put is deep ITM.
        // barrier=0.80 is far below spot=1.10 => very unlikely to knock out.
        // We use a moderate vol so the MC path stays active.
        let mc_option = FxBarrierOption::builder()
            .id("FXBAR-MC-PUT-BUG".into())
            .strike(1.30)
            .barrier(0.80)
            .rebate_opt(None)
            .option_type(OptionType::Put)
            .barrier_type(BarrierType::DownAndOut)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .monitoring(Monitoring::Discrete {
                observation_dates: vec![as_of, expiry],
            })
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("mc put option");

        // Matching continuous-monitoring analytical option.
        let analytical_option = FxBarrierOption::builder()
            .id("FXBAR-ANAL-PUT-BUG".into())
            .strike(1.30)
            .barrier(0.80)
            .rebate_opt(None)
            .option_type(OptionType::Put)
            .barrier_type(BarrierType::DownAndOut)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .monitoring(Monitoring::Continuous)
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("analytical put option");

        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.97)])
                    .build()
                    .expect("dom curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.98)])
                    .build()
                    .expect("for curve"),
            )
            .insert_surface(
                VolSurface::builder("EURUSD-VOL")
                    .expiries(&[0.25, 0.5, 1.0])
                    .strikes(&[0.9, 1.1, 1.3])
                    .row(&[0.10, 0.10, 0.10])
                    .row(&[0.10, 0.10, 0.10])
                    .row(&[0.10, 0.10, 0.10])
                    .build()
                    .expect("vol surface"),
            )
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));

        use crate::instruments::common_impl::traits::Instrument;

        let mc_pv = mc_option
            .value(&market, as_of)
            .expect("MC put price")
            .amount();

        let diagnostic_result = FxBarrierOptionMcPricer::new()
            .price_dyn(&mc_option, &market, as_of)
            .expect("MC diagnostic result");
        let Some(crate::results::ValuationDetails::MonteCarlo(diagnostics)) =
            diagnostic_result.details
        else {
            panic!("MC barrier result must include typed diagnostics");
        };
        assert!(diagnostics.standard_error.is_finite());
        assert!(diagnostics.estimator_paths > 0);
        assert!(diagnostics.simulated_paths >= diagnostics.estimator_paths);
        assert_eq!(
            diagnostics.time_grid.first().copied(),
            Some(0.0),
            "time grid must include valuation time"
        );
        let expected_maturity_time = mc_option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("maturity time");
        assert_eq!(
            diagnostics.time_grid.last().copied(),
            Some(expected_maturity_time),
            "time grid must end at contractual maturity"
        );

        let analytical_pv = analytical_option
            .value(&market, as_of)
            .expect("analytical put price")
            .amount();

        // Both must be positive (deep ITM put, barrier not hit)
        assert!(
            mc_pv > 0.0,
            "MC put price should be positive (deep ITM), got {}",
            mc_pv
        );
        assert!(
            analytical_pv > 0.0,
            "Analytical put price should be positive (deep ITM), got {}",
            analytical_pv
        );

        // MC and analytical must agree within 10% (MC tolerance for 100K paths)
        let rel_err = (mc_pv - analytical_pv).abs() / analytical_pv;
        assert!(
            rel_err < 0.10,
            "MC put price {} differs from analytical {} by {:.1}% (>10%), \
             option_type is likely being ignored in MC path",
            mc_pv,
            analytical_pv,
            rel_err * 100.0
        );
    }

    #[test]
    fn mc_inputs_pass_domestic_discount_factor_not_year_fraction() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let expiry = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let option = FxBarrierOption::builder()
            .id("FXBAR-MC-DF".into())
            .strike(1.10)
            .barrier(1.30)
            .rebate_opt(None)
            .option_type(OptionType::Call)
            .barrier_type(BarrierType::UpAndOut)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .monitoring(Monitoring::Discrete {
                observation_dates: vec![as_of, expiry],
            })
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("fx barrier option");

        let domestic_df_at_two_years = (-0.03_f64 * 2.0).exp();
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (2.0, domestic_df_at_two_years)])
                    .build()
                    .expect("dom curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (2.0, (-0.01_f64 * 2.0).exp())])
                    .build()
                    .expect("for curve"),
            )
            .insert_surface(
                VolSurface::builder("EURUSD-VOL")
                    .expiries(&[2.0])
                    .strikes(&[1.10])
                    .row(&[0.12])
                    .build()
                    .expect("vol surface"),
            )
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));

        let expected_domestic_df = market
            .get_discount("USD-OIS")
            .expect("dom curve")
            .df_between_dates(as_of, expiry)
            .expect("domestic df");
        let (_, _, _, _, discount_factor) =
            collect_fx_barrier_inputs(&option, &market, as_of).expect("inputs");

        assert!(
            (discount_factor - expected_domestic_df).abs() < 1e-12,
            "MC discount factor must be domestic DF {expected_domestic_df}, got {discount_factor}"
        );
    }
}
