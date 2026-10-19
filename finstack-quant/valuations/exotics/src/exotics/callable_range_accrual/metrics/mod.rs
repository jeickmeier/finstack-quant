//! Callable Range Accrual metrics module.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register callable range accrual metrics with the registry.
pub(crate) fn register_callable_range_accrual_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{Dv01CalculatorConfig, UnifiedDv01Calculator};
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CallableRangeAccrual,
        metrics: [
            (BucketedDv01, UnifiedDv01Calculator::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
            (Dv01, UnifiedDv01Calculator::new(Dv01CalculatorConfig::parallel_combined())),
        ]
    }
    Ok(())
}
