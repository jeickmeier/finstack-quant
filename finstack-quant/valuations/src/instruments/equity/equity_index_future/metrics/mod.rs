//! Equity index future metrics module.
//!
//! Provides metric calculators specific to `EquityIndexFuture`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_equity_index_future_metrics`.
//!
//! Exposed metrics:
//! - DV01 (interest rate sensitivity)
//! - Bucketed DV01 (key rate sensitivity)
//! - Theta (time decay)

mod delta;
mod pricing;

use crate::metrics::MetricRegistry;

/// Register all EquityIndexFuture metrics with the registry.
pub(crate) fn register_equity_index_future_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::EquityIndexFuture,
        metrics: [
            (Delta, delta::DeltaCalculator),
            (FuturesPrice, pricing::FuturesPriceCalculator),
            (Basis, pricing::BasisCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::equity::equity_index_future::EquityIndexFuture,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::equity::equity_index_future::EquityIndexFuture,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;

    #[test]
    fn registers_futures_price_and_basis_metrics() {
        let mut registry = MetricRegistry::new();
        register_equity_index_future_metrics(&mut registry);
        let metrics = registry.metrics_for_instrument(InstrumentType::EquityIndexFuture);

        assert!(metrics.contains(&MetricId::FuturesPrice));
        assert!(metrics.contains(&MetricId::Basis));
    }
}
