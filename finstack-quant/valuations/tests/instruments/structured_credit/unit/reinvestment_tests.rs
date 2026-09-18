//! Deal-level reinvestment: `pool.reinvestment_period` alone decides which
//! notes are held flat, which amortize inside the window, and what the
//! recycled principal buys. The 2026-09-15 audit found reinvestment silently
//! inert unless every note also carried per-tranche revolving flags.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AssetPool, CoverageTestAction, CoverageTestSpec, DealType,
    InstrumentCollateral, PoolAsset, ReinvestmentAssumptions, ReinvestmentCriteria,
    ReinvestmentPeriod, StructuredCredit, Tranche, TrancheCashflows, TrancheCoupon,
    TrancheSeniority, TrancheStructure,
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

const CLOSE: (i32, u8, u8) = (2024, 1, 1);
const WINDOW_END: (i32, u8, u8) = (2027, 1, 1);
const MATURITY: (i32, u8, u8) = (2032, 1, 1);

fn window(amortizing: &[&str], assumptions: Option<ReinvestmentAssumptions>) -> ReinvestmentPeriod {
    ReinvestmentPeriod {
        end_date: d(WINDOW_END.0, WINDOW_END.1, WINDOW_END.2),
        is_active: true,
        criteria: ReinvestmentCriteria::default(),
        amortizing_tranches: amortizing.iter().map(|id| (*id).to_string()).collect(),
        assumptions,
    }
}

/// Five-class CLO from the audit: ten 10M bullet loans at 8%, A 60 / B 15 /
/// C 10 / D 5 / E 10 (equity). 20% CPR supplies principal to recycle; no
/// defaults unless `cdr` says otherwise.
fn clo(
    period: Option<ReinvestmentPeriod>,
    cdr: f64,
    tests: Vec<CoverageTestSpec>,
) -> (StructuredCredit, Date) {
    let close = d(CLOSE.0, CLOSE.1, CLOSE.2);
    let maturity = d(MATURITY.0, MATURITY.1, MATURITY.2);
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
    pool.reinvestment_period = period;
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
    let mut deal =
        StructuredCredit::new_clo("CLO-REINVEST", pool, tranches, close, maturity, "USD-OIS")
            .with_payment_calendar("nyse")
            .with_coverage_triggers(tests)
            .expect("coverage tests");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(cdr);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    (deal, close)
}

fn simulate(
    period: Option<ReinvestmentPeriod>,
    cdr: f64,
    tests: Vec<CoverageTestSpec>,
) -> HashMap<String, TrancheCashflows> {
    let (deal, as_of) = clo(period, cdr, tests);
    let market = market(as_of);
    run_simulation(&deal, &market, as_of).expect("simulation")
}

fn principal_through(results: &HashMap<String, TrancheCashflows>, id: &str, date: Date) -> f64 {
    results[id]
        .principal_flows
        .iter()
        .filter(|(flow_date, _)| *flow_date <= date)
        .map(|(_, amount)| amount.amount())
        .sum()
}

fn window_end() -> Date {
    d(WINDOW_END.0, WINDOW_END.1, WINDOW_END.2)
}

#[test]
fn every_note_is_held_flat_inside_the_window_by_default() {
    let results = simulate(Some(window(&[], None)), 0.0, Vec::new());
    for note in ["A", "B", "C", "D"] {
        assert!(
            principal_through(&results, note, window_end()) < 1.0,
            "{note} must receive no principal while the deal reinvests, got {}",
            principal_through(&results, note, window_end())
        );
        assert!(
            results[note].deferred_flows.is_empty(),
            "{note} keeps its coupon on the recycled pool"
        );
    }
    let after_window =
        results["A"].total_principal.amount() - principal_through(&results, "A", window_end());
    assert!(
        after_window > 1_000_000.0,
        "A amortizes from the 20% CPR once the window closes, got {after_window}"
    );
    assert!(
        results["A"].final_balance.amount() < 1.0,
        "A is repaid in full by legal final"
    );
    // The control deal without a window pays A from the first period.
    let control = simulate(None, 0.0, Vec::new());
    assert!(
        principal_through(&control, "A", d(2024, 7, 1)) > 1_000_000.0,
        "without a window the prepayments reach A immediately"
    );
}

