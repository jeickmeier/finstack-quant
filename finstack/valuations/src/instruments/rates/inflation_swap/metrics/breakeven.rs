//! Breakeven inflation metric for `InflationSwap`.

use crate::instruments::rates::inflation_swap::InflationSwap;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates breakeven inflation rate for inflation swaps.
///
/// Computes the fixed rate that makes the swap's present value zero.
/// Formula: K_BE = (E[I(T_mat)]/I(T_start))^(1/τ) - 1
pub(crate) struct BreakevenCalculator;

impl MetricCalculator for BreakevenCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let s: &InflationSwap = context.instrument_as()?;
        s.par_rate(context.curves.as_ref(), context.as_of)
    }
}
