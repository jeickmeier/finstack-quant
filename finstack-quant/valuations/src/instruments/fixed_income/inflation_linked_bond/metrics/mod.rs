//! ILB metrics module.
//!
//! Splits ILB-specific metric calculators into focused files and
//! registers them with the shared metrics framework.

mod breakeven_inflation;
mod index_ratio;
mod inflation01;
mod inflation_convexity;
mod real_duration;
mod real_yield;

pub(crate) use breakeven_inflation::BreakevenInflationCalculator;
pub(crate) use index_ratio::IndexRatioCalculator;
pub(crate) use inflation01::Inflation01Calculator;
pub(crate) use inflation_convexity::InflationConvexityCalculator;
pub(crate) use real_duration::RealDurationCalculator;
pub(crate) use real_yield::RealYieldCalculator;

use crate::metrics::MetricId;
use crate::metrics::MetricRegistry;
use std::sync::Arc;

/// Register all ILB metrics with the registry
pub(crate) fn register_ilb_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    // Custom metric: Inflation01 (inflation curve sensitivity per 1bp)
    registry.register_metric(
        MetricId::Inflation01,
        Arc::new(Inflation01Calculator),
        &[InstrumentType::InflationLinkedBond],
    );

    // Custom metric: InflationConvexity (second-order inflation sensitivity)
    registry.register_metric(
        MetricId::InflationConvexity,
        Arc::new(InflationConvexityCalculator),
        &[InstrumentType::InflationLinkedBond],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::InflationLinkedBond,
        metrics: [
            (RealYield, RealYieldCalculator),
            (IndexRatio, IndexRatioCalculator),
            (RealDuration, RealDurationCalculator),
            (BreakevenInflation, BreakevenInflationCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::InflationLinkedBond,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            // Theta is now registered universally in metrics::standard_registry()
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::InflationLinkedBond,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
}
