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
use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, AssetType, DealType, PoolAsset, RepLine,
    SimulationRun, StructuredCredit, Tranche, TrancheSeniority, TrancheStructure,
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
            RateSpec::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("A"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(total * 0.1),
            RateSpec::Fixed { rate: 0.0 },
            maturity,
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_cmbs("CMBS-TERM", pool, tranches, close(), maturity, "USD-OIS")
            .expect("valid structured-credit dates")
            .with_calendar_id("nyse");
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

fn zero_rate_deal(explicit_payment: bool, io_months: Option<u32>) -> StructuredCredit {
    let maturity = d(2025, 1, 1);
    let mut pool = AssetPool::new("ZERO", DealType::Cmbs, Currency::USD);
    let mut loan = PoolAsset::fixed_rate_bond(
        "ZERO",
        usd(12_000_000.0),
        0.0,
        maturity,
        DayCount::Thirty360,
    );
    loan.asset_type = AssetType::CommercialMortgage { ltv: None };
    loan.amortization_term_months = Some(12);
    loan.io_months = io_months;
    loan.contractual_payment = explicit_payment.then(|| usd(1_000_000.0));
    pool.assets.push(loan);
    cmbs(pool, maturity)
}

#[test]
fn zero_rate_explicit_and_inferred_payments_amortize_and_converge() {
    for explicit in [false, true] {
        let deal = zero_rate_deal(explicit, None);
        let zero = simulate(&deal);
        assert_eq!(zero.diagnostics.periods.len(), 12);
        let mut total = 0.0;
        for (i, period) in zero.diagnostics.periods.iter().enumerate() {
            let principal = period.principal_collections.amount();
            assert!(
                (principal - 1_000_000.0).abs() < 1e-6,
                "month {i}: {principal}"
            );
            assert!((period.pool_balance.amount() - (11 - i) as f64 * 1_000_000.0).abs() < 1e-6);
            total += principal;
        }
        assert!((total - 12_000_000.0).abs() < 1e-6);
        // The same dates/discount factors and nearly identical principal
        // imply continuity of principal PV and weighted average life.
        let mut small_rate_deal = deal;
        small_rate_deal.pool.assets[0].rate = 1e-10;
        let small = simulate(&small_rate_deal);
        for (zero_period, small_period) in zero
            .diagnostics
            .periods
            .iter()
            .zip(&small.diagnostics.periods)
        {
            assert!(
                (zero_period.principal_collections.amount()
                    - small_period.principal_collections.amount())
                .abs()
                    < 0.01
            );
            assert!(
                (zero_period.pool_balance.amount() - small_period.pool_balance.amount()).abs()
                    < 0.01
            );
        }
    }
}

#[test]
fn zero_rate_interest_only_window_defers_amortization() {
    let run = simulate(&zero_rate_deal(false, Some(3)));
    for (i, period) in run.diagnostics.periods.iter().enumerate() {
        let expected = if i < 3 { 0.0 } else { 12_000_000.0 / 9.0 };
        assert!(
            (period.principal_collections.amount() - expected).abs() < 1e-6,
            "month {i}: {} vs {expected}",
            period.principal_collections.amount()
        );
    }
}

#[test]
fn zero_rate_payment_scales_with_default_and_prepayment_survival() {
    let mut deal = zero_rate_deal(true, None);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(1.0 - 0.99_f64.powi(12));
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(1.0 - 0.98_f64.powi(12));
    let run = simulate(&deal);
    let mut balance = 12_000_000.0;
    let mut payment = 1_000_000.0;
    for (i, period) in run.diagnostics.periods.iter().take(11).enumerate() {
        let default = balance * 0.01;
        let scheduled = payment * 0.99;
        let prepayment = (balance - default - scheduled) * 0.02;
        balance -= default + scheduled + prepayment;
        payment *= 0.99 * 0.98;
        assert!(
            (period.defaults.amount() - default).abs() < 1e-6,
            "default month {i}"
        );
        assert!(
            (period.principal_collections.amount() - scheduled - prepayment).abs() < 1e-6,
            "principal month {i}"
        );
        assert!(
            (period.pool_balance.amount() - balance).abs() < 1e-6,
            "balance month {i}"
        );
    }
}
