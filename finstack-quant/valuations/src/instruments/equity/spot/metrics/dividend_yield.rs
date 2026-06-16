//! Equity metric: dividend yield (annualized, decimal).

use crate::instruments::equity::Equity;
use crate::metrics::{MetricCalculator, MetricContext};

/// Computes the dividend yield using `{ticker}-DIVYIELD` if present, or 0.0.
pub(crate) struct DividendYieldCalculator;

impl MetricCalculator for DividendYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        equity.dividend_yield(&context.curves)
    }
}
