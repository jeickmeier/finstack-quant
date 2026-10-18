//! Declarative macro for simplifying metric registration.
//!
//! This module provides a macro to reduce boilerplate in instrument metric registration.

/// Simplifies instrument-specific metric registration by providing a declarative syntax.
///
/// Each `(metric, instrument)` pair has one owner. Checked registration rejects
/// accidental duplicates while allowing a specialized calculator alongside the
/// registry's universal default. Intentional caller overrides use
/// [`MetricRegistry::replace_metric`](crate::metrics::MetricRegistry::replace_metric).
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
            )?;
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
    fn macro_rejects_duplicate_instrument_owner() {
        fn register(
            registry: &mut MetricRegistry,
        ) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
            register_metrics! {
                registry: registry,
                instrument: InstrumentType::Bond,
                metrics: [(Accrued, DummyCalculator)]
            }
            Ok(())
        }

        let mut registry = MetricRegistry::new();
        register(&mut registry).expect("first owner");
        assert!(matches!(
            register(&mut registry),
            Err(crate::metrics::MetricRegistryError::DuplicateRegistration {
                metric,
                instrument: Some(InstrumentType::Bond),
            }) if metric == MetricId::Accrued
        ));
    }

    #[test]
    fn test_register_metrics_macro() -> std::result::Result<(), crate::metrics::MetricRegistryError>
    {
        let mut registry = MetricRegistry::new();

        register_metrics! {
            registry: registry,
            instrument: InstrumentType::Bond,
            metrics: [
                (Accrued, DummyCalculator),
                (Ytm, DummyCalculator),
            ]
        }

        assert!(registry.is_applicable(&MetricId::Accrued, InstrumentType::Bond));
        assert!(registry.is_applicable(&MetricId::Ytm, InstrumentType::Bond));
        Ok(())
    }
}
