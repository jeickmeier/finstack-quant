//! FX Swap metrics module.
//!
//! Provides metric calculators specific to `FxSwap`.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_fx_swap_metrics`.
//!
//! Exposed metrics:
//! - Forward points (far rate - near rate)
//! - FX01 (sensitivity to a 1% relative spot move) — shared
//!   `GenericFx01Calculator`
//! - DV01 (domestic) and DV01 (foreign)

mod ir01_domestic;
mod ir01_foreign;

use crate::fx_swap::pricing_helper::FxSwapPricingContext;
use crate::fx_swap::FxSwap;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all FX Swap metrics with the registry
pub(crate) fn register_fx_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::MetricId;
    use finstack_quant_valuations_core::pricer::InstrumentType;
    use std::sync::Arc;

    // Custom metrics
    for (id, calculator) in [
        (
            MetricId::custom("carry_pv"),
            Arc::new(CarryPv) as Arc<dyn finstack_quant_valuations_core::metrics::MetricCalculator>,
        ),
        (MetricId::custom("forward_points"), Arc::new(ForwardPoints)),
        (
            MetricId::Fx01,
            finstack_quant_valuations_core::metrics::sensitivities::fx01::arc_generic_fx01(),
        ),
        (
            MetricId::FxDelta,
            finstack_quant_valuations_core::metrics::sensitivities::fx01::arc_generic_fx01(),
        ),
        (
            MetricId::Dv01Domestic,
            Arc::new(ir01_domestic::DomesticIR01),
        ),
        (MetricId::Dv01Foreign, Arc::new(ir01_foreign::ForeignIR01)),
    ] {
        registry.register_metric(id, calculator, &[InstrumentType::FxSwap])?;
    }

    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxSwap,
        metrics: [
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

/// Carry PV calculator for FX swaps.
///
/// Carry PV represents the PV contribution from the interest rate differential
/// between the two currencies. It is computed as the difference between:
/// - Near leg value at model spot (receive base, convert to quote)
/// - Far leg value at model forward (pay base, convert to quote)
///
/// Computes the present value contribution from holding the foreign leg
/// (converted to the domestic currency) using model-implied spot/forward
/// rates. This approximates the "carry" earned from the interest differential
/// between currencies when maintaining the swap position.
pub(crate) struct CarryPv;

impl MetricCalculator for CarryPv {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx_swap: &FxSwap = context.instrument_as()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        // Use shared pricing context for consistent calculations
        let ctx = FxSwapPricingContext::build(fx_swap, &curves, as_of)?;

        // Carry PV: model-implied near value minus model-implied far value
        // This captures the benefit/cost of the interest rate differential
        let term1 = if ctx.include_near {
            ctx.notional * ctx.model_spot * ctx.df_dom_near
        } else {
            0.0
        };
        let term2 = if ctx.include_far {
            ctx.notional * ctx.model_forward * ctx.df_dom_far
        } else {
            0.0
        };

        let carry_pv = term1 - term2;
        Ok(carry_pv)
    }
}

/// Forward points (far rate - near rate).
///
/// Forward points represent the interest rate differential between the two
/// currencies expressed in FX terms. When domestic rates exceed foreign rates,
/// forward points are positive (forward at premium).
///
/// Computes forward points as `far_rate - near_rate`, where the near rate is
/// either provided on the instrument or sourced from the FX matrix, and the far
/// rate is either provided or derived from covered interest parity using the
/// discount curves.
pub(crate) struct ForwardPoints;

impl MetricCalculator for ForwardPoints {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx_swap: &FxSwap = context.instrument_as()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        // Use shared pricing context for consistent calculations
        let ctx = FxSwapPricingContext::build(fx_swap, &curves, as_of)?;

        Ok(ctx.forward_points())
    }
}
