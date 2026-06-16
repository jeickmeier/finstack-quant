//! Haircut01 calculator for Repo.
//!
//! Computes Haircut01 (haircut sensitivity) using finite differences.
//! Haircut01 measures the change in PV for a 1bp (0.0001 = 0.01%) change in haircut.
//!
//! # Formula
//! ```text
//! Haircut01 = (PV(haircut + 1bp) - PV(haircut - 1bp)) / (2 * bump_size)
//! ```
//! Where bump_size is 1bp (0.0001).
//!
//! # Important Limitation
//!
//! In the current simple repo model, haircut affects the **required collateral value**
//! but does **not** directly affect the PV calculation. The repo PV is computed from:
//! - Initial cash outflow at start
//! - Discounted repayment (principal + interest) at maturity
//!
//! Neither of these cashflows depends on the haircut parameter. Therefore, this metric
//! will return approximately **zero** for standard repos.
//!
//! The metric becomes meaningful in more sophisticated models that incorporate:
//! - Margin call cashflows based on collateral coverage
//! - Collateral funding costs
//! - Credit valuation adjustments (CVA) sensitive to overcollateralization
//!
//! # Alternative Use
//!
//! For sensitivity of collateral requirements to haircut changes, use the
//! `RequiredCollateral` metric with manual haircut perturbation, or compute:
//! ```text
//! d(RequiredCollateral)/d(haircut) = Cash / (1 - haircut)^2
//! ```

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::repo::Repo;
use crate::metrics::{central_diff_by_width, MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard haircut bump: 1bp (0.0001 = 0.01%)
const HAIRCUT_BUMP: f64 = 0.0001;

/// Haircut01 calculator for Repo.
pub(crate) struct Haircut01Calculator;

impl MetricCalculator for Haircut01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo: &Repo = context.instrument_as()?;
        let as_of = context.as_of;

        // Bump haircut up
        let haircut_up = repo.haircut + HAIRCUT_BUMP;
        let mut repo_up = repo.clone();
        repo_up.haircut = haircut_up;
        let pv_up = repo_up.value(context.curves.as_ref(), as_of)?.amount();

        // Bump haircut down, clamped at zero (a negative haircut is invalid).
        let haircut_down = (repo.haircut - HAIRCUT_BUMP).max(0.0);
        let mut repo_down = repo.clone();
        repo_down.haircut = haircut_down;
        let pv_down = repo_down.value(context.curves.as_ref(), as_of)?.amount();

        // Normalize by the *actual* applied bump width: when the down-bump
        // clamps at zero (haircut < 1bp), the width is `haircut_up -
        // haircut_down < 2bp` and dividing by a fixed 2bp would understate
        // the sensitivity. `haircut_up > haircut_down` always holds, so the
        // helper's degenerate-width error path cannot trigger here.
        central_diff_by_width(pv_up, pv_down, haircut_up - haircut_down)
    }
}
