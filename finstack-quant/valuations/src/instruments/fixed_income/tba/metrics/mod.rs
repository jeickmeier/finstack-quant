//! Agency TBA risk metrics.
//!
//! TBA-specific metrics inherit from MBS metrics but use assumed pool
//! characteristics for calculation.

use crate::metrics::MetricRegistry;

/// Register TBA metrics with the registry.
pub(crate) fn register_tba_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::AgencyTba,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::AgencyTba,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::AgencyTba,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
