//! Collateral index floors: a floating row pays `max(index, floor) + spread`,
//! so a floor above the forward lifts the pool coupon and the tranche OAS.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_oas, run_simulation_with_diagnostics, AssetPool, DealType, OasConfig,
    PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

use super::instrument_pool_tests::market_with_curves;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn close() -> Date {
    d(2024, 1, 15)
}

/// 10M SOFR + 400 loan (SOFR forward flat at 4%), A 9M fixed 5%, E 1M.
fn deal(index_floor: Option<f64>) -> StructuredCredit {
    let maturity = d(2029, 1, 15);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    let mut loan = PoolAsset::floating_rate_loan(
        "L",
        usd(10_000_000.0),
        "USD-SOFR-3M",
        400.0,
        maturity,
        DayCount::Act360,
    );
    loan.index_floor = index_floor;
    pool.assets.push(loan);
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            usd(9_000_000.0),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("A"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(1_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_clo("CLO-FLOOR", pool, tranches, close(), maturity, "USD-OIS")
            .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    deal
}

/// With SOFR at 4% a 6% floor lifts the loan coupon from 8% to 10%: the
/// first period's pool interest rises by 2% × 10M × the accrual, and the
/// equity's residual with it; a 1% floor below the index changes nothing.
#[test]
fn an_index_floor_above_the_forward_lifts_the_pool_coupon() {
    let market = market_with_curves(close());
    let unfloored = run_simulation_with_diagnostics(&deal(None), &market, close()).expect("run");
    let floored =
        run_simulation_with_diagnostics(&deal(Some(0.06)), &market, close()).expect("run");
    let slack = run_simulation_with_diagnostics(&deal(Some(0.01)), &market, close()).expect("run");

    let first = |run: &finstack_quant_valuations::instruments::fixed_income::structured_credit::SimulationRun| {
        run.diagnostics.periods[0].interest_collections.amount()
    };
    let base = first(&unfloored);
    let lifted = first(&floored);
    assert!(
        (lifted / base - 10.0 / 8.0).abs() < 1e-6,
        "a 6% floor on a 4% index lifts SOFR+400 from 8% to 10%: {base} -> {lifted}"
    );
    assert!(
        (first(&slack) - base).abs() < 1e-6,
        "a floor below the index is inert"
    );
    assert!(
        floored.tranches["E"].total_interest.amount()
            > unfloored.tranches["E"].total_interest.amount() + 500_000.0,
        "the extra coupon reaches the residual"
    );
}

/// The floor changes the collateral's rate-path dependence, so the OAS at a
/// given price differs from the unfloored pool's.
#[test]
fn oas_differs_between_floored_and_unfloored_pools() {
    let market = market_with_curves(close());
    let unfloored = calculate_tranche_oas(
        &deal(None),
        "E",
        90.0,
        &market,
        close(),
        &OasConfig::default(),
    )
    .expect("oas");
    let floored = calculate_tranche_oas(
        &deal(Some(0.06)),
        "E",
        90.0,
        &market,
        close(),
        &OasConfig::default(),
    )
    .expect("oas");
    assert!(
        (floored.oas - unfloored.oas).abs() > 1e-4,
        "floored OAS {} vs unfloored {}",
        floored.oas,
        unfloored.oas
    );
}
