//! Constituent count metric calculator.
//!
//! Returns the number of constituents in the basket.

use crate::instruments::exotics::basket::types::Basket;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate number of constituents in the basket
pub struct ConstituentCountCalculator;

impl MetricCalculator for ConstituentCountCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket = context.instrument_as::<Basket>()?;
        Ok(basket.constituent_count() as f64)
    }
}
