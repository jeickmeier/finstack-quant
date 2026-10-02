//! Variance vega metric (per 1 point change in variance).

use super::super::types::FxVarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate variance vega (sensitivity to 1 point change in variance).
pub(crate) struct VarianceVegaCalculator;

impl MetricCalculator for VarianceVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<FxVarianceSwap>()?;
        // Derivative with respect to one annualized variance unit of the
        // remaining return samples, holding fixed samples unchanged.
        let remaining_fraction = 1.0 - swap.realized_fraction_by_observations(context.as_of)?;
        if remaining_fraction == 0.0 {
            return Ok(0.0);
        }
        // Date-based discounting: `df_between_dates` resolves the year fraction
        // on the curve's own time axis, unlike `df()` fed an instrument
        // day-count year fraction.
        let disc = context
            .curves
            .get_discount(swap.domestic_discount_curve_id.as_str())?;
        let df = disc.df_between_dates(context.as_of, swap.effective_settlement_date()?)?;
        Ok(df * swap.notional.amount() * remaining_fraction * swap.side.sign())
    }
}
