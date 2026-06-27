//! XIRR metrics: to-maturity and to-worst-exit.
//!
//! XIRR is the annualized internal rate of return of the investor's cashflow
//! stream, computed from the issue date forward using the `Act/365F` day-count
//! convention. This matches the day count used by
//! [`finstack_quant_core::cashflow::xirr`] and therefore matches the lowering
//! logic in `bond/pricing/return_floor.rs` (so each early-call path's XIRR
//! reproduces the floor target exactly).
//!
//! # Floor scope (important)
//!
//! The return floor is **call-protection only**: it bounds the realized XIRR on
//! EARLY (issuer-called/put) redemptions, NOT the held-to-maturity path. The
//! to-worst metric takes the minimum over **all** exits — every early-call/put
//! path AND the unfloored held-to-maturity path — so it is **not** bounded below
//! by the floor target. When the bond's natural maturity return is below the
//! target, the maturity path is the worst case and the metric reflects that. The
//! floor's guarantee (every early-call path meets the target) is verified
//! separately by the `xirr_floor_meets_target_at_each_call` test in
//! `bond/pricing/return_floor.rs`.
//!
//! # Cashflow sign convention
//!
//! - `(issue_date, -V0)` is the initial outflow (holder pays `V0`).
//! - All subsequent positive flows received by the holder are inflows.

use super::moic::cost_basis;
use crate::instruments::fixed_income::bond::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::cashflow::xirr;

/// XIRR (Act/365F) if the bond is held to maturity.
///
/// Constructs the cashflow vector `[(issue_date, -V0), (d1, c1), …]` and
/// delegates to `finstack_quant_core::cashflow::xirr`.
///
/// # Returns
///
/// Annualized IRR as a decimal (e.g., `0.10` = 10 %).
///
/// # Errors
///
/// Returns an error if the instrument is not a `Bond`, cashflow generation
/// fails, or the XIRR solver does not converge.
pub(crate) struct XirrCalculator;

impl MetricCalculator for XirrCalculator {
    fn calculate(&self, ctx: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = ctx.instrument_as()?;
        let (t0, v0) = cost_basis(bond)?;

        let mut flows: Vec<(finstack_quant_core::dates::Date, f64)> = vec![(t0, -v0)];
        for (d, m) in bond.pricing_dated_cashflows(&ctx.curves, ctx.as_of)? {
            if d > t0 {
                flows.push((d, m.amount()));
            }
        }

        xirr(&flows, None)
    }
}

/// Worst (minimum) realized XIRR across **all** exits: every early-call/put
/// path AND the held-to-maturity path.
///
/// Considers:
/// 1. The held-to-maturity path.
/// 2. Every call/put candidate: coupons received in `(issue, exit]` plus
///    the stated redemption price (% of notional).
///
/// Returns the **minimum** XIRR across these paths. Paths where the solver
/// fails to converge are silently skipped (degenerate or trivially dominated).
///
/// # Floor scope
///
/// The return floor protects only EARLY redemptions; the held-to-maturity path
/// is unfloored. This value is therefore **not** bounded below by the floor
/// target — when the bond's natural maturity return is below the target, the
/// maturity path is the worst case and this metric reflects that. The floor's
/// guarantee (every EARLY-CALL path meets the target) is verified separately by
/// the `xirr_floor_meets_target_at_each_call` test in
/// `bond/pricing/return_floor.rs`.
///
/// # Limitation
///
/// Redemption uses the initial notional (`bond.notional`), which is exact for
/// bullet bonds. Amortizing bonds need the outstanding principal at the call
/// date here; that is a v1 limitation (TODO).
///
/// # Errors
///
/// Returns an error if the instrument is not a `Bond`, if the effective bond
/// cannot be derived, or if the maturity-path XIRR solver fails.
pub(crate) struct XirrToWorstCalculator;

