//! Vega metric for variance swaps (per 1% volatility move).

use super::super::types::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Calculate vega (sensitivity to 1% change in volatility).
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;

        // Use the instrument's forward variance logic to get a consistent current vol estimate
        // If calculation fails, fallback to strike volatility
        let current_vol = swap
            .remaining_forward_variance(&context.curves, context.as_of)
            .map(|v| v.sqrt())
            .unwrap_or_else(|_| swap.strike_variance.sqrt());

        // Remaining fraction of the variance accrual period. Must match the
        // day-count `time_elapsed_fraction` used by `compute_pv` (W-32) so vega
        // differentiates the same PV; an observation-count fraction diverges for
        // non-uniform (e.g. weekend-skipping) schedules.
        let remaining_fraction = 1.0 - swap.time_elapsed_fraction(context.as_of);

        // Discount factor to maturity
        let t = swap
            .day_count
            .year_fraction(context.as_of, swap.maturity, Default::default())?;
        let disc = context
            .curves
            .get_discount(swap.discount_curve_id.as_str())?;
        let df = disc.df(t);

        // Vega per 1% vol move: DF * 2 * Notional * sigma * 0.01 * remaining_fraction
        // Formula derivation: V = N * (sigma^2 - K^2) * DF
        // dV/dsigma = N * 2 * sigma * DF
        let vega = df * 2.0 * swap.notional.amount() * current_vol * 0.01 * remaining_fraction;
        Ok(vega * swap.side.sign())
    }
}
