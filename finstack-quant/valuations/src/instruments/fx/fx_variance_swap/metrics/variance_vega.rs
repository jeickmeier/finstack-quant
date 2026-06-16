//! Variance vega metric (per 1 point change in variance).

use super::super::types::FxVarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate variance vega (sensitivity to 1 point change in variance).
pub(crate) struct VarianceVegaCalculator;

impl MetricCalculator for VarianceVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<FxVarianceSwap>()?;
        // Only the un-seasoned forward variance carries variance sensitivity.
        // Weight by the day-count `time_elapsed_fraction` to match the pricer's
        // seasoned-MTM time-weighting (observation-count fractions drift for
        // weekend-skipping daily schedules).
        let remaining_fraction = 1.0 - swap.time_elapsed_fraction(context.as_of)?;
        // Date-based discounting: `df_between_dates` resolves the year fraction
        // on the curve's own time axis, unlike `df()` fed an instrument
        // day-count year fraction.
        let disc = context
            .curves
            .get_discount(swap.domestic_discount_curve_id.as_str())?;
        let df = disc.df_between_dates(context.as_of, swap.maturity)?;
        Ok(df * swap.notional.amount() * remaining_fraction * swap.side.sign())
    }
}
