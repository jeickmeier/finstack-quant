//! Available-funds cap with net-WAC carryover: the coupon the cap withholds
//! accrues and is repaid from excess interest ahead of equity. The
//! 2026-09-17 audit found no carryover mechanism at all.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AfcSpec, AssetPool, DealType, PoolAsset, SimulationRun,
    StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure, WaterfallRules,
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
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(close())
        .knots(vec![(0.0, 1.0), (5.0, 0.90)])
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Pool of 1M at 5% (30/360 bullet); SR 800k fixed at 6% capped at the
/// 5% collateral WAC; EQ 200k. The pool's 4,166.67 of monthly interest
/// covers the capped coupon and leaves excess for the carryover.
fn deal(carryover: bool) -> StructuredCredit {
    let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        usd(1_000_000.0),
        0.05,
        maturity(),
        DayCount::Thirty360,
    ));
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "SR",
            0.0,
            80.0,
            TrancheSeniority::Senior,
            usd(800_000.0),
            TrancheCoupon::Fixed { rate: 0.06 },
            maturity(),
        )
        .expect("SR"),
        Tranche::new(
            "EQ",
            80.0,
            100.0,
            TrancheSeniority::Equity,
            usd(200_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("EQ"),
    ])
    .expect("structure");
    let mut sc =
        StructuredCredit::new_abs("ABS-AFC", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse");
    sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
    sc.waterfall_rules = Some(WaterfallRules {
        afc: Some(AfcSpec {
            capped_tranches: vec!["SR".to_string()],
            net_wac_fee_bp: None,
            carryover,
        }),
        ..Default::default()
    });
    sc
}

fn simulate(deal: &StructuredCredit) -> SimulationRun {
    run_simulation_with_diagnostics(deal, &market(), close()).expect("simulation")
}

fn interest_on(run: &SimulationRun, id: &str, date: Date) -> f64 {
    run.tranches[id]
        .interest_flows
        .iter()
        .filter(|(flow_date, _)| *flow_date == date)
        .map(|(_, amount)| amount.amount())
        .sum()
}

/// The 1% the cap withholds in period one (on the note's own accrual)
/// is repaid in period two from the excess interest ahead of equity, so the
/// senior receives its capped coupon plus the prior carryover and equity the
/// remainder; without the carryover the senior only ever sees the capped
/// coupon.
#[test]
fn withheld_interest_accrues_as_a_carryover_and_is_repaid_from_excess_before_equity() {
    let with = simulate(&deal(true));
    let without = simulate(&deal(false));
    let dates: Vec<Date> = with
        .diagnostics
        .periods
        .iter()
        .take(3)
        .map(|period| period.payment_date)
        .collect();
    let accrual = |from: Date, to: Date| {
        DayCount::Act360
            .year_fraction(from, to, DayCountContext::default())
            .expect("accrual")
    };
    let capped_1 = 800_000.0 * 0.05 * accrual(close(), dates[0]);
    let withheld_1 = 800_000.0 * 0.01 * accrual(close(), dates[0]);
    let capped_2 = 800_000.0 * 0.05 * accrual(dates[0], dates[1]);
    let withheld_2 = 800_000.0 * 0.01 * accrual(dates[0], dates[1]);
    let pool_interest = 1_000_000.0 * 0.05 / 12.0;

    assert!(
        (interest_on(&with, "SR", dates[0]) - capped_1).abs() < 0.01,
        "period one pays the capped coupon only: {}",
        interest_on(&with, "SR", dates[0])
    );
    assert!(
        (interest_on(&with, "EQ", dates[0]) - (pool_interest - capped_1)).abs() < 0.01,
        "period one excess reaches equity: {}",
        interest_on(&with, "EQ", dates[0])
    );
    assert!(
        (interest_on(&with, "SR", dates[1]) - (capped_2 + withheld_1)).abs() < 0.01,
        "period two repays the period-one carryover: {} vs {}",
        interest_on(&with, "SR", dates[1]),
        capped_2 + withheld_1
    );
    assert!(
        (interest_on(&with, "EQ", dates[1]) - (pool_interest - capped_2 - withheld_1)).abs() < 0.01,
        "equity is behind the carryover: {}",
        interest_on(&with, "EQ", dates[1])
    );
    assert!(
        (interest_on(&with, "SR", dates[2])
            - (800_000.0 * 0.05 * accrual(dates[1], dates[2]) + withheld_2))
            .abs()
            < 0.01,
        "each period repays the previous period's carryover"
    );
    assert!(
        (interest_on(&without, "SR", dates[1]) - capped_2).abs() < 0.01,
        "without the carryover the capped-off coupon is never owed: {}",
        interest_on(&without, "SR", dates[1])
    );
    assert!(
        with.tranches["SR"].total_interest.amount()
            > without.tranches["SR"].total_interest.amount() + 30_000.0,
        "over the deal the carryover repays about 1% a year on 800k"
    );
    assert_eq!(
        with.tranches["SR"].total_deferred.amount(),
        0.0,
        "the carryover is not a deferred coupon"
    );
}
