//! InflationSwap metrics module.
//!
//! Provides metric calculators specific to `InflationSwap`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_inflation_swap_metrics`.
//!
//! Exposed metrics:
//! - Breakeven inflation
//! - Fixed leg PV
//! - Inflation leg PV
//! - DV01 (approximate)
//! - Inflation01 (approximate)

mod breakeven;
mod fixed_leg_pv;
mod inflation01;
mod inflation_convexity;
mod inflation_leg_pv;
mod par_rate;
mod yoy_inflation01;

use crate::metrics::MetricRegistry;

/// Register all inflation swap metrics with the registry
pub(crate) fn register_inflation_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Custom metrics
    for (id, calculator) in [
        (
            MetricId::custom("breakeven"),
            Arc::new(breakeven::BreakevenCalculator) as Arc<dyn crate::metrics::MetricCalculator>,
        ),
        (
            MetricId::custom("fixed_leg_pv"),
            Arc::new(fixed_leg_pv::FixedLegPvCalculator),
        ),
        (
            MetricId::custom("inflation_leg_pv"),
            Arc::new(inflation_leg_pv::InflationLegPvCalculator),
        ),
        (
            MetricId::Inflation01,
            Arc::new(inflation01::Inflation01Calculator),
        ),
        (
            MetricId::InflationConvexity,
            Arc::new(inflation_convexity::InflationConvexityCalculator),
        ),
    ] {
        registry.register_metric(id, calculator, &[InstrumentType::InflationSwap])?;
    }
    // Note: `Npv01` is intentionally NOT registered — it was an exact
    // duplicate of `Dv01` (same `parallel_combined` config). Use `Dv01`.

    registry.register_metric(
        MetricId::Inflation01,
        Arc::new(yoy_inflation01::YoYInflation01Calculator),
        &[InstrumentType::YoYInflationSwap],
    )?;

    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::InflationSwap,
        metrics: [
            (ParRate, par_rate::ParRateCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::InflationSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::InflationSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::YoYInflationSwap,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::YoYInflationSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::YoYInflationSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
