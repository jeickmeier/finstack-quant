//! Interest rate option metrics module.
//!
//! Provides metric calculators specific to `CapFloor`. The calculators compose
//! with the shared metrics framework and are registered via
//! `register_cap_floor_metrics`.
//!
//! Exposed metrics:
//! - Delta
//! - Gamma
//! - Vega (market normal-vol quote sensitivity)
//! - HwSigmaVega (direct Hull-White short-rate sigma sensitivity)
//! - Theta
//! - Rho
//! - ImpliedVol (placeholder)

mod common;
mod dv01;
mod implied_vol;
mod theta;
mod vega;

use self::common::CapletInputs;
use crate::cap_floor::CapFloor;
use crate::cap_floor::RateOptionType;
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::sensitivities::cs01::sensitivity_central_diff;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all CapFloor metrics with the registry
pub(crate) fn register_cap_floor_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::dv01::{
        Dv01CalculatorConfig, UnifiedDv01Calculator,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CapFloor,
        metrics: [
            (Delta, DeltaCalculator),
            (Gamma, GammaCalculator),
            (Vega, vega::VegaCalculator),
            (HwSigmaVega, vega::HwSigmaVegaCalculator),
            (Dv01, dv01::Dv01Calculator),
            (Theta, theta::ThetaCalculator),
            // Rho = parallel bump of the discount curve only. Routing through
            // the unified DV01 calculator keeps Rho aligned with the workspace
            // bump-size config and central-difference convention.
            (Rho, UnifiedDv01Calculator::new(
                Dv01CalculatorConfig::parallel_discount_only(),
            )),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
            (ForwardPv01, ForwardPv01Calculator),
            (BucketedDv01, UnifiedDv01Calculator::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
        ]
    }
    Ok(())
}

/// Delta calculator (model-consistent forward delta, aggregated for caps/floors).
///
/// Dispatches to the appropriate model based on `vol_type`:
/// - `Lognormal`: Black-76 delta = N(d₁)
/// - `ShiftedLognormal`: Black-76 delta on shifted rates
/// - `Normal`: Bachelier delta = N(d)
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        if context.pricing_model().is_some_and(|model| {
            model == finstack_quant_valuations_core::pricer::ModelKey::HullWhite1F
        }) {
            return finstack_quant_valuations_core::metrics::sensitivities::option_greeks::selected_rate_greek(
                context,
                &option.forward_curve_id,
                false,
            );
        }
        let is_cap = matches!(
            option.rate_option_type,
            RateOptionType::Caplet | RateOptionType::Cap
        );
        common::aggregate_over_caplets(option, context, |c: CapletInputs| caplet_delta(is_cap, c))
    }
}

fn caplet_delta(is_cap: bool, c: CapletInputs) -> f64 {
    use crate::cap_floor::pricing::{black, normal};
    use finstack_quant_models::volatility::VolatilityConvention;
    let coupon_delta = match c.convention {
        VolatilityConvention::Normal => {
            normal::delta(is_cap, c.strike, c.forward, c.sigma, c.time_to_fixing)
        }
        _ => black::delta(is_cap, c.strike, c.forward, c.sigma, c.time_to_fixing),
    };
    coupon_delta * c.forward_sensitivity
}

/// Forward PV01 calculator (per 1bp parallel forward curve bump).
///
/// The projection curve alone is shifted, keeping its interpolation,
/// extrapolation and calibration metadata, and the option is repriced with
/// the pricing model the metric request selected. Discounting is unchanged.
pub(crate) struct ForwardPv01Calculator;

impl MetricCalculator for ForwardPv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        // A projection curve distinct from discounting must exist to be bumped.
        context.curves.get_forward(&option.forward_curve_id)?;

        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_pricing_overrides(),
        )?
        .rate_bump_bp;
        let reprice = |bp: f64| -> Result<f64> {
            let bumped = context.curves.bump([MarketBump::Curve {
                id: option.forward_curve_id.clone(),
                spec: BumpSpec::parallel_bp(bp),
            }])?;
            context.reprice_raw(&bumped, context.as_of)
        };
        Ok(sensitivity_central_diff(
            reprice(bump_bp)?,
            reprice(-bump_bp)?,
            bump_bp,
        ))
    }
}

/// Gamma calculator (model-consistent forward gamma, aggregated for caps/floors).
///
/// Dispatches to the appropriate model based on `vol_type`:
/// - `Lognormal`: Black-76 gamma = n(d₁) / (F·σ·√T)
/// - `ShiftedLognormal`: Black-76 gamma on shifted rates
/// - `Normal`: Bachelier gamma = n(d) / (σ·√T)
pub(crate) struct GammaCalculator;

impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        if context.pricing_model().is_some_and(|model| {
            model == finstack_quant_valuations_core::pricer::ModelKey::HullWhite1F
        }) {
            return finstack_quant_valuations_core::metrics::sensitivities::option_greeks::selected_rate_greek(
                context,
                &option.forward_curve_id,
                true,
            );
        }
        let is_cap = matches!(
            option.rate_option_type,
            RateOptionType::Caplet | RateOptionType::Cap
        );
        common::aggregate_over_caplets(option, context, |c: CapletInputs| caplet_gamma(is_cap, c))
    }
}

fn caplet_gamma(is_cap: bool, c: CapletInputs) -> f64 {
    use crate::cap_floor::pricing::{black, normal};
    use finstack_quant_models::volatility::VolatilityConvention;
    let (delta, gamma) = match c.convention {
        VolatilityConvention::Normal => (
            normal::delta(is_cap, c.strike, c.forward, c.sigma, c.time_to_fixing),
            normal::gamma(c.strike, c.forward, c.sigma, c.time_to_fixing),
        ),
        _ => (
            black::delta(is_cap, c.strike, c.forward, c.sigma, c.time_to_fixing),
            black::gamma(c.strike, c.forward, c.sigma, c.time_to_fixing),
        ),
    };
    gamma * c.forward_sensitivity * c.forward_sensitivity + delta * c.forward_second_sensitivity
}
