//! Vega metric for variance swaps (per 1% volatility move).

use super::super::types::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

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
/// This calculator uses the current remaining forward volatility for PV
/// sensitivity; strike volatility remains reserved for notional conversion.
/// The separate [`VarianceSwap::vega_to_variance_notional`] helpers continue to
/// use strike volatility because they convert a quoted notional, not a market
/// PV sensitivity.
///
/// The chain-rule identity therefore uses the forward volatility on the PV
/// axis, while notional conversion uses the strike volatility by design.
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<VarianceSwap>()?;

        // Remaining fraction of the variance accrual period. Must match the
        // day-count `time_elapsed_fraction` used by `compute_pv` (W-32) so vega
        // differentiates the same PV; an observation-count fraction diverges for
        // non-uniform (e.g. weekend-skipping) schedules.
        let remaining_fraction = 1.0 - swap.time_elapsed_fraction(context.as_of);
        if remaining_fraction <= 0.0 {
            return Ok(0.0);
        }

        // Vega is the sensitivity of the actual forward-variance leg.  The
        // strike volatility is appropriate for converting a quoted vega
        // notional, but it is not the derivative of PV when the market
        // forward variance differs from strike variance.
        let forward_variance = super::super::pricer::remaining_forward_variance(
            swap,
            context.curves.as_ref(),
            context.as_of,
        )?;
        let forward_vol = forward_variance.max(0.0).sqrt();

        // Discount factor to maturity
        let disc = context
            .curves
            .get_discount(swap.discount_curve_id.as_str())?;
        let df = disc.df_between_dates(context.as_of, swap.effective_settlement_date()?)?;

        // Vega per 1% vol move: DF * 2 * Notional * σ_forward * 0.01 * remaining_fraction.
        // Derivation: PV = N * (σ² - K²) * DF * remaining_fraction
        //             ∂PV/∂σ = N * 2σ * DF * remaining_fraction
        let vega = df * 2.0 * swap.notional.amount() * forward_vol * 0.01 * remaining_fraction;
        Ok(vega * swap.side.sign())
    }
}
