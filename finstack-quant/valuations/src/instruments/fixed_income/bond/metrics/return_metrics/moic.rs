//! MOIC (money-on-invested-capital) metrics: to-maturity and to-worst-exit.
//!
//! MOIC is the total investor return as a multiple of the invested capital:
//!
//! ```text
//! MOIC = Σ(positive cashflows received after issue) / V0
//! ```
//!
//! where `V0` is the cost basis (invested capital) derived from the bond's
//! [`IssuePrice`]: par notional by default, or an explicit amount / OID percentage.
//!
//! # Floor scope (important)
//!
//! The return floor is **call-protection only**: it bounds the realized return
//! on EARLY (issuer-called/put) redemptions, NOT the held-to-maturity path (see
//! `lower_return_floor` in `bond/pricing/return_floor.rs`). The to-worst metric
//! takes the minimum over **all** exits — every early-call/put path AND the
//! unfloored held-to-maturity path — so it is **not** bounded below by the floor
//! target. When the bond's natural maturity return is below the target, the
//! maturity path is the worst case and the metric reflects that. The floor's
//! guarantee (every early-call path meets the target) is verified separately by
//! the property and mutation tests in `bond/pricing/return_floor.rs`
//! (`moic_floor_holds_on_every_early_call_path_across_rate_scenarios`,
//! `xirr_floor_holds_on_every_early_call_path`,
//! `moic_check_has_teeth_redemption_below_floor_breaks_target`).

use crate::instruments::fixed_income::bond::Bond;
use crate::instruments::fixed_income::bond::IssuePrice;
use crate::metrics::{MetricCalculator, MetricContext};

/// Resolve the cost basis `(issue_date, V0)` from the bond.
///
/// `V0` (invested capital) uses the bond's `return_floor.issue_price` when
/// present, otherwise par notional. The anchor date is always `bond.issue_date`
/// (issuer-side convention). `V0` is resolved via the shared
/// [`IssuePrice::resolve`] (the same resolver the floor lowering uses) so the
/// metric and the floor cannot drift.
///
/// # Errors
///
/// Returns `Error::Validation` if an [`IssuePrice::Amount`] currency does not
/// match the bond's notional currency, or if the resolved `V0` is not strictly
/// positive.
pub(crate) fn cost_basis(
    bond: &Bond,
) -> finstack_quant_core::Result<(finstack_quant_core::dates::Date, f64)> {
    let issue_price = bond
        .return_floor
        .as_ref()
        .map_or(IssuePrice::Par, |s| s.issue_price);
    let v0 = issue_price.resolve(bond.notional)?;
    if v0 <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "return floor metrics require a positive issue price".to_string(),
        ));
    }
    Ok((bond.issue_date, v0))
}

/// Full holder-view contractual cashflows used by lifetime return metrics.
/// Unlike pricing cashflows, this intentionally retains already-paid coupons
/// so an issue-date cost basis is matched with an issue-date return horizon.
pub(crate) fn lifetime_dated_cashflows(
    bond: &Bond,
    curves: &finstack_quant_core::market_data::context::MarketContext,
) -> finstack_quant_core::Result<
    Vec<(
        finstack_quant_core::dates::Date,
        finstack_quant_core::money::Money,
    )>,
> {
    use finstack_quant_core::cashflow::CFKind;

    let schedule = bond.full_cashflow_schedule(curves)?;
    Ok(schedule
        .flows
        .into_iter()
        .filter(|cf| {
            cf.kind != CFKind::PIK && !(cf.kind == CFKind::Notional && cf.amount.amount() < 0.0)
        })
        .map(|cf| (cf.date, cf.amount))
        .collect())
}

/// MOIC if held to maturity.
///
/// Computes the sum of all positive cashflows received by the holder strictly
/// after the issue date, divided by `V0`.
///
/// # Returns
///
/// `Ok(moic)` where `moic >= 1.0` for a par bond that pays coupons and returns
/// principal. Values below 1.0 indicate a loss of principal.
///
/// # Errors
///
/// Returns an error if the instrument is not a `Bond` or if cashflow generation fails.
pub(crate) struct MoicCalculator;

impl MetricCalculator for MoicCalculator {
    fn calculate(&self, ctx: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = ctx.instrument_as()?;
        let (t0, v0) = cost_basis(bond)?;
        let flows = lifetime_dated_cashflows(bond, &ctx.curves)?;
        let total_in: f64 = flows
            .iter()
            .filter(|(d, _)| *d > t0)
            .map(|(_, m)| m.amount().max(0.0))
            .sum();
        Ok(total_in / v0)
    }
}

