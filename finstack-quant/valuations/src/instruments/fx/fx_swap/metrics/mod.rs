//! FX Swap metrics module.
//!
//! Provides metric calculators specific to `FxSwap`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_fx_swap_metrics`.
//!
//! Exposed metrics:
//! - Forward points (far rate - near rate)
//! - FX01 (sensitivity to a 1% relative spot move) — shared
//!   `GenericFx01Calculator`
//! - DV01 (domestic) and DV01 (foreign)

mod carry_pv;
mod forward_points;
mod ir01_domestic;
mod ir01_foreign;

use crate::metrics::MetricRegistry;

/// Register all FX Swap metrics with the registry
pub(crate) fn register_fx_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Custom metrics
    for (id, calculator) in [
        (
            MetricId::custom("carry_pv"),
            Arc::new(carry_pv::CarryPv) as Arc<dyn crate::metrics::MetricCalculator>,
        ),
        (
            MetricId::custom("forward_points"),
            Arc::new(forward_points::ForwardPoints),
        ),
        (
            MetricId::Fx01,
            crate::metrics::sensitivities::fx01::arc_generic_fx01(),
        ),
        (
            MetricId::FxDelta,
            crate::metrics::sensitivities::fx01::arc_generic_fx01(),
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
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxSwap,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
