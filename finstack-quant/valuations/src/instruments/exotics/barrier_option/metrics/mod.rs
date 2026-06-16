//! Barrier option metrics module.
//!
//! Provides full greek coverage for barrier options using finite difference methods.
//! Delta and Gamma use generic FD calculators.
//! Note: Barrier options exhibit discontinuous greeks near the barrier level.

// mod dv01; // removed - using GenericParallelDv01
// mod vanna; // removed - using GenericFdVanna
// mod volga; // removed - using GenericFdVolga

use crate::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register barrier option metrics with the registry.
pub(crate) fn register_barrier_option_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga};
    use crate::pricer::InstrumentType;

    // Use generic FD calculators for Delta, Gamma, Vanna, and Volga
    registry.register_metric(
        MetricId::Delta,
        Arc::new(GenericFdDelta::<crate::instruments::BarrierOption>::default()),
        &[InstrumentType::BarrierOption],
    );

    registry.register_metric(
        MetricId::Gamma,
        Arc::new(GenericFdGamma::<crate::instruments::BarrierOption>::default()),
        &[InstrumentType::BarrierOption],
    );

    registry.register_metric(
        MetricId::Vanna,
        Arc::new(GenericFdVanna::<crate::instruments::BarrierOption>::default()),
        &[InstrumentType::BarrierOption],
    );

    registry.register_metric(
        MetricId::Volga,
        Arc::new(GenericFdVolga::<crate::instruments::BarrierOption>::default()),
        &[InstrumentType::BarrierOption],
    );

    // Other metrics use custom implementations

    {
        crate::register_metrics! {
            registry: registry,
            instrument: InstrumentType::BarrierOption,
            metrics: [
                (Vega, crate::metrics::GenericFdVega::<crate::instruments::BarrierOption>::default()),
                (Rho, crate::metrics::UnifiedDv01Calculator::<
                    crate::instruments::BarrierOption,
                >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
                (Dv01, crate::metrics::UnifiedDv01Calculator::<
                    crate::instruments::BarrierOption,
                >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                    crate::instruments::BarrierOption,
                >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
                // Theta is now registered universally in metrics::standard_registry()
            ]
        }
    }
}
