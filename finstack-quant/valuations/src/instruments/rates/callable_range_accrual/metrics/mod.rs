//! Callable Range Accrual metrics module.

use crate::metrics::MetricRegistry;

/// Register callable range accrual metrics with the registry.
pub(crate) fn register_callable_range_accrual_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{Dv01CalculatorConfig, MetricId, UnifiedDv01Calculator};
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::Dv01,
        Arc::new(UnifiedDv01Calculator::<super::CallableRangeAccrual>::new(
            Dv01CalculatorConfig::parallel_combined(),
        )),
        &[InstrumentType::CallableRangeAccrual],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CallableRangeAccrual,
        metrics: [
            (BucketedDv01, UnifiedDv01Calculator::<super::CallableRangeAccrual>::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
        ]
    }
}
