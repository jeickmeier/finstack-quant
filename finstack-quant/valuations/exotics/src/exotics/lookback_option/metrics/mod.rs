//! Lookback option metrics module.
//!
//! Provides full greek coverage for lookback options using finite difference methods.
//! Delta and Gamma use generic FD calculators.

mod rho;

use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register lookback option metrics with the registry.
pub(crate) fn register_lookback_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    registry.register_metric(
        MetricId::Delta,
        Arc::new(GenericFdDelta::<crate::LookbackOption>::default()),
        &[InstrumentType::LookbackOption],
    )?;

    registry.register_metric(
        MetricId::Gamma,
        Arc::new(GenericFdGamma::<crate::LookbackOption>::default()),
        &[InstrumentType::LookbackOption],
    )?;

    registry.register_metric(
        MetricId::Vanna,
        Arc::new(GenericFdVanna::<crate::LookbackOption>::default()),
        &[InstrumentType::LookbackOption],
    )?;

    registry.register_metric(
        MetricId::Volga,
        Arc::new(GenericFdVolga::<crate::LookbackOption>::default()),
        &[InstrumentType::LookbackOption],
    )?;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::LookbackOption,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::GenericFdVega::<crate::exotics::lookback_option::LookbackOption>::default()),
                (Rho, rho::RhoCalculator),
                (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
                // Theta is now registered universally in crate::exotics::EXOTICS.metrics()
            ]
        }
    }
    Ok(())
}
