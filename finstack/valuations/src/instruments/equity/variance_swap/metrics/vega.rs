//! Vega metric for variance swaps (per 1% volatility move).

use super::super::types::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Calculate vega (sensitivity to 1% change in volatility).
///
/// # Vol basis reconciliation (W-34)
///
/// Variance-swap vega and [`super::variance_vega::VarianceVegaCalculator`]
/// must satisfy the chain-rule identity
///
/// ```text
/// vega ≈ variance_vega · 2σ · 0.01
/// ```
///
/// because `∂PV/∂σ = ∂PV/∂V · ∂V/∂σ` with `∂V/∂σ = 2σ`. For this identity to
/// hold with a *single, well-defined* `σ`, both metrics must reference the same
/// volatility base.
///
/// This calculator anchors `σ` to the **strike volatility**
/// `σ_K = √(strike_variance)`. That is the codebase's own vega-notional
/// convention: [`VarianceSwap::vega_to_variance_notional`] and
/// [`VarianceSwap::variance_to_vega_notional`] both use the strike vol
/// (`vega_notional = 2·σ_K·variance_notional`). Anchoring the vega metric to
/// the same `σ_K` makes the whole vega / variance-vega / notional family
/// internally consistent.
///
/// Previously `σ` was the *forward* vol `√(remaining_forward_variance)`, a
/// different base from the strike-vol notional machinery, so the identity
/// `vega = variance_vega · 2σ · 0.01` did not close for any well-defined `σ`.
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;

        // Strike volatility — the vega-notional convention's vol base. This is
        // the same σ that `vega_to_variance_notional` uses, and the same σ for
        // which `vega = variance_vega · 2σ · 0.01` closes exactly.
        let strike_vol = swap.strike_variance.max(0.0).sqrt();

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

        // Vega per 1% vol move: DF * 2 * Notional * σ_K * 0.01 * remaining_fraction.
        // Derivation: PV = N * (σ² - K²) * DF * remaining_fraction
        //             ∂PV/∂σ = N * 2σ * DF * remaining_fraction
        // Evaluated at σ = σ_K it equals `variance_vega · 2·σ_K · 0.01`.
        let vega = df * 2.0 * swap.notional.amount() * strike_vol * 0.01 * remaining_fraction;
        Ok(vega * swap.side.sign())
    }
}
