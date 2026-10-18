//! Cliquet option metrics module.
//!
//! Provides full greek coverage for cliquet options using finite difference methods.
//! Delta and Gamma use generic FD calculators.

mod rho;

use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register cliquet option metrics with the registry.
pub(crate) fn register_cliquet_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    registry.register_metric(
        MetricId::Delta,
        Arc::new(GenericFdDelta::<crate::CliquetOption>::default()),
        &[InstrumentType::CliquetOption],
    )?;

    registry.register_metric(
        MetricId::Gamma,
        Arc::new(GenericFdGamma::<crate::CliquetOption>::default()),
        &[InstrumentType::CliquetOption],
    )?;

    registry.register_metric(
        MetricId::Vanna,
        Arc::new(GenericFdVanna::<crate::CliquetOption>::default()),
        &[InstrumentType::CliquetOption],
    )?;

    registry.register_metric(
        MetricId::Volga,
        Arc::new(GenericFdVolga::<crate::CliquetOption>::default()),
        &[InstrumentType::CliquetOption],
    )?;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::CliquetOption,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::GenericFdVega::<crate::CliquetOption>::default()),
                (Rho, rho::RhoCalculator),
                (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            ]
        }
    }
    Ok(())
}
