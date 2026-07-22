//! Utilization rate metric for revolving credit facilities.

use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext};

use super::drawn_balance_as_of;

/// Calculator for facility utilization rate (drawn / committed).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct UtilizationRateCalculator;

impl MetricCalculator for UtilizationRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let drawn = drawn_balance_as_of(facility, context.as_of)?.amount();
        let commitment = facility.commitment_amount.amount();
        let utilization_rate = if commitment > 0.0 {
            drawn / commitment
        } else {
            0.0
        };
        Ok(utilization_rate)
    }
}
