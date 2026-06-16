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

use crate::metrics::MetricRegistry;
pub(crate) use dv01::FraRateCurveDv01Calculator;
pub(crate) use par_rate::FraParRateCalculator;

/// Registers all FRA metrics to a registry.
///
/// Each metric is registered with the "FRA" instrument type to ensure
/// proper applicability filtering.
pub(crate) fn register_fra_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FRA,
        metrics: [
            (Dv01, FraRateCurveDv01Calculator),
            (Pv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::ForwardRateAgreement,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_forward_only())),
            (ForwardPv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::ForwardRateAgreement,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_forward_only())),
            (ParRate, FraParRateCalculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::ForwardRateAgreement,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
