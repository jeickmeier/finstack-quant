//! CollateralPrice01 calculator for Repo.
//!
//! Computes CollateralPrice01 (collateral price sensitivity) using finite differences.
//! CollateralPrice01 measures the change in PV for a 1% change in collateral price.
//!
//! # Formula
//! ```text
//! CollateralPrice01 = (PV(collateral_price * 1.01) - PV(collateral_price * 0.99)) / (2 * bump_size)
//! ```
//! Where bump_size is 1% (0.01).
//!
//! # Note
//! Collateral price is accessed via `collateral.market_value_id` from MarketContext.
//! This metric bumps the collateral price in MarketContext and reprices the repo.
//! Changes in collateral price may affect margin requirements and collateral coverage.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::repo::Repo;
use crate::metrics::{
    central_diff_by_half_bump, replace_scalar_value, scalar_numeric_value, MetricCalculator,
    MetricContext,
};
use finstack_core::Result;

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

        // CollateralPrice01 = (PV_up - PV_down) / (2 * bump_size)
        // Result is PV change per 1% change in collateral price
        // bump_size = current_price * 0.01, so dividing by (2 * bump_size) normalizes
        // to "per unit price move" and then multiplying by 0.01 gives "per 1% move"
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
