//! Snowball / Inverse Floater metrics module.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register snowball metrics with the registry.
pub(crate) fn register_snowball_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::dv01::{
        Dv01CalculatorConfig, UnifiedDv01Calculator,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Snowball,
        metrics: [
            (BucketedDv01, UnifiedDv01Calculator::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
            (Dv01, UnifiedDv01Calculator::new(Dv01CalculatorConfig::parallel_combined())),
        ]
    }
    Ok(())
}
