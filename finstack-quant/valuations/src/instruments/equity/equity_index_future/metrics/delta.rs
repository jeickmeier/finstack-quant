//! Delta calculator for equity index futures.

use crate::instruments::equity::equity_index_future::EquityIndexFuture;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Delta calculator for equity index futures.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &EquityIndexFuture = context.instrument_as()?;
        Ok(future.delta())
    }
}