/// Worst (minimum) realized money multiple across **all** exits: every
/// early-call/put path AND the held-to-maturity path.
///
/// Considers:
/// 1. The held-to-maturity path (all positive flows after issue).
/// 2. Every call/put candidate produced by `enumerate_exit_paths`: coupons
///    received in `(issue, exit]` plus the stated redemption price (% of notional).
///
/// Returns the **minimum** MOIC across these paths.
///
/// # Floor scope
///
/// The return floor protects only EARLY redemptions; the held-to-maturity path
/// is unfloored. This value is therefore **not** bounded below by the floor
/// target — when the bond's natural maturity return is below the target, the
/// maturity path is the worst case and this metric reflects that. The floor's
/// guarantee (every EARLY-CALL path meets the target) is verified separately by
/// the property and mutation tests in `bond/pricing/return_floor.rs`
/// (`moic_floor_holds_on_every_early_call_path_across_rate_scenarios`,
/// `xirr_floor_holds_on_every_early_call_path`,
/// `moic_check_has_teeth_redemption_below_floor_breaks_target`).
///
/// # Errors
///
/// Returns an error if the instrument is not a `Bond`, if the effective bond
/// cannot be derived, or if cashflow generation fails.
pub(crate) struct MoicToWorstCalculator;

impl MetricCalculator for MoicToWorstCalculator {
    fn calculate(&self, ctx: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = ctx.instrument_as()?;
        let (t0, v0) = cost_basis(bond)?;

        // Lower any return-floor into call_put before enumerating exit paths.
        let eff = bond.effective_for_pricing(&ctx.curves, ctx.as_of)?;
        let flows = lifetime_dated_cashflows(&eff, &ctx.curves)?;
        let schedule = eff.full_cashflow_schedule(&ctx.curves)?;

        let candidates = crate::instruments::fixed_income::bond::pricing::quote_conversions::enumerate_exit_paths(
            &eff, &flows, ctx.as_of,
        );

        // Held-to-maturity path: all positive inflows after issue.
        let to_mat: f64 = flows
            .iter()
            .filter(|(d, _)| *d > t0)
            .map(|(_, m)| m.amount().max(0.0))
            .sum::<f64>()
            / v0;

        // Worst across maturity and each call/put candidate.
        let mut worst = to_mat;
        for cand in candidates {
            // Coupons received in (issue, exit_date].
            let coupons: f64 = flows
                .iter()
                .filter(|(d, _)| *d > t0 && *d <= cand.date)
                .map(|(_, m)| m.amount().max(0.0))
                .sum();
            let outstanding = crate::instruments::fixed_income::bond::pricing::quote_conversions::outstanding_principal_at_date(
                &schedule,
                cand.date,
            );
            let redemption = outstanding * cand.price_pct_of_par / 100.0;
            worst = worst.min((coupons + redemption) / v0);
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

    /// 2-year 10% semi-annual bullet at par-100.
    ///
    /// 4 coupons × 5.0 = 20.0, plus 100 redemption = 120.0 total inflow.
    /// MOIC = 120 / 100 = 1.20.
    #[test]
    fn moic_to_maturity_for_par_10pct_2y_is_1_20() {
        let bond = Bond::fixed(
            "L",
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

        let moic = MoicCalculator.calculate(&mut ctx).unwrap();
        assert!(
            (moic - 1.20).abs() < 1e-3,
            "expected MOIC ≈ 1.20, got {moic}"
        );
    }

    #[test]
    fn lifetime_moic_retains_already_paid_coupons() {
        let bond = Bond::fixed(
            "LIFETIME",
            Money::new(100.0, Currency::USD),
            Rate::from_percent(10.0),
            date!(2024 - 01 - 15),
            date!(2026 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");
        let curves = Arc::new(MarketContext::new());
        let mut at_issue = MetricContext::new(
            Arc::new(bond.clone()),
            Arc::clone(&curves),
            date!(2024 - 01 - 15),
            Money::new(100.0, Currency::USD),
            MetricContext::default_config(),
        );
        let mut after_coupon = MetricContext::new(
            Arc::new(bond),
            curves,
            date!(2024 - 08 - 01),
            Money::new(100.0, Currency::USD),
            MetricContext::default_config(),
        );

        let issue_moic = MoicCalculator.calculate(&mut at_issue).expect("issue MOIC");
        let seasoned_moic = MoicCalculator
            .calculate(&mut after_coupon)
            .expect("seasoned MOIC");
        assert!((issue_moic - seasoned_moic).abs() < 1e-12);
    }

    /// Bullet bond without call options: to-worst equals to-maturity.
    #[test]
    fn moic_to_worst_equals_to_maturity_for_bullet_bond() {
        let bond = Bond::fixed(
            "L",
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

        let moic_mat = MoicCalculator.calculate(&mut ctx).unwrap();
        let moic_worst = MoicToWorstCalculator.calculate(&mut ctx).unwrap();
        assert!(
            (moic_mat - moic_worst).abs() < 1e-9,
            "bullet bond: to-maturity {moic_mat} should equal to-worst {moic_worst}"
        );
    }
}
