//! Vega from a parallel implied-volatility bump through the booked pricer.

use super::super::types::FxVarianceSwap;
use crate::metrics::{GenericFdVega, MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Cash sensitivity per 0.01 absolute implied-volatility move.
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<FxVarianceSwap>()?;
        if context.as_of >= swap.final_observation_date()? {
            return Ok(0.0);
        }
        GenericFdVega::<FxVarianceSwap>::default().calculate(context)
    }
}
