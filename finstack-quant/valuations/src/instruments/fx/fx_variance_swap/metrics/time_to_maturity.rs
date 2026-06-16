//! Time-to-maturity metric.

use super::super::types::FxVarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate time to maturity in years.
pub(crate) struct TimeToMaturityCalculator;

impl MetricCalculator for TimeToMaturityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<FxVarianceSwap>()?;
        let as_of = context.as_of;

        if as_of >= swap.maturity {
            return Ok(0.0);
        }

        swap.day_count
            .year_fraction(as_of, swap.maturity, Default::default())
    }
}
