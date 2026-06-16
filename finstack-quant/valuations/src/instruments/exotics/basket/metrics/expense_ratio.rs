//! Expense ratio metric calculator.
//!
//! Returns the configured expense ratio as a percentage.

use crate::instruments::exotics::basket::types::Basket;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate expense ratio as percentage
pub struct ExpenseRatioCalculator;

impl MetricCalculator for ExpenseRatioCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket = context.instrument_as::<Basket>()?;
        Ok(basket.expense_ratio * 100.0)
    }
}
