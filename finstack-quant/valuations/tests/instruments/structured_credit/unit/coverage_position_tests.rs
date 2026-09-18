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
    run_simulation, run_simulation_with_diagnostics, AssetPool, CoverageTestAction,
    CoverageTestSpec, DealType, PoolAsset, SimulationRun, StructuredCredit, Tranche,
    TrancheCashflows, TrancheCoupon, TrancheSeniority, TrancheStructure,
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

fn residual_on(results: &HashMap<String, TrancheCashflows>, date: Date) -> f64 {
    flow_on(&results["E"].interest_flows, date)
}

fn simulate_run(tests: Vec<CoverageTestSpec>) -> SimulationRun {
    let (deal, as_of) = clo(tests);
    let market = market(as_of);
    run_simulation_with_diagnostics(&deal, &market, as_of).expect("simulation")
}

/// Original balances of the notes, for opening-balance reconstruction.
fn original_balance(id: &str) -> f64 {
    match id {
        "A" => 60_000_000.0,
        "B" => 15_000_000.0,
        "D" => 5_000_000.0,
        _ => 10_000_000.0,
    }
}

/// Balance of `id` at the start of the period paid on `date` (par-preserving
/// notes: original balance less the principal received before that date).
fn opening_balance(results: &HashMap<String, TrancheCashflows>, id: &str, date: Date) -> f64 {
    original_balance(id)
        - results[id]
            .principal_flows
            .iter()
            .filter(|(flow_date, _)| *flow_date < date)
            .map(|(_, amount)| amount.amount())
            .sum::<f64>()
}

/// The cure the executor sized on `date`: with the period's OC ratio `r` on
/// the stack `D`, the interest-funded paydown restoring `trigger` is
/// `D − N / trigger = D × (1 − r / trigger)`.
fn cure_on(run: &SimulationRun, test_id: &str, trigger: f64, stack: &[&str], date: Date) -> f64 {
    let period = run
        .diagnostics
        .periods
        .iter()
        .find(|period| period.payment_date == date)
        .expect("period diagnostics");
    let test = period
        .coverage_tests
        .iter()
        .find(|test| test.test_id == test_id)
        .expect("test diagnostic");
    assert!(
        !test.passing,
        "{test_id} must fail on {date}, ratio {}",
        test.ratio
    );
    let denominator: f64 = stack
        .iter()
        .map(|id| opening_balance(&run.tranches, id, date))
        .sum();
    (denominator * (1.0 - test.ratio / trigger)).max(0.0)
}

fn first_failure(run: &SimulationRun, test_id: &str) -> Date {
    run.diagnostics
        .periods
        .iter()
        .find(|period| {
            period
                .coverage_tests
                .iter()
                .any(|test| test.test_id == test_id && !test.passing)
        })
        .map(|period| period.payment_date)
        .expect("the test fails at some point")
}

#[test]
fn d_only_oc_test_traps_only_the_cash_below_the_d_coupon() {
    let run = simulate_run(vec![CoverageTestSpec::oc("D", 1.10)]);
    let results = &run.tranches;
    let control = simulate(vec![]);

    // 100M / 90M = 1.11 at close; the 1.10 test fails once defaults erode the
    // collateral. Its position is after D's own coupon, so the cure can only
    // trap the residual, and A receives it as principal long before the first
    // recoveries arrive. Only the cure is diverted; the rest of the residual
    // still reaches equity.
    let breach = first_principal(results, "A").expect("the failing D test pays down A");
    assert_eq!(breach, first_failure(&run, "OC_D"));
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
    let a_paydown = principal_on(results, "A", breach);
    let residual = residual_on(&control, breach);
    let cure = cure_on(&run, "OC_D", 1.10, &["A", "B", "C", "D"], breach);
    assert!(
        cure > 0.0 && (a_paydown - cure.min(residual)).abs() < 1.0,
        "A must receive the cure (capped at the residual) on {breach}: paid {a_paydown}, \
         cure {cure}, residual {residual}"
    );
    assert!(
        (residual_on(results, breach) - (residual - a_paydown)).abs() < 1.0,
        "equity keeps the residual left after the cure on {breach}: got {}, expected {}",
        residual_on(results, breach),
        residual - a_paydown
    );
    for junior in ["B", "C", "D"] {
        assert!(
            principal_on(results, junior, breach) < 1.0,
            "{junior} receives no principal while the cure pays A"
        );
    }
}

#[test]
fn senior_oc_breach_traps_junior_coupons_only_up_to_the_cure() {
    let run = simulate_run(vec![CoverageTestSpec::oc("A", 1.65)]);
    let results = &run.tranches;
    let control = simulate(vec![]);

    // 100M / 60M = 1.667 at close; the 1.65 test fails within two quarters.
    // The test sits after A's coupon, so everything junior is at risk, but
    // only `min(interest below the test, cure)` is diverted: the residual is
    // trapped first, then the most junior coupons.
    let breach = first_failure(&run, "OC_A");
    assert!(
        breach < first_recovery_date(),
        "the A test must breach before recoveries start, got {breach}"
    );
    assert!(
        results["A"].deferred_flows.is_empty(),
        "A ranks above its own test and is never deferred"
    );
    let interest_below = residual_on(&control, breach)
        + flow_on(&control["C"].interest_flows, breach)
        + flow_on(&control["D"].interest_flows, breach);
    let cure = cure_on(&run, "OC_A", 1.65, &["A"], breach);
    let a_paydown = principal_on(results, "A", breach);
    assert!(
        a_paydown > 0.0 && (a_paydown - cure.min(interest_below)).abs() < 1.0,
        "A's paydown on {breach} must be the cure capped at the interest below the test: \
         paid {a_paydown}, cure {cure}, interest below {interest_below}"
    );
    // As defaults accumulate the cure outgrows the residual and traps the
    // coupons below the test, most junior first.
    let d_first = first_deferral(results, "D").expect("a deeper breach later traps D's coupon");
    assert!(
        first_deferral(results, "C").is_none_or(|c_first| c_first >= d_first),
        "C is trapped no earlier than D"
    );
    assert!(
        first_deferral(results, "B").is_none_or(|b_first| b_first >= d_first),
        "B is trapped no earlier than D"
    );
    // On that date the cure paid to A (its principal beyond the recovery
    // principal both runs share) is the cure capped at everything ranked
    // below the test; the earlier cures shrank A's coupon, so more interest
    // sits below the test than in the control run.
    let cure_paid = principal_on(results, "A", d_first) - principal_on(&control, "A", d_first);
    let interest_below_d = residual_on(&control, d_first)
        + flow_on(&control["B"].interest_flows, d_first)
        + flow_on(&control["C"].interest_flows, d_first)
        + flow_on(&control["D"].interest_flows, d_first)
        + (flow_on(&control["A"].interest_flows, d_first)
            - flow_on(&results["A"].interest_flows, d_first));
    let cure_on_d = cure_on(&run, "OC_A", 1.65, &["A"], d_first);
    assert!(
        (cure_paid - cure_on_d.min(interest_below_d)).abs() < 1.0,
        "on {d_first} the cure paid {cure_paid} must be the cure {cure_on_d} capped at the \
         interest below the test {interest_below_d}"
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
        (residual_on(&reinvest, breach) - residual_on(&pay_down, breach)).abs() < 1.0,
        "the action changes where the cash goes, not how much is trapped"
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
