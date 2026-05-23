//! Basket metrics module.
//!
//! Provides metric calculators specific to `Basket`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_basket_metrics`.
//!
//! Exposed metrics:
//! - Constituent count
//! - Expense ratio (percentage)
//! - Asset exposure by `AssetType`
//!
//! Note: Present value is handled by the instrument's built-in value() method.

mod asset_exposure;
mod constituent_count;
mod constituent_delta;
mod expense_ratio;
mod weight_risk;

use crate::metrics::MetricId;
use crate::metrics::MetricRegistry;
use std::sync::Arc;

pub use asset_exposure::AssetExposureCalculator;
pub use constituent_count::ConstituentCountCalculator;
pub(crate) use constituent_delta::ConstituentDeltaCalculator;
pub use expense_ratio::ExpenseRatioCalculator;
pub(crate) use weight_risk::WeightRiskCalculator;

/// Register all Basket metrics with the registry
pub(crate) fn register_basket_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    // Custom metrics for basket-specific risks
    registry.register_metric(
        MetricId::ConstituentDelta,
        Arc::new(ConstituentDeltaCalculator),
        &[InstrumentType::Basket],
    );
    registry.register_metric(
        MetricId::custom("weight_risk"),
        Arc::new(WeightRiskCalculator),
        &[InstrumentType::Basket],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Basket,
        metrics: [
            (ConstituentCount, ConstituentCountCalculator),
            (ExpenseRatio, ExpenseRatioCalculator),
        ]
    };
}
