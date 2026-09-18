//! Amortization terms and interest-only periods on level-pay collateral: the
//! schedule runs over the term from origination (balloon at maturity), no
//! principal is scheduled inside the interest-only window, and a seasoned rep
//! line amortizes over the term less its seasoning.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, AssetType, DealType, PoolAsset, RepLine,
    SimulationRun, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
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

/// Monthly CMBS on `pool`: A 90% at 5%, E 10%; no fees, prepayments or
/// defaults.
fn cmbs(pool: AssetPool, maturity: Date) -> StructuredCredit {
    let total = pool.total_balance().expect("balance").amount();
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            usd(total * 0.9),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("A"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(total * 0.1),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_cmbs("CMBS-TERM", pool, tranches, close(), maturity, "USD-OIS")
            .with_payment_calendar("nyse");
    deal.fees = None;
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);
    deal
}

fn simulate(deal: &StructuredCredit) -> SimulationRun {
    run_simulation_with_diagnostics(deal, &market(), close()).expect("simulation")
}

/// Balance after `k` level payments of a schedule over `n` periods at
/// periodic rate `r` on principal `p`.
fn balance_after(p: f64, r: f64, n: i32, k: i32) -> f64 {
    p * ((1.0 + r).powi(n) - (1.0 + r).powi(k)) / ((1.0 + r).powi(n) - 1.0)
}

/// A 10-year 6% commercial mortgage: interest-only for five years, then a
/// 30-year schedule on the full balance for the last five, leaving the
/// balance after 60 of 360 payments as the balloon.
#[test]
fn interest_only_then_thirty_year_schedule_reproduces_the_balance_path() {
    let maturity = d(2034, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Cmbs, Currency::USD);
    let mut loan =
        PoolAsset::fixed_rate_bond("L", usd(10_000_000.0), 0.06, maturity, DayCount::Thirty360);
    loan.asset_type = AssetType::CommercialMortgage { ltv: None };
    loan.amortization_term_months = Some(420);
    loan.io_months = Some(60);
    pool.assets.push(loan);
    let run = simulate(&cmbs(pool, maturity));
    let periods = &run.diagnostics.periods;

    for period in &periods[..60] {
        assert!(
            period.principal_collections.amount() < 1e-6,
            "no scheduled principal inside the interest-only window: {} on {}",
            period.principal_collections.amount(),
            period.payment_date
        );
    }
    let r: f64 = 0.06 / 12.0;
    // After the IO window 360 months of schedule remain (420 − 60).
    let level_payment = 10_000_000.0 * r / (1.0 - (1.0 + r).powi(-360));
    let first_amortizing = periods[60].principal_collections.amount();
    assert!(
        (first_amortizing - (level_payment - 10_000_000.0 * r)).abs() < 1.0,
        "month 61 pays the first scheduled principal of the 30-year schedule: {first_amortizing}"
    );
    let balance_before_maturity = balance_after(10_000_000.0, r, 360, 59);
    let last = periods.last().expect("periods");
    assert!(
        (last.principal_collections.amount() - balance_before_maturity).abs() < 1.0,
        "the balloon is the balance after 59 of 360 payments: {} vs {balance_before_maturity}",
        last.principal_collections.amount()
    );
    let total: f64 = periods
        .iter()
        .map(|period| period.principal_collections.amount())
        .sum();
    assert!(
        (total - 10_000_000.0).abs() < 1.0,
        "every dollar comes back: {total}"
    );
}

/// A rep line with a 360-month term at factor 0.5 (180 months seasoned)
/// amortizes its current balance over the 180 months left on the schedule.
#[test]
fn a_seasoned_rep_line_amortizes_over_the_remaining_term() {
    let maturity = d(2039, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Rmbs, Currency::USD);
    let mut line = RepLine::new(
        "LINE",
        usd(5_000_000.0),
        0.06,
        maturity,
        DayCount::Thirty360,
        AssetType::SingleFamilyMortgage { ltv: None },
    );
    line.seasoning_months = 180;
    line.amortization_term_months = Some(360);
    pool.rep_lines = Some(vec![line]);
    let run = simulate(&cmbs(pool, maturity));
    let r: f64 = 0.06 / 12.0;
    let level_payment = 5_000_000.0 * r / (1.0 - (1.0 + r).powi(-180));
    let first = run.diagnostics.periods[0].principal_collections.amount();
    assert!(
        (first - (level_payment - 5_000_000.0 * r)).abs() < 1.0,
        "first scheduled principal on a 180-month remaining schedule: {first}"
    );
    let total: f64 = run
        .diagnostics
        .periods
        .iter()
        .map(|period| period.principal_collections.amount())
        .sum();
    assert!(
        (total - 5_000_000.0).abs() < 1.0,
        "fully amortized by maturity: {total}"
    );

    // A term shorter than the loan's life is rejected.
    let mut short = AssetPool::new("P", DealType::Cmbs, Currency::USD);
    let mut loan =
        PoolAsset::fixed_rate_bond("L", usd(1_000_000.0), 0.06, maturity, DayCount::Thirty360);
    loan.asset_type = AssetType::CommercialMortgage { ltv: None };
    loan.amortization_term_months = Some(60);
    short.assets.push(loan);
    assert!(
        run_simulation_with_diagnostics(&cmbs(short, maturity), &market(), close()).is_err(),
        "an amortization term inside the loan's life is rejected"
    );
}