impl MetricCalculator for XirrToWorstCalculator {
    fn calculate(&self, ctx: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = ctx.instrument_as()?;
        let (t0, v0) = cost_basis(bond)?;

        // Lower any return-floor into call_put before enumerating exit paths.
        let eff = bond.effective_for_pricing(&ctx.curves, ctx.as_of)?;
        let flows = eff.pricing_dated_cashflows(&ctx.curves, ctx.as_of)?;

        let candidates = crate::instruments::fixed_income::bond::pricing::quote_conversions::enumerate_exit_paths(
            &eff, &flows, ctx.as_of,
        );

        // Maturity path.
        let mut mat: Vec<(finstack_quant_core::dates::Date, f64)> = vec![(t0, -v0)];
        for (d, m) in &flows {
            if *d > t0 {
                mat.push((*d, m.amount()));
            }
        }
        let mut worst = xirr(&mat, None)?;

        // Each call/put candidate path.
        for cand in candidates {
            let mut path: Vec<(finstack_quant_core::dates::Date, f64)> = vec![(t0, -v0)];
            for (d, m) in &flows {
                if *d > t0 && *d <= cand.date {
                    path.push((*d, m.amount()));
                }
            }
            // Redemption cash = stated price % of notional added at exit date.
            let redemption = bond.notional.amount() * cand.price_pct_of_par / 100.0;
            path.push((cand.date, redemption));

            if let Ok(r) = xirr(&path, None) {
                worst = worst.min(r);
            }
        }

        Ok(worst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::Bond;
    use crate::metrics::{MetricCalculator, MetricContext};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Rate;
    use std::sync::Arc;
    use time::macros::date;

    /// A 2-year 10% semi-annual bullet at par-100 has XIRR close to 10%.
    ///
    /// (30/360 coupons give a coupon of exactly 5.0 per period, so the XIRR
    /// is very close to but not necessarily exactly 10% due to day-count basis.)
    #[test]
    fn xirr_to_maturity_for_par_10pct_2y_is_near_10pct() {
        let bond = Bond::fixed(
            "X",
            Money::new(100.0, Currency::USD),
            Rate::from_percent(10.0),
            date!(2024 - 01 - 15),
            date!(2026 - 01 - 15),
            "USD-OIS",
        )
        .unwrap();

        let curves = Arc::new(MarketContext::new());
        let mut ctx = MetricContext::new(
            Arc::new(bond),
            curves,
            date!(2024 - 01 - 15),
            Money::new(100.0, Currency::USD),
            MetricContext::default_config(),
        );

        let r = XirrCalculator.calculate(&mut ctx).unwrap();
        // Par bond: XIRR should be close to the coupon rate (within 50bp of 10%).
        assert!(r > 0.09 && r < 0.11, "expected XIRR ≈ 10%, got {:.4}", r);
    }

    /// Bullet bond without call options: to-worst XIRR equals to-maturity XIRR.
    #[test]
    fn xirr_to_worst_equals_to_maturity_for_bullet_bond() {
        let bond = Bond::fixed(
            "X",
            Money::new(100.0, Currency::USD),
            Rate::from_percent(10.0),
            date!(2024 - 01 - 15),
            date!(2026 - 01 - 15),
            "USD-OIS",
        )
        .unwrap();

        let curves = Arc::new(MarketContext::new());
        let mut ctx = MetricContext::new(
            Arc::new(bond),
            curves,
            date!(2024 - 01 - 15),
            Money::new(100.0, Currency::USD),
            MetricContext::default_config(),
        );

        let xirr_mat = XirrCalculator.calculate(&mut ctx).unwrap();
        let xirr_worst = XirrToWorstCalculator.calculate(&mut ctx).unwrap();
        assert!(
            (xirr_mat - xirr_worst).abs() < 1e-9,
            "bullet bond: to-maturity {xirr_mat:.6} should equal to-worst {xirr_worst:.6}"
        );
    }
}
