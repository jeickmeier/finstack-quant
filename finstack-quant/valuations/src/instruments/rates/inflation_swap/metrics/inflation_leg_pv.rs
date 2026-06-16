//! Inflation leg PV metric for `InflationSwap`.

use crate::instruments::rates::inflation_swap::InflationSwap;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates PV of inflation leg for inflation swaps.
pub(crate) struct InflationLegPvCalculator;

impl MetricCalculator for InflationLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let s: &InflationSwap = context.instrument_as()?;
        let pv_inflation = s.pv_inflation_leg(&context.curves, context.as_of)?;
        Ok(pv_inflation.amount())
    }
}
