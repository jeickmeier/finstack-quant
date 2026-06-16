//! TARN metrics module.
//!
//! Provides DV01 and bucketed DV01 for TARN instruments.
//! Full MC-based Greeks will be added when MC pricers are implemented.

use crate::metrics::MetricRegistry;

/// Register TARN metrics with the registry.
pub(crate) fn register_tarn_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{Dv01CalculatorConfig, MetricId, UnifiedDv01Calculator};
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::Dv01,
        Arc::new(UnifiedDv01Calculator::<super::Tarn>::new(
            Dv01CalculatorConfig::parallel_combined(),
        )),
        &[InstrumentType::Tarn],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Tarn,
        metrics: [
            (BucketedDv01, UnifiedDv01Calculator::<super::Tarn>::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
        ]
    }
}
