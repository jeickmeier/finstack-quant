//! CMS swap metrics module.
//!
//! Provides risk metrics for CMS swaps:
//! - **DV01**: Parallel rate sensitivity (unified calculator)
//! - **BucketedDv01**: Key-rate sensitivities
//! - **ConvexityAdjustmentRisk**: Dollar value of the convexity adjustment

use crate::metrics::MetricRegistry;
use std::sync::Arc;

/// Register CMS swap metrics with the registry.
pub(crate) fn register_cms_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CmsSwap,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::rates::cms_swap::CmsSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::rates::cms_swap::CmsSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }

    registry.register_metric(
        MetricId::ConvexityAdjustmentRisk,
        Arc::new(
            crate::instruments::rates::cms_common::ConvexityAdjustmentRiskCalculator::<
                crate::instruments::rates::cms_swap::CmsSwap,
            >(std::marker::PhantomData),
        ),
        &[InstrumentType::CmsSwap],
    )?;
    Ok(())
}
