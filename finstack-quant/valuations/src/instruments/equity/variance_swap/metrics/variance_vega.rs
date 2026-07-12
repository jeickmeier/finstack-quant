//! Variance vega metric (per 1 point change in variance).

use super::super::types::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate variance vega (sensitivity to 1 point change in variance).
///
/// This is the clean `∂PV/∂V` of the forward-variance leg:
/// `variance_vega = DF · variance_notional · remaining_fraction · side`.
///
/// It satisfies the chain-rule identity with
/// [`super::vega::VegaCalculator`] (W-34):
///
/// ```text
/// vega = variance_vega · 2·σ_K · 0.01
/// ```
///
/// where `σ` is the current remaining forward volatility used by the PV vega.
/// Strike volatility remains the basis for quoted-notional conversion
/// ([`VarianceSwap::vega_to_variance_notional`]).
pub(crate) struct VarianceVegaCalculator;

impl MetricCalculator for VarianceVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;
        // Remaining fraction and discounting like vega. Uses the day-count
        // `time_elapsed_fraction` to stay consistent with `compute_pv` (W-32),
        // not an observation-count fraction.
        let remaining_fraction = 1.0 - swap.time_elapsed_fraction(context.as_of);
        let disc = context
            .curves
            .get_discount(swap.discount_curve_id.as_str())?;
        let df = disc.df_between_dates(context.as_of, swap.effective_settlement_date()?)?;
        Ok(df * swap.notional.amount() * remaining_fraction * swap.side.sign())
    }
}
