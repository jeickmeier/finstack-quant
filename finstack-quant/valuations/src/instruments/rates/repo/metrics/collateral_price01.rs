//! CollateralPrice01 calculator for Repo.
//!
//! Computes CollateralPrice01 (collateral price sensitivity) using finite differences.
//!
//! # Formula
//! ```text
//! CollateralPrice01 = (PV(price × 1.01) - PV(price × 0.99)) / (2 × 0.01)
//! ```
//! i.e. the central difference normalized by the **relative** bump width, so the
//! result is the PV change per unit relative move in the collateral price
//! (divide by 100 for a per-1% view).
//!
//! # Important Limitation
//!
//! In the current simple repo model the collateral price affects collateral
//! **coverage** metrics (via `collateral.market_value_id`) but does **not**
//! enter the PV calculation at all. The repo PV is computed from:
//! - Initial cash outflow at start
//! - Discounted repayment (principal + interest) at maturity
//!
//! Neither cashflow depends on the collateral price, so this metric returns
//! exactly **zero** for standard repos. It becomes meaningful only in models
//! that incorporate margin-call cashflows, collateral funding costs, or
//! default/close-out exposure to the collateral's market value.
//!
//! For collateral mark-to-market monitoring, use the `CollateralValue` and
//! `CollateralCoverage` metrics instead.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::repo::Repo;
use crate::metrics::{
    central_diff_by_half_bump, replace_scalar_value, scalar_numeric_value, MetricCalculator,
    MetricContext,
};
use finstack_quant_core::Result;

/// Standard collateral price bump: 1% (0.01)
const COLLATERAL_PRICE_BUMP_PCT: f64 = 0.01;

/// CollateralPrice01 calculator for Repo.
pub(crate) struct CollateralPrice01Calculator;

impl MetricCalculator for CollateralPrice01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo: &Repo = context.instrument_as()?;
        let as_of = context.as_of;

        // Get current collateral price
        let market_value_id = &repo.collateral.market_value_id;
        let current_scalar = context.curves.get_price(market_value_id)?;
        let current_price = scalar_numeric_value(current_scalar);

        // Bump collateral price up by 1%
        let bumped_price_up = current_price * (1.0 + COLLATERAL_PRICE_BUMP_PCT);
        let ctx_up = replace_scalar_value(
            &context.curves,
            market_value_id.as_str(),
            current_scalar,
            bumped_price_up,
        );
        let pv_up = repo.value(&ctx_up, as_of)?.amount();

        // Bump collateral price down by 1%
        let bumped_price_down = current_price * (1.0 - COLLATERAL_PRICE_BUMP_PCT);
        let ctx_down = replace_scalar_value(
            &context.curves,
            market_value_id.as_str(),
            current_scalar,
            bumped_price_down,
        );
        let pv_down = repo.value(&ctx_down, as_of)?.amount();

        // CollateralPrice01 = (PV_up - PV_down) / (2 * 0.01): the central
        // difference normalized by the relative bump width, i.e. the PV change
        // per *unit* relative move in the collateral price (divide by 100 for
        // a per-1% view). Identically zero in the current model — see the
        // module-level limitation note.
        // `COLLATERAL_PRICE_BUMP_PCT` is a fixed positive constant, so the bump
        // width is never degenerate; the helper's error path cannot trigger here.
        let collateral_price01 = if current_price.abs() > 1e-10 {
            central_diff_by_half_bump(pv_up, pv_down, COLLATERAL_PRICE_BUMP_PCT)?
        } else {
            0.0
        };

        Ok(collateral_price01)
    }
}
