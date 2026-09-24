//! XIRR metrics: to-maturity and to-worst-exit.
//!
//! XIRR is the annualized internal rate of return of the investor's cashflow
//! stream, computed from the issue date forward using `Act/365F` unless an XIRR
//! return floor specifies another day-count convention. The metric and floor
//! lowering therefore use the same annualization basis.
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
//! separately by the property and mutation tests in
//! `bond/pricing/return_floor.rs`
//! (`moic_floor_holds_on_every_early_call_path_across_rate_scenarios`,
//! `xirr_floor_holds_on_every_early_call_path`,
//! `moic_check_has_teeth_redemption_below_floor_breaks_target`).
//!
//! # Cashflow sign convention
//!
//! - `(issue_date, -V0)` is the initial outflow (holder pays `V0`).
//! - All subsequent positive flows received by the holder are inflows.

use super::moic::{cost_basis, lifetime_dated_cashflows};
use crate::instruments::fixed_income::bond::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::cashflow::xirr_with_daycount;
use finstack_quant_core::dates::DayCount;

fn day_count(bond: &Bond) -> DayCount {
    bond.return_floor
        .as_ref()
        .and_then(|spec| match spec.kind {
            crate::instruments::fixed_income::bond::ReturnFloorKind::Xirr(_) => spec.day_count,
            crate::instruments::fixed_income::bond::ReturnFloorKind::Moic(_) => None,
        })
        .unwrap_or(DayCount::Act365F)
}