#[test]
fn amortizing_tranches_are_paid_down_inside_the_window() {
    let results = simulate(Some(window(&["A"], None)), 0.0, Vec::new());
    let a_in_window = principal_through(&results, "A", window_end());
    assert!(
        a_in_window > 10_000_000.0,
        "A is listed as amortizing and takes the prepayments, got {a_in_window}"
    );
    for junior in ["B", "C", "D"] {
        assert!(
            principal_through(&results, junior, window_end()) < 1.0,
            "{junior} is held flat while A amortizes"
        );
    }
    let held_flat = simulate(Some(window(&[], None)), 0.0, Vec::new());
    assert!(
        results["E"].total_interest.amount() < held_flat["E"].total_interest.amount(),
        "paying A down instead of recycling shrinks the pool and the residual: \
         amortizing {} vs held flat {}",
        results["E"].total_interest.amount(),
        held_flat["E"].total_interest.amount()
    );
}

#[test]
fn reinvestment_assumptions_change_the_replacement_collateral() {
    let pro_rata = simulate(Some(window(&[], None)), 0.0, Vec::new());
    // Replacement loans at a 3.50% fixed coupon bought at 99 with a six-year
    // tenor, against an 8% pool cloned at par.
    let assumed = simulate(
        Some(window(
            &[],
            Some(ReinvestmentAssumptions {
                spread_bp: 350.0,
                price_pct: 99.0,
                maturity_months: 72,
                index_id: None,
                coupon_floor: None,
            }),
        )),
        0.0,
        Vec::new(),
    );
    let residual_delta =
        pro_rata["E"].total_interest.amount() - assumed["E"].total_interest.amount();
    assert!(
        residual_delta > 1_000_000.0,
        "a 3.50% replacement coupon must cut the residual well below the 8% clone, \
         got pro-rata {} vs assumed {}",
        pro_rata["E"].total_interest.amount(),
        assumed["E"].total_interest.amount()
    );
    for note in ["A", "B", "C", "D"] {
        assert!(
            principal_through(&assumed, note, window_end()) < 1.0,
            "{note} is still held flat during the window"
        );
        assert!(
            assumed[note].final_balance.amount() < 1.0,
            "{note} is repaid in full: the par build at 99 and the bullets maturing \
             inside the deal's life cover the notes"
        );
    }
}

#[test]
fn synthetic_purchases_mature_no_later_than_legal_final() {
    let results = simulate(
        Some(window(
            &[],
            Some(ReinvestmentAssumptions {
                spread_bp: 800.0,
                price_pct: 100.0,
                maturity_months: 240,
                index_id: None,
                coupon_floor: None,
            }),
        )),
        0.0,
        Vec::new(),
    );
    let legal_final = d(MATURITY.0, MATURITY.1, MATURITY.2);
    for note in ["A", "B", "C", "D", "E"] {
        assert!(
            results[note].final_balance.amount() < 1.0,
            "{note} must be repaid by legal final even though the purchases were \
             booked with a twenty-year tenor, final balance {}",
            results[note].final_balance.amount()
        );
        assert!(
            results[note]
                .writedown_flows
                .iter()
                .all(|(_, amount)| amount.amount() < 1.0),
            "{note} suffers no write-down: the replacement collateral matures with the deal"
        );
        let grace = legal_final + time::Duration::days(7);
        assert!(
            results[note]
                .principal_flows
                .iter()
                .all(|(date, _)| *date <= grace),
            "{note} receives no principal after legal final"
        );
    }
}

