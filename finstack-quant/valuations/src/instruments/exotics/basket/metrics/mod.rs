//! Basket metrics module.
//!
//! Provides metric calculators specific to `Basket`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_basket_metrics`.
//!
//! Exposed metrics:
//! - Constituent count
//! - Expense ratio (percentage)
//!
//! Note: Present value is handled by the instrument's built-in value() method.

mod constituent_count;
mod constituent_delta;
mod expense_ratio;
mod weight_risk;

use crate::metrics::MetricId;
use crate::metrics::MetricRegistry;
use std::sync::Arc;

use constituent_count::ConstituentCountCalculator;
pub(crate) use constituent_delta::ConstituentDeltaCalculator;
use expense_ratio::ExpenseRatioCalculator;
pub(crate) use weight_risk::WeightRiskCalculator;

/// Register all Basket metrics with the registry
pub(crate) fn register_basket_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;
    // Custom metrics for basket-specific risks
    registry.register_metric(
        MetricId::ConstituentDelta,
        Arc::new(ConstituentDeltaCalculator),
        &[InstrumentType::Basket],
    )?;
    registry.register_metric(
        MetricId::WeightRisk,
        Arc::new(WeightRiskCalculator),
        &[InstrumentType::Basket],
    )?;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Basket,
        metrics: [
            (ConstituentCount, ConstituentCountCalculator),
            (ExpenseRatio, ExpenseRatioCalculator),
        ]
    };
    Ok(())
}
