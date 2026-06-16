//! Commodity swap metrics module.
//!
//! Provides metric calculators specific to `CommoditySwap`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_commodity_swap_metrics`.
//!
//! Exposed metrics:
//! - commodity delta (sensitivity to floating index)
//! - theta (time decay)
//! - DV01 (interest rate sensitivity)

mod delta;

use crate::metrics::MetricRegistry;

/// Register all CommoditySwap metrics with the registry.
pub(crate) fn register_commodity_swap_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommoditySwap,
        metrics: [
            (Delta, delta::DeltaCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::commodity::commodity_swap::CommoditySwap,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::commodity::commodity_swap::CommoditySwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
