//! Positional coverage tests at the deal level: an OC/IC test on class X sits
//! after X's interest tier and can only trap the coupons ranked below it.
//! The 2026-09-15 audit found a D-only OC test deferring the B and C coupons
//! because every failing test diverted the whole subordinated interest tier.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AssetPool, CoverageTestAction, CoverageTestSpec, DealType, PoolAsset,
    StructuredCredit, Tranche, TrancheCashflows, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn market(as_of: Date) -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=12)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Recoveries arrive 18 months after default, so before this date the only
/// principal a note can receive is a coverage-cure diversion.
fn first_recovery_date() -> Date {
    d(2025, 7, 1)
}

/// Five-class CLO from the audit: ten 10M bullet loans at 8%, A 60 / B 15 /
/// C 10 / D 5 / E 10 (equity), CDR 5%, recovery 40% with an 18-month lag, no
/// prepayments and no reinvestment.
fn clo(tests: Vec<CoverageTestSpec>) -> (StructuredCredit, Date) {
    let close = d(2024, 1, 1);
    let maturity = d(2032, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            0.08,
            maturity,
            DayCount::Act360,
        ));
    }
    let tr = |id: &str, a: f64, b: f64, sen: TrancheSeniority, bal: f64, cpn: f64| {
        Tranche::new(
            id,
            a,
            b,
            sen,
            usd(bal),
            TrancheCoupon::Fixed { rate: cpn },
            maturity,
        )
        .expect("tranche")
    };
    let tranches = TrancheStructure::new(vec![
        tr("A", 0.0, 60.0, TrancheSeniority::Senior, 60_000_000.0, 0.05),
        tr(
            "B",
            60.0,
            75.0,
            TrancheSeniority::Mezzanine,
            15_000_000.0,
            0.07,
        ),
        tr(
            "C",
            75.0,
            85.0,
            TrancheSeniority::Mezzanine,
            10_000_000.0,
            0.09,
        ),
        tr(
            "D",
            85.0,
            90.0,
            TrancheSeniority::Subordinated,
            5_000_000.0,
            0.12,
        ),
        tr(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            10_000_000.0,
            0.0,
        ),
    ])
    .expect("structure");
    let mut deal = StructuredCredit::new_clo(
        "CLO-COVERAGE-POSITION",
        pool,
        tranches,
        close,
        maturity,
        "USD-OIS",
    )
    .with_payment_calendar("nyse")
    .with_coverage_triggers(tests)
    .expect("coverage tests");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 18);
    (deal, close)
}

fn simulate(tests: Vec<CoverageTestSpec>) -> HashMap<String, TrancheCashflows> {
    let (deal, as_of) = clo(tests);
    let market = market(as_of);
    run_simulation(&deal, &market, as_of).expect("simulation")
}

fn first_deferral(results: &HashMap<String, TrancheCashflows>, id: &str) -> Option<Date> {
    results[id].deferred_flows.first().map(|(date, _)| *date)
}

fn first_principal(results: &HashMap<String, TrancheCashflows>, id: &str) -> Option<Date> {
    results[id]
        .principal_flows
        .iter()
        .find(|(_, amount)| amount.amount() > 0.0)
        .map(|(date, _)| *date)
}

fn flow_on(flows: &[(Date, Money)], date: Date) -> f64 {
    flows
        .iter()
        .filter(|(flow_date, _)| *flow_date == date)
        .map(|(_, amount)| amount.amount())
        .sum()
}

fn principal_on(results: &HashMap<String, TrancheCashflows>, id: &str, date: Date) -> f64 {
    flow_on(&results[id].principal_flows, date)
}

fn deferred_on(results: &HashMap<String, TrancheCashflows>, id: &str, date: Date) -> f64 {
    flow_on(&results[id].deferred_flows, date)
}

fn residual_on(results: &HashMap<String, TrancheCashflows>, date: Date) -> f64 {
    flow_on(&results["E"].interest_flows, date)
}

