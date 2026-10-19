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

mod constituent_delta;
mod weight_risk;

use crate::metrics::MetricRegistry;

use crate::instruments::exotics::basket::types::Basket;
use crate::metrics::MetricCalculator;
use crate::metrics::MetricContext;
pub(crate) use constituent_delta::ConstituentDeltaCalculator;
use finstack_quant_core::Result;
pub(crate) use weight_risk::WeightRiskCalculator;

/// Register all Basket metrics with the registry
pub(crate) fn register_basket_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Basket,
        metrics: [
            (ConstituentCount, ConstituentCountCalculator),
            (ExpenseRatio, ExpenseRatioCalculator),
            // Custom metrics for basket-specific risks
            (ConstituentDelta, ConstituentDeltaCalculator),
            (WeightRisk, WeightRiskCalculator),
        ]
    };
    Ok(())
}

/// Calculate number of constituents in the basket
///
/// Returns the number of constituents in the basket.
pub(super) struct ConstituentCountCalculator;

impl MetricCalculator for ConstituentCountCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket = context.instrument_as::<Basket>()?;
        Ok(basket.constituent_count() as f64)
    }
}

/// Calculate expense ratio as percentage
///
/// Returns the configured expense ratio as a percentage.
pub(super) struct ExpenseRatioCalculator;

impl MetricCalculator for ExpenseRatioCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket = context.instrument_as::<Basket>()?;
        Ok(basket.expense_ratio * 100.0)
    }
}
