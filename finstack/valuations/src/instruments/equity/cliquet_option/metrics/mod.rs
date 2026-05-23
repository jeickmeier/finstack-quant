//! Cliquet option metrics module.
//!
//! Provides full greek coverage for cliquet options using finite difference methods.
//! Delta and Gamma use generic FD calculators.

// mod vanna; // removed - using GenericFdVanna
// mod volga; // removed - using GenericFdVolga
mod rho;

use crate::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register cliquet option metrics with the registry.
pub(crate) fn register_cliquet_option_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga};
    use crate::pricer::InstrumentType;

    // Use generic FD calculators for Delta, Gamma, Vanna, and Volga
    registry.register_metric(
        MetricId::Delta,
        Arc::new(GenericFdDelta::<crate::instruments::CliquetOption>::default()),
        &[InstrumentType::CliquetOption],
    );

    registry.register_metric(
        MetricId::Gamma,
        Arc::new(GenericFdGamma::<crate::instruments::CliquetOption>::default()),
        &[InstrumentType::CliquetOption],
    );

    registry.register_metric(
        MetricId::Vanna,
        Arc::new(GenericFdVanna::<crate::instruments::CliquetOption>::default()),
        &[InstrumentType::CliquetOption],
    );

    registry.register_metric(
        MetricId::Volga,
        Arc::new(GenericFdVolga::<crate::instruments::CliquetOption>::default()),
        &[InstrumentType::CliquetOption],
    );

    // Other metrics use custom implementations

    {
        crate::register_metrics! {
            registry: registry,
            instrument: InstrumentType::CliquetOption,
            metrics: [
                (Vega, crate::metrics::GenericFdVega::<crate::instruments::CliquetOption>::default()),
                (Rho, rho::RhoCalculator),
                (Dv01, crate::metrics::UnifiedDv01Calculator::<
                    crate::instruments::CliquetOption,
                >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                    crate::instruments::CliquetOption,
                >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            ]
        }
    }
}
