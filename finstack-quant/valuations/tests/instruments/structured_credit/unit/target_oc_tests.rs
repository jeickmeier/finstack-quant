//! Targeted-overcollateralization amortization: the notes are paid down to
//! the amount that holds the pool's OC at the target, from interest first,
//! and the collections above the requirement are released to equity.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, AssetType, DealType, PoolAsset, SimulationRun,
    StructuredCredit, TargetOcSpec, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    WaterfallRules,
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

/// Monthly auto ABS: ten 1M five-year level-pay 8% auto loans, class A of
/// `senior` at 4%, equity for the rest, no fees, no prepayments or defaults,
/// target OC 12% of current with a 1.5%-of-original floor.
fn auto_abs(senior: f64) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    for i in 0..10 {
        let mut loan = PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(1_000_000.0),
            0.08,
            maturity(),
            DayCount::Thirty360,
        );
        loan.asset_type = AssetType::NewAutoLoan { ltv: None };
        pool.assets.push(loan);
    }
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            senior / 100_000.0,
            TrancheSeniority::Senior,
            usd(senior),
            TrancheCoupon::Fixed { rate: 0.04 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "E",
            senior / 100_000.0,
            100.0,
            TrancheSeniority::Equity,
            usd(10_000_000.0 - senior),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal = StructuredCredit::new_abs(
        "ABS-TARGET-OC",
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
    deal.waterfall_rules = Some(WaterfallRules {
        target_oc: Some(TargetOcSpec {
            pct_of_current: 0.12,
            floor_pct_of_original: 0.015,
        }),
        ..Default::default()
    });
    deal
}

fn simulate(deal: &StructuredCredit) -> SimulationRun {
    run_simulation_with_diagnostics(deal, &market(), close()).expect("simulation")
}

fn on(run: &SimulationRun, id: &str, date: Date) -> (f64, f64) {
    let flows = &run.tranches[id];
    let sum = |flows: &[(Date, Money)]| -> f64 {
        flows
            .iter()
            .filter(|(flow_date, _)| *flow_date == date)
            .map(|(_, amount)| amount.amount())
            .sum()
    };
    (sum(&flows.interest_flows), sum(&flows.principal_flows))
}

/// Regular principal distribution amount, period one, from the diagnostics:
/// the pool after collections, the 12% target and the floor.
fn required(run: &SimulationRun, notes: f64) -> f64 {
    let closing_pool = run.diagnostics.periods[0].pool_balance.amount();
    let target = (0.12 * closing_pool).max(0.015 * 10_000_000.0);
    (notes - (closing_pool - target).max(0.0)).max(0.0)
}

/// With 12% OC already in place (A 8.8M on a 10M pool) the period-one
/// requirement is below the scheduled collections: A receives exactly the
/// hand-computed amount and equity the released remainder plus the excess
/// interest.
#[test]
fn notes_receive_the_regular_principal_distribution_amount_and_equity_the_release() {
    let run = simulate(&auto_abs(8_800_000.0));
    let first = run.diagnostics.periods[0].payment_date;
    let collections = run.diagnostics.periods[0].principal_collections.amount();
    let expected = required(&run, 8_800_000.0);
    assert!(
        expected > 1_000.0 && expected < collections,
        "the requirement is positive and inside the period's collections: {expected} vs {collections}"
    );
    let (a_interest, a_principal) = on(&run, "A", first);
    assert!(
        (a_principal - expected).abs() < 1.0,
        "A is paid down to the target: {a_principal} vs {expected}"
    );
    let pool_interest = run.diagnostics.periods[0].interest_collections.amount();
    let (e_interest, e_principal) = on(&run, "E", first);
    assert!(
        (e_interest + e_principal - (pool_interest - a_interest + collections - expected)).abs()
            < 1.0,
        "equity receives the excess interest and the released collections: {} vs {}",
        e_interest + e_principal,
        pool_interest - a_interest + collections - expected
    );
    // The requirement is paid from interest first: A's principal exceeds the
    // interest left after its coupon only when collections are needed.
    assert!(a_principal > pool_interest - a_interest);
}

/// With 10% OC (A 9M) the period-one requirement exceeds the collections:
/// every dollar of excess interest turbos the notes and equity gets nothing.
#[test]
fn a_requirement_above_the_collections_turbos_from_excess_interest() {
    let run = simulate(&auto_abs(9_000_000.0));
    let first = run.diagnostics.periods[0].payment_date;
    let collections = run.diagnostics.periods[0].principal_collections.amount();
    let expected = required(&run, 9_000_000.0);
    assert!(expected > collections, "{expected} vs {collections}");
    let pool_interest = run.diagnostics.periods[0].interest_collections.amount();
    let (a_interest, a_principal) = on(&run, "A", first);
    assert!(
        (a_principal - (collections + pool_interest - a_interest)).abs() < 1.0,
        "A takes the collections plus all excess interest: {a_principal}"
    );
    let (e_interest, e_principal) = on(&run, "E", first);
    assert!(
        e_interest + e_principal < 1.0,
        "nothing reaches equity while the target is unmet: {}",
        e_interest + e_principal
    );
    // Once the target is restored the notes receive only the requirement and
    // the release resumes.
    let later = run.diagnostics.periods[12].payment_date;
    let (_, e_later) = on(&run, "E", later);
    let (e_later_interest, _) = on(&run, "E", later);
    assert!(
        e_later + e_later_interest > 1_000.0,
        "equity receives releases once the OC target is met: {}",
        e_later + e_later_interest
    );
}
