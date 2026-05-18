//! Vega metric for FX variance swaps (per 1% volatility move).

use super::super::types::FxVarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Calculate vega (sensitivity to 1% change in volatility).
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<FxVarianceSwap>()?;

        let current_vol = swap
            .remaining_forward_variance(&context.curves, context.as_of)
            .map(|v| v.sqrt())
            .unwrap_or_else(|_| swap.strike_variance.sqrt());

        // Only the un-seasoned forward variance carries vol sensitivity. Weight
        // by the day-count `time_elapsed_fraction` so the metric is consistent
        // with the pricer's seasoned-MTM time-weighting (an observation-count
        // fraction drifts for weekend-skipping daily schedules).
        let remaining_fraction = 1.0 - swap.time_elapsed_fraction(context.as_of)?;

        // Date-based discounting: `df_between_dates` resolves the year fraction
        // on the curve's own time axis. Passing an instrument-day-count year
        // fraction into `df()` (which expects the curve axis) is incorrect when
        // the day-counts differ or `as_of != base_date`.
        let disc = context
            .curves
            .get_discount(swap.domestic_discount_curve_id.as_str())?;
        let df = disc.df_between_dates(context.as_of, swap.maturity)?;

        let vega = df * 2.0 * swap.notional.amount() * current_vol * 0.01 * remaining_fraction;
        Ok(vega * swap.side.sign())
    }
}
