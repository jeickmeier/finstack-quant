//! CMS Spread Option metrics module.

use crate::metrics::MetricRegistry;

/// Register CMS spread option metrics with the registry.
pub(crate) fn register_cms_spread_option_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{Dv01CalculatorConfig, MetricId, UnifiedDv01Calculator};
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::Dv01,
        Arc::new(UnifiedDv01Calculator::<super::CmsSpreadOption>::new(
            Dv01CalculatorConfig::parallel_combined(),
        )),
        &[InstrumentType::CmsSpreadOption],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CmsSpreadOption,
        metrics: [
            (BucketedDv01, UnifiedDv01Calculator::<super::CmsSpreadOption>::new(
                Dv01CalculatorConfig::triangular_key_rate(),
            )),
        ]
    }
}
