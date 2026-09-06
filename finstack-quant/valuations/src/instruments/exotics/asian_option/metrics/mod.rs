//! Asian option metrics module.
//!
//! Provides full greek coverage for Asian options using finite difference methods.
//! Delta and Gamma use generic FD calculators.

use crate::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register Asian option metrics with the registry.
pub(crate) fn register_asian_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::metrics::{GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga};
    use crate::pricer::InstrumentType;

    registry.register_metric(
        MetricId::Delta,
        Arc::new(GenericFdDelta::<crate::instruments::AsianOption>::default()),
        &[InstrumentType::AsianOption],
    )?;

    registry.register_metric(
        MetricId::Gamma,
        Arc::new(GenericFdGamma::<crate::instruments::AsianOption>::default()),
        &[InstrumentType::AsianOption],
    )?;

    registry.register_metric(
        MetricId::Vanna,
        Arc::new(GenericFdVanna::<crate::instruments::AsianOption>::default()),
        &[InstrumentType::AsianOption],
    )?;

    registry.register_metric(
        MetricId::Volga,
        Arc::new(GenericFdVolga::<crate::instruments::AsianOption>::default()),
        &[InstrumentType::AsianOption],
    )?;

    {
        crate::register_metrics! {
            registry: registry,
            instrument: InstrumentType::AsianOption,
            metrics: [
                (Vega, crate::metrics::GenericFdVega::<crate::instruments::AsianOption>::default()),
                (Rho, crate::metrics::UnifiedDv01Calculator::<
                    crate::instruments::AsianOption,
                >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
                (Dv01, crate::metrics::UnifiedDv01Calculator::<
                    crate::instruments::AsianOption,
                >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                    crate::instruments::AsianOption,
                >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
                // Theta is now registered universally in metrics::standard_registry()
            ]
        }
    }
    Ok(())
}
