//! Snowball / Inverse Floater metrics module.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register snowball metrics with the registry.
pub(crate) fn register_snowball_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        Dv01CalculatorConfig, MetricId, UnifiedDv01Calculator,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::Dv01,
        Arc::new(UnifiedDv01Calculator::new(
            Dv01CalculatorConfig::parallel_combined(),
        )),
        &[InstrumentType::Snowball],
    )?;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Snowball,
        metrics: [
            (BucketedDv01, UnifiedDv01Calculator::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
        ]
    }
    Ok(())
}
