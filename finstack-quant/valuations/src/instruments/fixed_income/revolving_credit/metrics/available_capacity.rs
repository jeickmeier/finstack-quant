//! Available capacity metric for revolving credit facilities.

use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculator for available facility capacity (commitment - drawn).
///
/// Returns the amount as a float (in the instrument's currency units).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AvailableCapacityCalculator;

impl MetricCalculator for AvailableCapacityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let available = facility.undrawn_amount()?;
        Ok(available.amount())
    }
}
