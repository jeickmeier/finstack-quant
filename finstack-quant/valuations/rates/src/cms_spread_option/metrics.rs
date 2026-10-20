//! CMS Spread Option metrics module.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register CMS spread option metrics with the registry.
pub(crate) fn register_cms_spread_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::dv01::{
        Dv01CalculatorConfig, UnifiedDv01Calculator,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CmsSpreadOption,
        metrics: [
            (BucketedDv01, UnifiedDv01Calculator::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
            (Dv01, UnifiedDv01Calculator::new(Dv01CalculatorConfig::parallel_combined())),
        ]
    }
    Ok(())
}
