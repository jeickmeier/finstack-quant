//! FRA-specific metrics module.
//!
//! Provides metric calculators for FRAs, split into focused files for clarity
//! and parity with other instruments. Metrics include:
//! - DV01 (parallel rate sensitivity via generic calculator)
//! - Par rate
//! - Bucketed DV01 (key-rate sensitivity)
//!
//! Note: PV is available in `ValuationResult.value`, not as a metric in measures.
//!
//! See unit tests and `examples/` for usage.

mod dv01;
mod par_rate;

pub(crate) use dv01::FraRateCurveDv01Calculator;
use finstack_quant_valuations_core::metrics::MetricRegistry;
pub(crate) use par_rate::FraParRateCalculator;

/// Registers all FRA metrics to a registry.
///
/// Each metric is registered with the "FRA" instrument type to ensure
/// proper applicability filtering.
pub(crate) fn register_fra_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Fra,
        metrics: [
            (Dv01, FraRateCurveDv01Calculator),
            (Pv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_forward_only())),
            (ForwardPv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_forward_only())),
            (ParRate, FraParRateCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
