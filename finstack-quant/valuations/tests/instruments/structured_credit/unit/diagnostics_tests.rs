//! Deal-level diagnostics and equity analytics: the per-period record
//! carries the executor's own coverage ratios, the equity IRR reproduces the
//! XIRR of the equity flows (including reserve interest paid straight to the
//! residual class), and the diagnostics round-trip through JSON.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_equity_metrics, run_simulation_with_diagnostics, AssetPool, CoverageTestSpec,
    DealType, PoolAsset, ReserveInterestDestination, SimulationDiagnostics, StructuredCredit,
    Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
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
    d(2032, 1, 1)
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

/// Quarterly CLO: ten 10M 8% bullet loans, A 60M at 5% with an OC test at
/// 1.20, B 15M at 7%, E 25M; `cdr` annual defaults with 40% recovery after
/// one year, no prepayments.
fn clo(cdr: f64) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            0.08,
            maturity(),
            DayCount::Act360,
        ));
    }
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            60.0,
            TrancheSeniority::Senior,
            usd(60_000_000.0),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "B",
            60.0,
            75.0,
            TrancheSeniority::Mezzanine,
            usd(15_000_000.0),
            TrancheCoupon::Fixed { rate: 0.07 },
            maturity(),
        )
        .expect("B"),
        Tranche::new(
            "E",
            75.0,
            100.0,
            TrancheSeniority::Equity,
            usd(25_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_clo("CLO-DIAG", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse")
            .with_coverage_triggers(vec![CoverageTestSpec::oc("A", 1.20)])
            .expect("coverage test");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(cdr);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 12);
    deal
}

/// The period record is written after every payment date, carries the OC
/// ratio the executor tested (100M / 60M = 1.667 against 1.20 on a fresh
/// bullet pool) and follows the pool as it defaults.
#[test]
fn period_diagnostics_carry_the_executor_coverage_ratios_and_pool_path() {
    let clean = run_simulation_with_diagnostics(&clo(0.0), &market(), close()).expect("run");
    let periods = &clean.diagnostics.periods;
    assert_eq!(periods.len(), clean.tranches["A"].interest_flows.len());

    let first = &periods[0];
    assert_eq!(first.payment_date, clean.tranches["A"].interest_flows[0].0);
    assert!((first.pool_factor - 1.0).abs() < 1e-12);
    assert_eq!(first.coverage_tests.len(), 1);
    let oc = &first.coverage_tests[0];
    assert!(
        (oc.ratio - 100.0 / 60.0).abs() < 1e-9,
        "OC ratio {}",
        oc.ratio
    );
    assert_eq!(oc.trigger_level, 1.20);
    assert!((oc.cushion - (100.0 / 60.0 - 1.20)).abs() < 1e-9);
    assert!(oc.passing);
    assert!((first.weighted_avg_coupon - 0.08).abs() < 1e-12);
    assert_eq!(first.defaults.amount(), 0.0);
    assert!(first.interest_collections.amount() > 0.0);

    let stressed = run_simulation_with_diagnostics(&clo(0.10), &market(), close()).expect("run");
    let periods = &stressed.diagnostics.periods;
    assert!(periods[0].defaults.amount() > 0.0);
    assert!(periods
        .windows(2)
        .all(|pair| pair[1].pool_factor <= pair[0].pool_factor + 1e-12));
    // Recoveries arrive one year after the first defaults.
    assert_eq!(periods[0].recoveries.amount(), 0.0);
    assert!(periods
        .iter()
        .any(|period| period.recoveries.amount() > 0.0));
    // A failing OC test in the record is exactly a period in which the
    // executor diverted cash into class A principal.
    for period in periods {
        let diverted = stressed.tranches["A"]
            .principal_flows
            .iter()
            .any(|(date, amount)| *date == period.payment_date && amount.amount() > 0.0);
        let failing = period.coverage_tests.iter().any(|test| !test.passing);
        if failing && period.interest_collections.amount() > 0.0 {
            assert!(
                diverted,
                "failing OC on {} must divert",
                period.payment_date
            );
        }
    }
}

/// Equity IRR is the XIRR of the invested balance against every projected
/// distribution, including reserve interest routed straight to the equity
/// class; MOIC and the cash-on-cash series come from the same flows.
#[test]
fn equity_irr_reproduces_xirr_of_the_equity_flows() {
    let mut deal = clo(0.02);
    deal.pool.reserve_account = usd(5_000_000.0);
    deal.pool.reserve_account_rate = 0.05;
    deal.pool.reserve_interest_destination = ReserveInterestDestination::Tranche {
        tranche_id: "E".into(),
    };

    let run = run_simulation_with_diagnostics(&deal, &market(), close()).expect("run");
    let equity = &run.tranches["E"];
    assert!(
        run.diagnostics
            .reserve_interest_paid
            .iter()
            .any(|(_, amount)| amount.amount() > 0.0),
        "the reserve earns interest"
    );

    let metrics =
        calculate_equity_metrics(&deal, &market(), close(), None).expect("equity metrics");
    let mut flows: Vec<(Date, f64)> = vec![(close(), -25_000_000.0)];
    flows.extend(
        equity
            .cashflows
            .iter()
            .map(|(date, amount)| (*date, amount.amount())),
    );
    let expected = finstack_quant_core::cashflow::xirr(&flows, None).expect("xirr");

    assert_eq!(metrics.tranche_id, "E");
    assert_eq!(metrics.invested, 25_000_000.0);
    let irr = metrics.irr.expect("irr");
    assert!((irr - expected).abs() < 1e-12, "{irr} vs {expected}");
    let total: f64 = equity
        .cashflows
        .iter()
        .map(|(_, amount)| amount.amount())
        .sum();
    assert!((metrics.moic - total / 25_000_000.0).abs() < 1e-12);
    assert_eq!(metrics.cash_on_cash.len(), equity.cashflows.len());
    assert!(metrics.nav_pct > 0.0);

    let at_discount =
        calculate_equity_metrics(&deal, &market(), close(), Some(80.0)).expect("equity metrics");
    assert!(
        at_discount.irr.expect("irr") > irr,
        "a cheaper entry raises the IRR"
    );
    assert!((at_discount.moic - metrics.moic / 0.8).abs() < 1e-9);
}

/// Diagnostics with populated periods survive a JSON round-trip.
#[test]
fn diagnostics_round_trip_through_json() {
    let run = run_simulation_with_diagnostics(&clo(0.05), &market(), close()).expect("run");
    assert!(!run.diagnostics.periods.is_empty());

    let json = serde_json::to_string(&run.diagnostics).expect("serialize");
    let parsed: SimulationDiagnostics = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed, run.diagnostics);
}
