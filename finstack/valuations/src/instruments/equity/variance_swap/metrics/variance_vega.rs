//! Variance vega metric (per 1 point change in variance).

use super::super::types::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Calculate variance vega (sensitivity to 1 point change in variance).
pub(crate) struct VarianceVegaCalculator;

impl MetricCalculator for VarianceVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        // Remaining fraction and discounting like vega. Uses the day-count
        // `time_elapsed_fraction` to stay consistent with `compute_pv` (W-32),
        // not an observation-count fraction.
        let remaining_fraction = 1.0 - swap.time_elapsed_fraction(context.as_of);
        let t = swap
            .day_count
            .year_fraction(context.as_of, swap.maturity, Default::default())?;
        let disc = context
            .curves
            .get_discount(swap.discount_curve_id.as_str())?;
        let df = disc.df(t);
        Ok(df * swap.notional.amount() * remaining_fraction * swap.side.sign())
    }
}