#[test]
fn d_only_oc_test_traps_only_the_cash_below_the_d_coupon() {
    let results = simulate(vec![CoverageTestSpec::oc("D", 1.10)]);
    let control = simulate(vec![]);

    // 100M / 90M = 1.11 at close; the 1.10 test fails once defaults erode the
    // collateral. Its position is after D's own coupon, so the cure can only
    // trap the residual, and A receives it as principal long before the first
    // recoveries arrive.
    let breach = first_principal(&results, "A").expect("the failing D test pays down A");
    assert!(
        breach < first_recovery_date(),
        "the D test must breach before recoveries start, got {breach}"
    );
    for note in ["A", "B", "C", "D"] {
        assert!(
            results[note].deferred_flows.is_empty(),
            "{note} ranks above the D test and must never be deferred, got {:?}",
            results[note].deferred_flows.first()
        );
    }
    let a_paydown = principal_on(&results, "A", breach);
    let trapped_residual = residual_on(&control, breach);
    assert!(
        trapped_residual > 0.0 && (a_paydown - trapped_residual).abs() < 1.0,
        "A must receive exactly the residual equity would have taken on {breach}: \
         paid {a_paydown}, residual {trapped_residual}"
    );
    assert!(
        residual_on(&results, breach) < 1.0,
        "equity ranks below the failing test and gets nothing on {breach}"
    );
    for junior in ["B", "C", "D"] {
        assert!(
            principal_on(&results, junior, breach) < 1.0,
            "{junior} receives no principal while the cure pays A"
        );
    }
}

#[test]
fn senior_oc_breach_traps_junior_coupons_only_up_to_the_cure() {
    let results = simulate(vec![CoverageTestSpec::oc("A", 1.65)]);
    let control = simulate(vec![]);

    // 100M / 60M = 1.667 at close; the 1.65 test fails within two quarters.
    // The test sits after A's coupon, so everything junior is at risk, but
    // only `min(interest below the test, cure)` is diverted: the most junior
    // claims are trapped first and the cure is exhausted before B's coupon.
    let breach = first_deferral(&results, "D").expect("the A test fails and defers D's coupon");
    assert!(
        breach < first_recovery_date(),
        "the A test must breach before recoveries start, got {breach}"
    );
    assert!(
        results["A"].deferred_flows.is_empty(),
        "A ranks above its own test and is never deferred"
    );
    assert_eq!(
        first_deferral(&results, "C"),
        Some(breach),
        "C ranks below the A test and is trapped from the breach on"
    );
    let b_first = first_deferral(&results, "B").expect("a deeper breach later traps B too");
    assert!(
        b_first > breach,
        "B's coupon is paid in full on {breach} because the cure is exhausted below it"
    );
    let a_paydown = principal_on(&results, "A", breach);
    let trapped = deferred_on(&results, "C", breach)
        + deferred_on(&results, "D", breach)
        + residual_on(&control, breach);
    assert!(
        a_paydown > 0.0 && (a_paydown - trapped).abs() < 1.0,
        "A's paydown on {breach} must equal the trapped C and D coupons plus the \
         residual: paid {a_paydown}, trapped {trapped}"
    );
    assert!(
        residual_on(&results, breach) < 1.0,
        "equity gets nothing on {breach}"
    );
}

#[test]
fn reinvest_action_retains_the_diverted_interest_as_principal_proceeds() {
    let pay_down = simulate(vec![CoverageTestSpec::oc("D", 1.10)]);
    let reinvest = simulate(vec![
        CoverageTestSpec::oc("D", 1.10).with_action(CoverageTestAction::Reinvest)
    ]);

    let breach = first_principal(&pay_down, "A").expect("PayDownSenior redeems A on the breach");
    assert!(
        residual_on(&reinvest, breach) < 1.0,
        "the action changes where the cash goes, not whether the residual is trapped"
    );
    assert!(
        principal_on(&reinvest, "A", breach) < 1.0,
        "Reinvest retains the trapped interest as principal proceeds instead of \
         redeeming A on the breach date, got {}",
        principal_on(&reinvest, "A", breach)
    );
    // Outside a reinvestment period the retained cash reaches A through the
    // principal tier on the following payment date.
    let next = first_principal(&reinvest, "A").expect("retained cash is repaid to A");
    assert!(
        breach < next && next < first_recovery_date(),
        "the retained cash is repaid on the next payment date after {breach}, got {next}"
    );
    assert!(
        (principal_on(&reinvest, "A", next) - principal_on(&pay_down, "A", breach)).abs() < 1.0,
        "the deferred paydown carries the trapped amount forward unchanged"
    );
}
