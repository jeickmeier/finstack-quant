//! Per-asset seasoning: dated rows read seasoning-dependent prepayment and
//! default curves at their own age, and `origination_date` (not the
//! acquisition date) anchors that age. The 2026-09-17 audit found every row
//! ran at the pool's balance-weighted age, which misstates ABS/PSA/SDA
//! speeds for any pool mixing new and seasoned collateral.

use finstack_quant_cashflows::builder::{
    abs_to_smm, DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, DealType, PoolAsset, StructuredCredit, Tranche,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
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
    d(2029, 1, 1)
}

fn market() -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=10)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(close())
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Monthly ABS of two 1M five-year 6% bullets (`A`, `B`) behind one 2M
/// senior note; the caller dates the rows and sets the curves.
fn abs(rows: impl FnOnce(&mut PoolAsset, &mut PoolAsset)) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    let mut a =
        PoolAsset::fixed_rate_bond("A", usd(1_000_000.0), 0.06, maturity(), DayCount::Act360);
    let mut b =
        PoolAsset::fixed_rate_bond("B", usd(1_000_000.0), 0.06, maturity(), DayCount::Act360);
    rows(&mut a, &mut b);
    pool.assets.push(a);
    pool.assets.push(b);
    let tranches = TrancheStructure::new(vec![Tranche::new(
        "N",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        usd(2_000_000.0),
        TrancheCoupon::Fixed { rate: 0.04 },
        maturity(),
    )
    .expect("N")])
    .expect("structure");
    let mut deal = StructuredCredit::new_abs(
        "ABS-SEASONING",
        pool,
        tranches,
        close(),
        maturity(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);
    deal
}

fn first_period_principal(deal: &StructuredCredit) -> f64 {
    run_simulation_with_diagnostics(deal, &market(), close())
        .expect("simulation")
        .diagnostics
        .periods[0]
        .principal_collections
        .amount()
}

fn first_period_defaults(deal: &StructuredCredit) -> f64 {
    run_simulation_with_diagnostics(deal, &market(), close())
        .expect("simulation")
        .diagnostics
        .periods[0]
        .defaults
        .amount()
}

/// Under a 1.5% ABS speed a row originated at closing prepays at 1.5% SMM
/// in its first month while a row originated two years earlier is at
/// month 25 of its curve (2.34% SMM); the pool's 12-month average age
/// would have put both at 1.83%.
#[test]
fn dated_rows_read_the_abs_curve_at_their_own_age() {
    let mut deal = abs(|a, b| {
        a.origination_date = Some(close());
        b.origination_date = Some(d(2022, 1, 1));
    });
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::abs(0.015);
    let expected = 1_000_000.0 * abs_to_smm(0.015, 1).expect("smm")
        + 1_000_000.0 * abs_to_smm(0.015, 25).expect("smm");
    let pooled = 2_000_000.0 * abs_to_smm(0.015, 13).expect("smm");
    let principal = first_period_principal(&deal);
    assert!(
        (principal - expected).abs() < 1.0,
        "per-asset ABS speeds: {principal} vs {expected} (pool-average would be {pooled})"
    );
    assert!((principal - pooled).abs() > 1_000.0);
}

/// `origination_date` anchors the age; a row that only carries an
/// acquisition date ages from that instead, and an undated row is new at
/// closing.
#[test]
fn origination_date_is_preferred_over_acquisition_for_seasoning() {
    let mut deal = abs(|a, b| {
        a.origination_date = Some(d(2022, 1, 1));
        a.acquisition_date = Some(close());
        b.acquisition_date = Some(d(2023, 1, 1));
    });
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::abs(0.015);
    let expected = 1_000_000.0 * abs_to_smm(0.015, 25).expect("smm")
        + 1_000_000.0 * abs_to_smm(0.015, 13).expect("smm");
    let principal = first_period_principal(&deal);
    assert!(
        (principal - expected).abs() < 1.0,
        "origination then acquisition anchors: {principal} vs {expected}"
    );

    let mut undated = abs(|_, _| {});
    undated.credit_model.prepayment_spec = PrepaymentModelSpec::abs(0.015);
    let principal = first_period_principal(&undated);
    let expected = 2_000_000.0 * abs_to_smm(0.015, 1).expect("smm");
    assert!(
        (principal - expected).abs() < 1.0,
        "undated rows are new at closing: {principal} vs {expected}"
    );
}

/// The SDA ramp is read per dated row too: a 30-month-old row is at the
/// 0.6% CDR plateau while a new row is at month 1 of the ramp.
#[test]
fn dated_rows_read_the_sda_curve_at_their_own_age() {
    let mut deal = abs(|a, b| {
        a.origination_date = Some(close());
        b.origination_date = Some(d(2021, 7, 1));
    });
    let sda = DefaultModelSpec::sda(1.0);
    deal.credit_model.default_spec = sda.clone();
    let expected = 1_000_000.0 * sda.mdr(1).expect("mdr") + 1_000_000.0 * sda.mdr(31).expect("mdr");
    let defaults = first_period_defaults(&deal);
    assert!(
        (defaults - expected).abs() < 1.0,
        "per-asset SDA rates: {defaults} vs {expected}"
    );
    assert!(sda.mdr(31).expect("mdr") > 20.0 * sda.mdr(1).expect("mdr"));
}
