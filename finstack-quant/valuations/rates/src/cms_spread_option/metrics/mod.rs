//! CMS Spread Option metrics module.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register CMS spread option metrics with the registry.
pub(crate) fn register_cms_spread_option_metrics(
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
        &[InstrumentType::CmsSpreadOption],
    )?;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CmsSpreadOption,
        metrics: [
            (BucketedDv01, UnifiedDv01Calculator::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
        ]
    }
    Ok(())
}
