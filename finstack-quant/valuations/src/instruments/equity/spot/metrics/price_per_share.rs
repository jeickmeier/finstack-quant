//! Equity metric: price per share.

use crate::instruments::equity::Equity;
use crate::metrics::{MetricCalculator, MetricContext};

/// Computes the price per share for an `Equity`.
pub(crate) struct PricePerShareCalculator;

impl MetricCalculator for PricePerShareCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        let m = equity.price_per_share(&context.curves, context.as_of)?;
        Ok(m.amount())
    }
}