#[test]
fn reinvest_action_routes_the_cure_into_the_window_budget() {
    // A 1.10 D test fails once 5% CDR erodes the collateral. Inside the
    // reinvestment window a `Reinvest` cure is recycled with the principal
    // proceeds, while `PayDownSenior` redeems A regardless of the hold-flat.
    let pay_down = simulate(
        Some(window(&[], None)),
        0.05,
        vec![CoverageTestSpec::oc("D", 1.10)],
    );
    let reinvest = simulate(
        Some(window(&[], None)),
        0.05,
        vec![CoverageTestSpec::oc("D", 1.10).with_action(CoverageTestAction::Reinvest)],
    );
    assert!(
        principal_through(&pay_down, "A", window_end()) > 0.0,
        "PayDownSenior redeems A inside the window from the trapped residual"
    );
    assert!(
        principal_through(&reinvest, "A", window_end()) < 1.0,
        "Reinvest keeps the trapped residual in the reinvestment budget, got {}",
        principal_through(&reinvest, "A", window_end())
    );
    // Both actions trap the same cure out of the residual on the breach
    // date; the recycled cure rebuilds par, so the test can cure and the
    // residual resume later.
    let breach = pay_down["A"]
        .principal_flows
        .iter()
        .find(|(_, amount)| amount.amount() > 0.0)
        .map(|(date, _)| *date)
        .expect("PayDownSenior redeems A on the breach date");
    assert!(
        breach <= window_end(),
        "the breach happens inside the window"
    );
    let residual_on = |run: &HashMap<String, TrancheCashflows>| -> f64 {
        run["E"]
            .interest_flows
            .iter()
            .filter(|(date, _)| *date == breach)
            .map(|(_, amount)| amount.amount())
            .sum()
    };
    let control = simulate(Some(window(&[], None)), 0.05, vec![]);
    assert!(
        residual_on(&pay_down) < residual_on(&control) - 1.0,
        "the cure is trapped out of the residual on {breach}: {} vs untested {}",
        residual_on(&pay_down),
        residual_on(&control)
    );
    assert!(
        (residual_on(&reinvest) - residual_on(&pay_down)).abs() < 1.0,
        "the action changes where the cure goes, not how much is trapped on {breach}"
    );
    assert!(
        reinvest["A"].total_interest.amount() > pay_down["A"].total_interest.amount(),
        "the recycled cure keeps A outstanding longer and earning its coupon"
    );
}

#[test]
fn instrument_collateral_pools_cannot_reinvest() {
    let close = d(CLOSE.0, CLOSE.1, CLOSE.2);
    let maturity = d(MATURITY.0, MATURITY.1, MATURITY.2);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    pool.instruments = Some(InstrumentCollateral {
        bonds: vec![Bond::example().expect("fixed bond")],
        ..Default::default()
    });
    pool.reinvestment_period = Some(window(&[], None));
    let tranches = TrancheStructure::new(vec![Tranche::new(
        "A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        usd(1_000_000.0),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity,
    )
    .expect("tranche")])
    .expect("structure");
    let deal = StructuredCredit::new_clo("CLO-INSTR", pool, tranches, close, maturity, "USD-OIS")
        .with_payment_calendar("nyse");
    let err = run_simulation(&deal, &market(close), close)
        .expect_err("instrument pools reject reinvestment periods");
    assert!(
        err.to_string()
            .contains("reinvestment_period is not supported for instrument collateral"),
        "unexpected error: {err}"
    );
}

#[test]
fn amortizing_tranches_must_name_debt_classes_of_the_deal() {
    for (ids, expected) in [
        (&["E"][..], "is the equity class"),
        (&["X"][..], "is not a tranche of the deal"),
    ] {
        let (deal, as_of) = clo(Some(window(ids, None)), 0.0, Vec::new());
        let err = run_simulation(&deal, &market(as_of), as_of)
            .expect_err("invalid amortizing tranche is rejected");
        assert!(
            err.to_string().contains(expected),
            "{ids:?}: unexpected error {err}"
        );
    }
}