/// XIRR if the bond is held to maturity.
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
        let day_count = day_count(bond);

        let mut flows: Vec<(finstack_quant_core::dates::Date, f64)> = vec![(t0, -v0)];
        for (d, m) in lifetime_dated_cashflows(bond, &ctx.curves)? {
            if d > t0 {
                flows.push((d, m.amount()));
            }
        }

        xirr_with_daycount(&flows, day_count, None)
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
/// the property and mutation tests in `bond/pricing/return_floor.rs`
/// (`moic_floor_holds_on_every_early_call_path_across_rate_scenarios`,
/// `xirr_floor_holds_on_every_early_call_path`,
/// `moic_check_has_teeth_redemption_below_floor_breaks_target`).
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
        let day_count = day_count(bond);

        // Lower any return-floor into call_put before enumerating exit paths.
        let eff = bond.effective_for_pricing(&ctx.curves, ctx.as_of)?;
        let flows = lifetime_dated_cashflows(&eff, &ctx.curves)?;
        let schedule = eff.full_cashflow_schedule(&ctx.curves)?;
        let basis = crate::instruments::fixed_income::bond::pricing::quote_conversions::RedemptionBasis::new(
            &schedule,
            &eff.accrual_config(),
        )?;

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
        let mut worst = xirr_with_daycount(&mat, day_count, None)?;

        // Each call/put candidate path.
        for cand in candidates {
            let mut path: Vec<(finstack_quant_core::dates::Date, f64)> = vec![(t0, -v0)];
            for (d, m) in &flows {
                if *d > t0 && *d <= cand.date {
                    path.push((*d, m.amount()));
                }
            }
            // Exercise prices are clean; accrued is a same-date holder receipt
            // and must be included exactly once in the realized return path.
            let redemption = crate::instruments::fixed_income::bond::pricing::quote_conversions::exercise_redemption_amount(
                &eff,
                &ctx.curves,
                &flows,
                &basis,
                &cand,
            )?;
            path.push((cand.date, redemption));

            if let Ok(r) = xirr_with_daycount(&path, day_count, None) {
                worst = worst.min(r);
            }
        }

        Ok(worst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule, MakeWholeSpec};
    use crate::metrics::{MetricCalculator, MetricContext};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
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
            Money::from((100_i64, Currency::USD)),
            Rate::from_percent(10.0).expect("valid rate fixture"),
            date!(2024 - 01 - 15),
            date!(2026 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .unwrap();

        let curves = Arc::new(MarketContext::new());
        let mut ctx = MetricContext::new(
            Arc::new(bond),
            curves,
            date!(2024 - 01 - 15),
            Money::from((100_i64, Currency::USD)),
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
            Money::from((100_i64, Currency::USD)),
            Rate::from_percent(10.0).expect("valid rate fixture"),
            date!(2024 - 01 - 15),
            date!(2026 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .unwrap();

        let curves = Arc::new(MarketContext::new());
        let mut ctx = MetricContext::new(
            Arc::new(bond),
            curves,
            date!(2024 - 01 - 15),
            Money::from((100_i64, Currency::USD)),
            MetricContext::default_config(),
        );

        let xirr_mat = XirrCalculator.calculate(&mut ctx).unwrap();
        let xirr_worst = XirrToWorstCalculator.calculate(&mut ctx).unwrap();
        assert!(
            (xirr_mat - xirr_worst).abs() < 1e-9,
            "bullet bond: to-maturity {xirr_mat:.6} should equal to-worst {xirr_worst:.6}"
        );
    }

    #[test]
    fn amortizing_bond_xirr_to_worst_redeems_only_outstanding_principal() {
        use crate::cashflow::primitives::CFKind;
        use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

        let curves = Arc::new(MarketContext::new());
        let mut bond = Bond::example_amortizing().expect("amortizing bond");
        let schedule = bond
            .full_cashflow_schedule(&curves)
            .expect("cashflow schedule");
        let call_date = schedule
            .get_flows()
            .iter()
            .find(|flow| flow.kind == CFKind::Amortization && flow.amount.amount() > 0.0)
            .map(|flow| flow.date)
            .expect("amortization date");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: call_date,
                end_date: call_date,
                price_pct_of_par: 90.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        bond.validate().expect("valid callable bond");

        let schedule = bond
            .full_cashflow_schedule(&curves)
            .expect("cashflow schedule");
        let amortized: f64 = schedule
            .get_flows()
            .iter()
            .filter(|flow| {
                flow.date <= call_date
                    && matches!(flow.kind, CFKind::Amortization | CFKind::Notional)
                    && flow.amount.amount() > 0.0
            })
            .map(|flow| flow.amount.amount())
            .sum();
        let outstanding = bond.notional.amount() - amortized;
        assert!(outstanding < bond.notional.amount());

        let distributions: Vec<_> = lifetime_dated_cashflows(&bond, &curves)
            .expect("lifetime flows")
            .into_iter()
            .filter(|(date, _)| *date > bond.issue_date && *date <= call_date)
            .collect();
        let mut expected_path = vec![(bond.issue_date, -bond.notional.amount())];
        expected_path.extend(
            distributions
                .iter()
                .map(|(date, amount)| (*date, amount.amount())),
        );
        expected_path.push((call_date, 0.90 * outstanding));
        let expected = xirr_with_daycount(&expected_path, DayCount::Act365F, None)
            .expect("independent outstanding-principal XIRR");

        let mut initial_notional_path = expected_path[..expected_path.len() - 1].to_vec();
        initial_notional_path.push((call_date, 0.90 * bond.notional.amount()));
        let initial_notional_result =
            xirr_with_daycount(&initial_notional_path, DayCount::Act365F, None)
                .expect("initial-notional comparison XIRR");

        let mut ctx = MetricContext::new(
            Arc::new(bond.clone()),
            curves,
            bond.issue_date,
            bond.notional,
            MetricContext::default_config(),
        );
        let maturity = XirrCalculator.calculate(&mut ctx).expect("maturity XIRR");
        assert!(expected < maturity, "fixture must make the call path worst");
        let actual = XirrToWorstCalculator
            .calculate(&mut ctx)
            .expect("XIRR to worst");

        assert!(
            (actual - expected).abs() < 1e-12,
            "actual={actual}, expected={expected}"
        );
        assert!((actual - initial_notional_result).abs() > 0.01);
    }

    #[test]
    fn xirr_to_worst_applies_make_whole_reference_value() {
        let issue = date!(2025 - 01 - 15);
        let call_date = date!(2025 - 12 - 15);
        let mut make_whole_bond = Bond::fixed(
            "XIRR-MAKE-WHOLE",
            Money::from((100_i64, Currency::USD)),
            Rate::from_percent(10.0).expect("valid rate fixture"),
            issue,
            date!(2027 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        make_whole_bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: call_date,
                end_date: call_date,
                price_pct_of_par: 50.0,
                make_whole: Some(MakeWholeSpec {
                    reference_curve_id: "USD-REF".into(),
                    spread_bp: 0.0,
                }),
            }],
            puts: Vec::new(),
        });
        let mut fixed_strike_bond = make_whole_bond.clone();
        fixed_strike_bond
            .call_put
            .as_mut()
            .expect("call schedule")
            .calls[0]
            .make_whole = None;
        let curves = Arc::new(
            MarketContext::new()
                .insert(
                    DiscountCurve::builder("USD-OIS")
                        .base_date(issue)
                        .knots([(0.0, 1.0), (2.0, 0.90)])
                        .build()
                        .expect("discount curve"),
                )
                .insert(
                    DiscountCurve::builder("USD-REF")
                        .base_date(issue)
                        .knots([(0.0, 1.0), (2.0, 1.0)])
                        .build()
                        .expect("reference curve"),
                ),
        );
        let calculate = |bond: Bond, calculator: &dyn MetricCalculator| {
            let mut ctx = MetricContext::new(
                Arc::new(bond),
                Arc::clone(&curves),
                issue,
                Money::from((100_i64, Currency::USD)),
                MetricContext::default_config(),
            );
            calculator.calculate(&mut ctx).expect("XIRR")
        };

        let maturity = calculate(make_whole_bond.clone(), &XirrCalculator);
        let make_whole = calculate(make_whole_bond, &XirrToWorstCalculator);
        let fixed_strike = calculate(fixed_strike_bond, &XirrToWorstCalculator);

        assert!((make_whole - maturity).abs() < 1e-12);
        assert!(make_whole - fixed_strike > 0.25);
    }
}
