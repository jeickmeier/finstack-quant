//! FX forward metrics module.
//!
//! Provides metric calculators specific to `FxForward`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_fx_forward_metrics`.
//!
//! Exposed metrics:
//! - DV01 (interest rate sensitivity for domestic and foreign curves)
//! - FX01 (sensitivity to a 1% relative spot move) — uses the shared
//!   `metrics::sensitivities::fx01::GenericFx01Calculator`. The custom
//!   per-instrument calculator that used to live here was removed in favor of
//!   the generic one (which works for every instrument that publishes its FX
//!   pair via `MarketDependencies::fx_pairs`).

use crate::metrics::MetricRegistry;

/// Register all FxForward metrics with the registry.
pub(crate) fn register_fx_forward_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;

    registry.register_metric(
        MetricId::Fx01,
        crate::metrics::sensitivities::fx01::arc_generic_fx01(),
        &[InstrumentType::FxForward],
    );
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxForward,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fx::fx_forward::FxForward,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fx::fx_forward::FxForward,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
