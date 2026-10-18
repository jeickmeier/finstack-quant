//! Asian option metrics module.
//!
//! Provides full greek coverage for Asian options using finite difference methods.
//! Delta and Gamma use generic FD calculators.

use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register Asian option metrics with the registry.
pub(crate) fn register_asian_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    registry.register_metric(
        MetricId::Delta,
        Arc::new(GenericFdDelta::<crate::AsianOption>::default()),
        &[InstrumentType::AsianOption],
    )?;

    registry.register_metric(
        MetricId::Gamma,
        Arc::new(GenericFdGamma::<crate::AsianOption>::default()),
        &[InstrumentType::AsianOption],
    )?;

    registry.register_metric(
        MetricId::Vanna,
        Arc::new(GenericFdVanna::<crate::AsianOption>::default()),
        &[InstrumentType::AsianOption],
    )?;

    registry.register_metric(
        MetricId::Volga,
        Arc::new(GenericFdVolga::<crate::AsianOption>::default()),
        &[InstrumentType::AsianOption],
    )?;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::AsianOption,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::GenericFdVega::<crate::AsianOption>::default()),
                (Rho, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
                (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
                // Theta is now registered universally in crate::exotics::EXOTICS.metrics()
            ]
        }
    }
    Ok(())
}
