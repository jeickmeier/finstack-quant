//! Available capacity metric for revolving credit facilities.

use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext};

use super::drawn_balance_as_of;

/// Calculator for available facility capacity (commitment - drawn).
///
/// Returns the amount as a float (in the instrument's currency units).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AvailableCapacityCalculator;

impl MetricCalculator for AvailableCapacityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let drawn = drawn_balance_as_of(facility, context.as_of)?;
        let available = facility
            .commitment_at(context.as_of)
            .checked_sub(drawn)?
            .checked_sub(facility.lc_outstanding_at(context.as_of))?;
        Ok(available.amount())
    }
}
