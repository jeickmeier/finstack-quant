//! Coverage rules: the OC numerator must move when the deal carries
//! collateral valuation rules (excess-CCC bucket, discount obligations,
//! defaulted-asset valuation), and deal-level rules must reach the waterfall
//! the engine runs. The 2026-09-15 audit found the previous
//! `CoverageTestConfig` haircut maps silently inert.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CreditRating, InstrumentId};
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, run_simulation_with_diagnostics, AssetPool, CccBucketRule, CoverageRules,
    CoverageTest, CoverageTestSpec, DealType, DefaultedValuation, DiscountObligationRule,
    InstrumentCollateral, PoolAsset, StructuredCredit, TestContext, Tranche, TrancheCashflows,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;
use time::Month;

use super::instrument_pool_tests::{deal_with, DealSpec};
use crate::common::test_helpers::flat_discount_curve;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn maturity() -> Date {
    d(2032, 1, 1)
}

fn loan(id: &str, balance: f64, rating: CreditRating) -> PoolAsset {
    let mut asset =
        PoolAsset::fixed_rate_bond(id, usd(balance), 0.08, maturity(), DayCount::Act360);
    asset.credit_quality = Some(rating);
    asset
}

/// 100M pool: nine 10M BB loans and one 10M CCC loan marked at 60.
fn pool_with_ccc() -> AssetPool {
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..9 {
        pool.assets
            .push(loan(&format!("L{i}"), 10_000_000.0, CreditRating::BB));
    }
    let mut ccc = loan("L9", 10_000_000.0, CreditRating::CCC);
    ccc.market_price_pct = Some(60.0);
    pool.assets.push(ccc);
    pool
}

fn senior(balance: f64) -> TrancheStructure {
    TrancheStructure::new(vec![Tranche::new(
        "A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        usd(balance),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity(),
    )
    .expect("tranche")])
    .expect("structure")
}

/// OC ratio of class A at par, no cash, evaluated directly through the test.
fn oc_ratio(pool: &AssetPool, tranches: &TrancheStructure, rules: Option<&CoverageRules>) -> f64 {
    let caps: HashMap<&str, Option<f64>> = HashMap::default();
    let ctx = TestContext {
        pool,
        tranches,
        tranche_id: "A",
        as_of: d(2025, 1, 1),
        valuation_date: d(2025, 1, 1),
        period_start: None,
        cash_balance: usd(0.0),
        interest_collections: usd(0.0),
        rules,
        market: None,
        tranche_balances: None,
        payable_principal_tranche_ids: None,
        asset_balances: None,
        live_collateral: None,
        current_pool_balance: None,
        senior_fees: usd(0.0),
        restricted_cash: usd(0.0),
        defaulted_collateral_value: usd(0.0),
        interest_claim_caps: &caps,
        floating_rate_shift: 0.0,
        deferred_interest: None,
    };
    CoverageTest::new_oc(1.0)
        .calculate(&ctx)
        .expect("oc test")
        .current_ratio
}

fn ccc_rules(carry_at_market_value: bool) -> CoverageRules {
    CoverageRules {
        ccc_bucket: Some(CccBucketRule {
            threshold_pct: 7.5,
            carry_at_market_value,
        }),
        ..Default::default()
    }
}

/// 10M of CCC in a 100M pool with a 7.5% bucket: the 2.5M above the bucket is
/// carried at the 60 market price, so the numerator is 100M − 2.5M × 0.4 = 99M
/// and the OC ratio on a 50M senior note falls from 2.00 to 1.98.
#[test]
fn excess_ccc_is_carried_at_market_value() {
    let pool = pool_with_ccc();
    let tranches = senior(50_000_000.0);

    let at_par = oc_ratio(&pool, &tranches, None);
    let with_bucket = oc_ratio(&pool, &tranches, Some(&ccc_rules(true)));

    assert!((at_par - 2.0).abs() < 1e-12, "par OC {at_par}");
    assert_ne!(at_par, with_bucket, "the CCC bucket must move the OC ratio");
    assert!(
        (with_bucket - 1.98).abs() < 1e-9,
        "excess CCC at market value: expected 99M / 50M = 1.98, got {with_bucket}"
    );
}

/// Without the market-value carry the excess 2.5M is excluded outright:
/// 97.5M / 50M = 1.95.
#[test]
fn excess_ccc_is_excluded_when_not_carried_at_market_value() {
    let pool = pool_with_ccc();
    let tranches = senior(50_000_000.0);

    let with_bucket = oc_ratio(&pool, &tranches, Some(&ccc_rules(false)));

    assert!(
        (with_bucket - 1.95).abs() < 1e-9,
        "excess CCC excluded: expected 97.5M / 50M = 1.95, got {with_bucket}"
    );
}

