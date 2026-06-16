//! Declarative macro for simplifying metric registration.
//!
//! This module provides a macro to reduce boilerplate in instrument metric registration.

/// Simplifies metric registration by providing a declarative syntax.
///
/// See unit tests and `examples/` for usage.
#[macro_export]
macro_rules! register_metrics {
    (
        registry: $registry:expr,
        instrument: $instrument:expr,
        metrics: [
            $(($metric_id:ident, $calculator:expr)),* $(,)?
        ]
    ) => {{
        use $crate::metrics::MetricId;
        use std::sync::Arc;

        $(
            $registry.register_metric(
                MetricId::$metric_id,
                Arc::new($calculator),
                &[$instrument],
            );
        )*
    }};
}

#[cfg(test)]
mod tests {
    use crate::metrics::{MetricCalculator, MetricContext, MetricId, MetricRegistry};
    use crate::pricer::InstrumentType;
    use finstack_quant_core::Result;

    struct DummyCalculator;
    impl MetricCalculator for DummyCalculator {
        fn calculate(&self, _context: &mut MetricContext) -> Result<f64> {
            Ok(42.0)
        }
    }

    #[test]
    fn test_register_metrics_macro() {
        let mut registry = MetricRegistry::new();

        register_metrics! {
            registry: registry,
            instrument: InstrumentType::Bond,
            metrics: [
                (Accrued, DummyCalculator),
                (Ytm, DummyCalculator),
            ]
        }

        // Verify metrics were registered (basic smoke test)
        assert!(registry.is_applicable(&MetricId::Accrued, InstrumentType::Bond));
        assert!(registry.is_applicable(&MetricId::Ytm, InstrumentType::Bond));
    }
}
