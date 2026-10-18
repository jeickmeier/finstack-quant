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

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all FxForward metrics with the registry.
pub(crate) fn register_fx_forward_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::MetricId;
    use finstack_quant_valuations_core::pricer::InstrumentType;

    registry.register_metric(
        MetricId::Fx01,
        finstack_quant_valuations_core::metrics::sensitivities::fx01::arc_generic_fx01(),
        &[InstrumentType::FxForward],
    )?;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxForward,
        metrics: [
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
