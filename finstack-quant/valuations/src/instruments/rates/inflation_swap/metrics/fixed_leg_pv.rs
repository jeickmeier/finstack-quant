//! Fixed leg PV metric for `InflationSwap`.

use crate::instruments::rates::inflation_swap::InflationSwap;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates PV of fixed leg for inflation swaps.
pub(crate) struct FixedLegPvCalculator;

impl MetricCalculator for FixedLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let s: &InflationSwap = context.instrument_as()?;
        let pv_fixed = s.pv_fixed_leg(&context.curves, context.as_of)?;
        Ok(pv_fixed.amount())
    }
}