/// CCC collateral inside the bucket is carried at par.
#[test]
fn ccc_within_the_bucket_is_carried_at_par() {
    let pool = pool_with_ccc();
    let tranches = senior(50_000_000.0);
    let rules = CoverageRules {
        ccc_bucket: Some(CccBucketRule {
            threshold_pct: 10.0,
            carry_at_market_value: true,
        }),
        ..Default::default()
    };

    let ratio = oc_ratio(&pool, &tranches, Some(&rules));

    assert!(
        (ratio - 2.0).abs() < 1e-12,
        "10% CCC in a 10% bucket: {ratio}"
    );
}

/// An asset bought at 70 sits below the 80 discount-obligation threshold and
/// is carried at its purchase price; one bought at 90 stays at par. Two 10M
/// assets therefore count 7M + 10M = 17M against a 10M note: 1.70, not 2.00.
#[test]
fn discount_obligations_are_carried_at_purchase_price() {
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    let mut cheap = loan("CHEAP", 10_000_000.0, CreditRating::BB);
    cheap.purchase_price = Some(usd(7_000_000.0));
    let mut near_par = loan("PAR", 10_000_000.0, CreditRating::BB);
    near_par.purchase_price = Some(usd(9_000_000.0));
    pool.assets.push(cheap);
    pool.assets.push(near_par);
    let tranches = senior(10_000_000.0);
    let rules = CoverageRules {
        discount_obligation: Some(DiscountObligationRule {
            price_threshold_pct: 80.0,
        }),
        ..Default::default()
    };

    let at_par = oc_ratio(&pool, &tranches, None);
    let with_rule = oc_ratio(&pool, &tranches, Some(&rules));

    assert!((at_par - 2.0).abs() < 1e-12, "par OC {at_par}");
    assert!(
        (with_rule - 1.7).abs() < 1e-9,
        "discount obligation at purchase price: expected 17M / 10M = 1.70, got {with_rule}"
    );
}

/// Empty rules are the par convention: identical to no rules at all.
#[test]
fn default_rules_value_collateral_at_par() {
    let pool = pool_with_ccc();
    let tranches = senior(50_000_000.0);
    let rules = CoverageRules::default();

    assert!(rules.is_empty());
    assert!(!rules.adjusts_collateral());
    assert_eq!(
        oc_ratio(&pool, &tranches, None),
        oc_ratio(&pool, &tranches, Some(&rules))
    );
}

/// The standard CLO convention carries performing collateral at par (no
/// rating haircuts), a 7.5% CCC bucket at market value and an 80
/// discount-obligation threshold, and validates.
#[test]
fn clo_standard_rules_are_populated_and_valid() {
    let rules = CoverageRules::clo_standard();

    assert!(!rules.is_empty());
    assert!(rules.adjusts_collateral());
    assert!(
        rules.rating_haircuts.is_empty(),
        "the par-value test carries performing collateral at par"
    );
    assert_eq!(rules.defaulted_valuation, DefaultedValuation::Recovery);
    let bucket = rules.ccc_bucket.expect("CCC bucket");
    assert_eq!(bucket.threshold_pct, 7.5);
    assert!(bucket.carry_at_market_value);
    assert_eq!(
        rules
            .discount_obligation
            .expect("discount rule")
            .price_threshold_pct,
        80.0
    );

    let mut deal = StructuredCredit::example();
    deal.coverage_rules = Some(rules);
    deal.validate_for_pricing()
        .expect("standard rules validate");
}

/// Percent fields outside `[0, 100]` are rejected at validation.
#[test]
fn out_of_range_rules_are_rejected() {
    let mut deal = StructuredCredit::example();
    deal.coverage_rules = Some(CoverageRules {
        ccc_bucket: Some(CccBucketRule {
            threshold_pct: 150.0,
            carry_at_market_value: true,
        }),
        ..Default::default()
    });

    assert!(deal.validate_for_pricing().is_err());
}

/// Deal-level rules are attached to the waterfall the constructors generate.
#[test]
fn deal_rules_reach_the_generated_waterfall() {
    let mut deal = StructuredCredit::example();
    assert!(deal
        .create_waterfall()
        .expect("waterfall")
        .coverage_rules
        .is_none());

    deal.coverage_rules = Some(CoverageRules::clo_standard());
    let waterfall = deal.create_waterfall().expect("waterfall");

    assert_eq!(waterfall.coverage_rules, deal.coverage_rules);
}

// ---------------------------------------------------------------------------
// Defaulted-asset valuation through the engine
// ---------------------------------------------------------------------------

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

