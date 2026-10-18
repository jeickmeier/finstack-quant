//! Strike volatility metric.

use super::super::types::VarianceSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculate strike in volatility terms.
pub(crate) struct StrikeVolCalculator;

impl MetricCalculator for StrikeVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        Ok(swap.strike_variance.sqrt())
    }
}
