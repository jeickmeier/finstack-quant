//! Par rate metric for `InflationSwap`.

use crate::instruments::rates::inflation_swap::InflationSwap;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates the par real rate for an inflation swap.
pub(crate) struct ParRateCalculator;

impl MetricCalculator for ParRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let swap: &InflationSwap = context.instrument_as()?;
        swap.par_rate(context.curves.as_ref(), context.as_of)
    }
}