/// Three-class CLO: nine performing 10M bullet loans at 8% plus one 10M loan
/// that defaulted a month before closing with a 4M recovery still pending,
/// A 60 / B 15 / E 25 (equity), no further defaults or prepayments, and an
/// OC test on A at 1.54.
fn clo_with_closing_default(valuation: DefaultedValuation) -> (StructuredCredit, Date) {
    let close = d(2024, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..9 {
        pool.assets
            .push(loan(&format!("L{i}"), 10_000_000.0, CreditRating::BB));
    }
    let mut defaulted = loan("L9", 10_000_000.0, CreditRating::D);
    defaulted.is_defaulted = true;
    defaulted.default_date = Some(d(2023, 12, 1));
    defaulted.recovery_amount = Some(usd(4_000_000.0));
    pool.assets.push(defaulted);
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
    let mut deal = StructuredCredit::new_clo(
        "CLO-DEFAULTED-VALUATION",
        pool,
        tranches,
        close,
        maturity(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse")
    .with_coverage_triggers(vec![CoverageTestSpec::oc("A", 1.54)])
    .expect("coverage test");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 18);
    deal.coverage_rules = Some(CoverageRules {
        defaulted_valuation: valuation,
        ..Default::default()
    });
    (deal, close)
}

fn principal_on(results: &HashMap<String, TrancheCashflows>, id: &str, date: Date) -> f64 {
    results[id]
        .principal_flows
        .iter()
        .filter(|(flow_date, _)| *flow_date == date)
        .map(|(_, amount)| amount.amount())
        .sum()
}

/// The pending 4M recovery counts in full under the recovery convention
/// (94M / 60M = 1.567, passes 1.54) but only 20% of the 10M defaulted par
/// under the market-value convention (92M / 60M = 1.533, fails). The cure is
/// paid from interest, which is not in the numerator, so X solves
/// N / (D − X) = T: X = 60M − 92M / 1.54 = 259,740.26, diverted into class A
/// principal on the first payment date (inside the ≈1.06M of interest ranking
/// below the A coupon, so the diversion is the cure and not the cap). The
/// passing deal pays no principal until the recovery cash itself arrives.
#[test]
fn defaulted_collateral_at_market_value_fails_the_test_that_recovery_value_passes() {
    let (recovery, as_of) = clo_with_closing_default(DefaultedValuation::Recovery);
    let (market_value, _) = clo_with_closing_default(DefaultedValuation::MarketValue { pct: 20.0 });
    let market = market(as_of);
    let first_payment = d(2024, 4, 1);

    let recovery_run = run_simulation(&recovery, &market, as_of).expect("recovery run");
    let market_value_run = run_simulation(&market_value, &market, as_of).expect("market run");

    let recovery_cure = principal_on(&recovery_run, "A", first_payment);
    let market_value_cure = principal_on(&market_value_run, "A", first_payment);

    assert_ne!(
        recovery_cure, market_value_cure,
        "defaulted-asset valuation must move the OC cure"
    );
    assert_eq!(recovery_cure, 0.0, "recovery basis passes: no cure");
    assert!(
        (market_value_cure - 259_740.26).abs() < 1.0,
        "market-value basis cure: expected 60M − 92M / 1.54 = 259,740.26, got {market_value_cure}"
    );
}

// ---------------------------------------------------------------------------
// Instrument collateral
// ---------------------------------------------------------------------------

/// A 1M 8% bond financed by a 900k senior note and 100k of equity, with an OC
/// test on the note at 1.05 (1.11 at par).
fn bond_deal(rules: Option<CoverageRules>) -> StructuredCredit {
    let mut bond = Bond::example().expect("bond");
    bond.id = InstrumentId::new("B-8");
    bond.cashflow_spec =
        CashflowSpec::fixed(0.08, Tenor::semi_annual(), DayCount::Thirty360).expect("coupon");
    let mut deal = deal_with(DealSpec {
        collateral: InstrumentCollateral {
            bonds: vec![bond],
            ..Default::default()
        },
        reserve: 0.0,
        reserve_target: None,
        closing: date!(2024 - 01 - 15),
        maturity: date!(2034 - 01 - 15),
        senior: 900_000.0,
        equity: 100_000.0,
    })
    .with_coverage_triggers(vec![CoverageTestSpec::oc("A", 1.05)])
    .expect("oc test");
    deal.coverage_rules = rules;
    deal
}

/// Materialized instrument rows are unrated by construction, not by credit,
/// so an `NR` haircut leaves them at par: the OC test passes exactly as it
/// does without rules and senior principal waits for the bullet maturity.
#[test]
fn nr_haircut_leaves_instrument_collateral_at_par() {
    let closing = date!(2024 - 01 - 15);
    let market = MarketContext::new().insert(flat_discount_curve(0.03, closing, "USD-OIS"));
    let first_principal = |deal: &StructuredCredit| -> Date {
        let run = run_simulation_with_diagnostics(deal, &market, closing).expect("run");
        run.tranches["A"]
            .principal_flows
            .iter()
            .find(|(_, amount)| amount.amount() > 0.0)
            .map(|(date, _)| *date)
            .expect("senior principal")
    };

    let at_par = first_principal(&bond_deal(None));
    let haircut = first_principal(&bond_deal(Some(CoverageRules {
        rating_haircuts: [(CreditRating::NR, 0.5)].into_iter().collect(),
        ..Default::default()
    })));

    assert!(at_par >= date!(2034 - 01 - 15), "bullet at par: {at_par}");
    assert_eq!(
        haircut, at_par,
        "an NR haircut does not apply to unrated instrument rows: {haircut} vs {at_par}"
    );
}
