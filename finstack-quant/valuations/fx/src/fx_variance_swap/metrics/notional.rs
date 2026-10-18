//! Variance notional metric.

use super::super::types::FxVarianceSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculate variance notional.
pub(crate) struct VarianceNotionalCalculator;

impl MetricCalculator for VarianceNotionalCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<FxVarianceSwap>()?;
        Ok(swap.notional.amount())
    }
}
