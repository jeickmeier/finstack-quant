//! Realized variance-to-date metric.

use super::super::types::FxVarianceSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculate the current realized variance to date.
pub(crate) struct RealizedVarianceCalculator;

impl MetricCalculator for RealizedVarianceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<FxVarianceSwap>()?;
        swap.partial_realized_variance(&context.curves, context.as_of)
    }
}
