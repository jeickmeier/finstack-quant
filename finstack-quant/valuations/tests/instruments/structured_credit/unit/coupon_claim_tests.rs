//! Coupon claims: which coupons the template pays from principal proceeds
//! when interest falls short, and the CLO convention that mezzanine and
//! subordinated notes defer with accretion.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AssetPool, DealType, PoolAsset, StructuredCredit, Tranche, TrancheCoupon,
    TrancheSeniority, TrancheStructure,
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
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(close())
            .knots((0..=10).map(|i| (f64::from(i), (-0.04 * f64::from(i)).exp())))
            .build()
            .expect("curve"),
    )
}

fn capital_structure() -> TrancheStructure {
    TrancheStructure::new(vec![
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
            80.0,
            TrancheSeniority::Mezzanine,
            usd(20_000_000.0),
            TrancheCoupon::Fixed { rate: 0.07 },
            maturity(),
        )
        .expect("B"),
        Tranche::new(
            "E",
            80.0,
            100.0,
            TrancheSeniority::Equity,
            usd(20_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure")
}

/// Monthly ABS whose 2% collateral coupon cannot cover the 5% senior coupon,
/// let alone Class B's 7%, while 30% CPR supplies ample principal proceeds.
/// Principal covers senior interest.
fn starved_abs(tranches: TrancheStructure) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(100_000_000.0),
        0.02,
        maturity(),
        DayCount::Act360,
    ));
    let mut deal =
        StructuredCredit::new_abs("ABS-CLAIMS", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse");
    deal.fees = None;
    deal.principal_covers_senior_interest = Some(true);
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.30);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    deal
}

/// By convention only the senior coupon is covered from principal; Class B
/// marked non-deferrable is covered too, in full, from the same proceeds.
#[test]
fn a_non_deferrable_class_b_coupon_is_covered_from_principal() {
    let deferring =
        run_simulation(&starved_abs(capital_structure()), &market(), close()).expect("simulation");
    let mut covered_structure = capital_structure();
    covered_structure.tranches[1].non_deferrable = Some(true);
    let covered =
        run_simulation(&starved_abs(covered_structure), &market(), close()).expect("simulation");

    let first = |flows: &[(Date, Money)]| flows.first().map_or(0.0, |(_, amount)| amount.amount());
    // The ABS pays monthly: coupons accrue Act/360 over the first period.
    let first_date = covered["A"].interest_flows[0].0;
    let days = f64::from((first_date - close()).whole_days() as i32);
    let a_due = 60_000_000.0 * 0.05 * days / 360.0;
    let b_due = 20_000_000.0 * 0.07 * days / 360.0;
    assert!(
        (first(&deferring["A"].interest_flows) - a_due).abs() < 1_000.0,
        "the senior coupon is covered from principal by convention: {}",
        first(&deferring["A"].interest_flows)
    );
    let deferred = deferring["B"]
        .deferred_flows
        .first()
        .copied()
        .expect("a deferrable Class B defers its first coupon");
    assert_eq!(deferred.0, first_date);
    assert!(
        (deferred.1.amount() - b_due).abs() < 1_000.0,
        "the whole Class B coupon defers when interest is exhausted: {} vs {b_due}",
        deferred.1.amount()
    );
    assert!(
        deferring["B"]
            .interest_flows
            .iter()
            .all(|(date, _)| *date > first_date),
        "no Class B coupon cash in the first period"
    );
    assert!(
        (first(&covered["B"].interest_flows) - b_due).abs() < 1_000.0,
        "a non-deferrable Class B is covered in full from principal: {} vs {b_due}",
        first(&covered["B"].interest_flows)
    );
    assert!(
        first(&covered["A"].principal_flows) < first(&deferring["A"].principal_flows),
        "the principal that covers B's coupon is not paid to A"
    );
}

/// `new_clo` keeps the seniority claim convention: the senior coupon is
/// non-deferrable, the mezzanine defers as a tracked claim (`pik_enabled`
/// stays off — accreting deferred interest into the balance would report
/// write-downs above the original face).
#[test]
fn new_clo_mezzanine_defers_while_the_senior_claim_is_non_deferrable() {
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L",
        usd(100_000_000.0),
        0.08,
        maturity(),
        DayCount::Act360,
    ));
    let clo = StructuredCredit::new_clo(
        "CLO-CLAIMS",
        pool,
        capital_structure(),
        close(),
        maturity(),
        "USD-OIS",
    );
    let pik: Vec<(&str, bool)> = clo
        .tranches
        .tranches
        .iter()
        .map(|t| (t.id.as_str(), t.pik_enabled))
        .collect();
    assert_eq!(pik, vec![("A", false), ("B", false), ("E", false)]);
    assert!(clo.tranches.tranches[0].is_non_deferrable());
    assert!(!clo.tranches.tranches[1].is_non_deferrable());
}
