//! NPL/RPL resolution: a non-performing loan pays nothing until its
//! resolution date, where the liquidated share defaults with its net
//! proceeds as the recovery and the re-performing share pays modified
//! coupons from the next period on.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, DealType, LiquidationSpec, LossAllocationPolicy,
    PeriodDiagnostics, PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure,
};
use finstack_quant_valuations::instruments::Instrument;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn close() -> Date {
    d(2024, 1, 1)
}

fn maturity() -> Date {
    d(2030, 1, 1)
}

fn market() -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=12)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(close())
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Two-year workout: 55% gross proceeds less 5% carry, 20% re-performs at 4%.
fn workout() -> LiquidationSpec {
    LiquidationSpec {
        months_to_resolution: 24,
        proceeds_pct: 55.0,
        carry_cost_pct: 5.0,
        reperformance_prob: 0.2,
        modified_rate: Some(0.04),
    }
}

/// A 100M non-performing 6% bullet loan under `spec`, financed by a 50M
/// par-preserving senior note and a 50M residual; no fees, no modeled
/// prepayments or defaults, immediate recovery release.
fn npl_deal(spec: LiquidationSpec) -> StructuredCredit {
    let mut loan = PoolAsset::fixed_rate_bond(
        "NPL-1",
        usd(100_000_000.0),
        0.06,
        maturity(),
        DayCount::Thirty360,
    );
    loan.liquidation = Some(spec);
    let mut pool = AssetPool::new("P", DealType::Rmbs, Currency::USD);
    pool.assets = vec![loan];
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            50.0,
            100.0,
            TrancheSeniority::Senior,
            usd(50_000_000.0),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "E",
            0.0,
            50.0,
            TrancheSeniority::Equity,
            usd(50_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_rmbs("NPL-T18", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse");
    deal.fees = None;
    deal.loss_allocation = Some(LossAllocationPolicy::ParPreserving);
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec.recovery_lag = 0;
    deal
}

fn periods(deal: &StructuredCredit) -> Vec<PeriodDiagnostics> {
    run_simulation_with_diagnostics(deal, &market(), close())
        .expect("run")
        .diagnostics
        .periods
}

fn accrual(from: Date, to: Date) -> f64 {
    DayCount::Thirty360
        .year_fraction(from, to, DayCountContext::default())
        .expect("accrual")
}

/// Nothing is collected before the resolution date; there 80M liquidates
/// for 50% net (55% − 5% carry) and the 20M re-performing share pays 4%
/// from the next period on.
#[test]
fn non_performing_loan_resolves_net_of_carry_and_reperforms_at_the_modified_coupon() {
    let deal = npl_deal(workout());
    deal.validate_invariants().expect("valid");
    let periods = periods(&deal);
    let resolution = d(2026, 1, 1);
    let index = periods
        .iter()
        .position(|p| p.payment_date >= resolution)
        .expect("resolution period");
    assert!(index > 6, "monthly periods precede the resolution");
    for period in &periods[..index] {
        assert_eq!(
            period.interest_collections.amount(),
            0.0,
            "{}",
            period.payment_date
        );
        assert_eq!(
            period.principal_collections.amount(),
            0.0,
            "{}",
            period.payment_date
        );
        assert_eq!(period.defaults.amount(), 0.0, "{}", period.payment_date);
        assert!((period.pool_balance.amount() - 100_000_000.0).abs() < 1e-6);
    }
    let resolved = &periods[index];
    assert!((resolved.defaults.amount() - 80_000_000.0).abs() < 1e-6);
    assert!((resolved.recoveries.amount() - 40_000_000.0).abs() < 1e-6);
    assert_eq!(resolved.interest_collections.amount(), 0.0);
    assert!((resolved.pool_balance.amount() - 20_000_000.0).abs() < 1e-6);

    let next = &periods[index + 1];
    let expected = 20_000_000.0 * 0.04 * accrual(resolved.payment_date, next.payment_date);
    assert!(
        (next.interest_collections.amount() - expected).abs() < 1e-6,
        "modified coupon on the re-performing share: {} vs {expected}",
        next.interest_collections.amount()
    );
    let total_defaults: f64 = periods.iter().map(|p| p.defaults.amount()).sum();
    let total_recoveries: f64 = periods.iter().map(|p| p.recoveries.amount()).sum();
    assert!((total_defaults - 80_000_000.0).abs() < 1e-6);
    assert!((total_recoveries - 40_000_000.0).abs() < 1e-6);
}

/// With no re-performing share the loan is retired at resolution; with a
/// full re-performing share and no modification it keeps its own coupon.
#[test]
fn full_liquidation_retires_the_loan_and_full_reperformance_keeps_the_coupon() {
    let liquidated = npl_deal(LiquidationSpec {
        reperformance_prob: 0.0,
        ..workout()
    });
    let periods_liquidated = periods(&liquidated);
    let resolved = periods_liquidated
        .iter()
        .find(|p| p.payment_date >= d(2026, 1, 1))
        .expect("resolution period");
    assert!((resolved.defaults.amount() - 100_000_000.0).abs() < 1e-6);
    assert_eq!(resolved.pool_balance.amount(), 0.0);
    // The pool is empty from here on: the deal winds up and no later period
    // collects anything.
    let last = periods_liquidated.last().expect("periods");
    assert_eq!(last.pool_balance.amount(), 0.0);
    for period in periods_liquidated
        .iter()
        .filter(|p| p.payment_date > resolved.payment_date)
    {
        assert_eq!(
            period.interest_collections.amount(),
            0.0,
            "{}",
            period.payment_date
        );
        assert_eq!(period.pool_balance.amount(), 0.0, "{}", period.payment_date);
    }
    let total_recoveries: f64 = periods_liquidated
        .iter()
        .map(|p| p.recoveries.amount())
        .sum();
    assert!((total_recoveries - 50_000_000.0).abs() < 1e-6);

    let reperforming = npl_deal(LiquidationSpec {
        reperformance_prob: 1.0,
        modified_rate: None,
        ..workout()
    });
    let periods_reperforming = periods(&reperforming);
    let index = periods_reperforming
        .iter()
        .position(|p| p.payment_date >= d(2026, 1, 1))
        .expect("resolution period");
    let resolved = &periods_reperforming[index];
    let next = &periods_reperforming[index + 1];
    assert_eq!(resolved.defaults.amount(), 0.0);
    let expected = 100_000_000.0 * 0.06 * accrual(resolved.payment_date, next.payment_date);
    assert!((next.interest_collections.amount() - expected).abs() < 1e-6);
}

/// The default flag and recovery inputs are replaced by the timeline, and the
/// resolution must fall inside the deal's life.
#[test]
fn liquidation_terms_are_validated() {
    let mut flagged = npl_deal(workout());
    flagged.pool.assets[0].is_defaulted = true;
    assert!(flagged.validate_invariants().is_err());

    let mut late = npl_deal(LiquidationSpec {
        months_to_resolution: 120,
        ..workout()
    });
    late.pool.assets[0].liquidation = Some(LiquidationSpec {
        months_to_resolution: 120,
        ..workout()
    });
    assert!(late.validate_invariants().is_err());

    let proceeds = npl_deal(LiquidationSpec {
        proceeds_pct: 150.0,
        ..workout()
    });
    assert!(proceeds.validate_invariants().is_err());

    let prob = npl_deal(LiquidationSpec {
        reperformance_prob: 1.5,
        ..workout()
    });
    assert!(prob.validate_invariants().is_err());
}
