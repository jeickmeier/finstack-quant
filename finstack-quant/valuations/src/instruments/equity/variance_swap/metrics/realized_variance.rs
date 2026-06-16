//! Realized variance-to-date metric.

use super::super::types::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate the current realized variance to date.
pub(crate) struct RealizedVarianceCalculator;

impl MetricCalculator for RealizedVarianceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        // Use the instrument's own method which handles policy/market data consistently
        swap.partial_realized_variance(&context.curves, context.as_of)
    }
}
