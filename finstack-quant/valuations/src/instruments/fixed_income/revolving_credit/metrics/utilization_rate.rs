//! Utilization rate metric for revolving credit facilities.

use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculator for facility utilization rate (drawn / committed).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct UtilizationRateCalculator;

impl MetricCalculator for UtilizationRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let utilization_rate = facility.utilization_rate();
        Ok(utilization_rate)
    }
}
