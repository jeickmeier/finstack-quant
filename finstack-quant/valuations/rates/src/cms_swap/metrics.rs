//! CMS swap metrics module.
//!
//! Provides risk metrics for CMS swaps:
//! - **DV01**: Parallel rate sensitivity (unified calculator)
//! - **BucketedDv01**: Key-rate sensitivities
//! - **ConvexityAdjustmentRisk**: Dollar value of the convexity adjustment

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register CMS swap metrics with the registry.
pub(crate) fn register_cms_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CmsSwap,
        metrics: [
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (ConvexityAdjustmentRisk, crate::cms_common::ConvexityAdjustmentRiskCalculator::<crate::cms_swap::CmsSwap>(std::marker::PhantomData)),
        ]
    }

    Ok(())
}
