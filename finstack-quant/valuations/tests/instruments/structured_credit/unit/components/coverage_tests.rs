//! Unit tests for OC/IC coverage test calculations.
//!
//! Tests cover:
//! - OC test calculation logic
//! - IC test calculation logic
//! - Passing/failing scenarios
//! - Cure amount calculations
//! - Edge cases and boundary conditions

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CoverageTest, DealType, PoolAsset, TestContext, Tranche, TrancheCoupon,
    TrancheSeniority, TrancheStructure,
};
use time::Month;

/// Every tranche in this file owes its full, uncapped coupon. The engine
/// always supplies a spec-derived claim map, so the tests state it explicitly.
static UNCAPPED_CLAIMS: std::sync::LazyLock<
    finstack_quant_core::HashMap<&'static str, Option<f64>>,
> = std::sync::LazyLock::new(|| {
    ["SENIOR", "EQUITY", "EMPTY", "POOL", "TEST_TRANCHE"]
        .into_iter()
        .map(|id| (id, None))
        .collect()
});

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

fn context_for_tranche<'a>(
    pool: &'a AssetPool,
    tranches: &'a TrancheStructure,
    tranche_id: &'a str,
    cash_balance: Money,
    interest_collections: Money,
) -> TestContext<'a> {
    TestContext {
        pool,
        tranches,
        tranche_id,
        as_of: test_date(),
        valuation_date: test_date(),
        period_start: None,
        cash_balance,
        interest_collections,
        rules: None,
        market: None,
        tranche_balances: None,
        payable_principal_tranche_ids: None,
        asset_balances: None,
        live_collateral: None,
        current_pool_balance: None,
        senior_fees: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        restricted_cash: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        defaulted_collateral_value: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        interest_claim_caps: &UNCAPPED_CLAIMS,
        floating_rate_shift: 0.0,
        deferred_interest: None,
    }
}

// OC Test Calculation Tests

#[test]
fn test_oc_test_passing_scenario() {
    // Arrange: AssetPool value > required multiple of tranche
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(125_000_000.0, Currency::USD).expect("valid money fixture"),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: 125M / 100M = 1.25 (exactly at threshold, should pass)
    assert!(result.is_passing);
    assert_eq!(result.tranche_id, "SENIOR");
    assert!((result.current_ratio - 1.25).abs() < 0.01);
}

#[test]
fn test_coverage_test_result_preserves_tranche_id_with_underscore() {
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Act360,
    ));

    let tranche = Tranche::new(
        "CLASS_A_1",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();
    let tranches = TrancheStructure::new(vec![tranche]).unwrap();
    let context = context_for_tranche(
        &pool,
        &tranches,
        "CLASS_A_1",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
    );

    let result = CoverageTest::new_oc(1.25)
        .calculate(&context)
        .expect("coverage calculation");

    assert_eq!(result.test_id, "oc_test_125");
    assert_eq!(result.tranche_id, "CLASS_A_1");
    assert!(result.cure_amount.is_some());
}

#[test]
fn test_oc_test_failing_scenario() {
    // Arrange: AssetPool value < required multiple
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(120_000_000.0, Currency::USD).expect("valid money fixture"),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: 120M / 100M = 1.20 < 1.25 (failing)
    assert!(!result.is_passing);
    assert!((result.current_ratio - 1.20).abs() < 0.01);
}

#[test]
fn test_oc_test_with_cash_balance() {
    // Arrange: AssetPool + cash should pass
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(120_000_000.0, Currency::USD).expect("valid money fixture"),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: (120M + 5M) / 100M = 1.25 (passing)
    assert!(result.is_passing);
}

/// Defaulted collateral is carried at its modeled recovery value until the
/// recovery cash arrives (CLO par-OC convention), so the numerator rises by
/// exactly the pending recovery claim and a default costs the test its
/// expected loss rather than the whole defaulted par.
#[test]
fn oc_numerator_carries_pending_recovery_claims_at_recovery_value() {
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(115_000_000.0, Currency::USD).expect("valid money fixture"),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Act360,
    ));
    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();
    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();
    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();
    let zero = Money::new(0.0, Currency::USD).expect("valid money fixture");

    // 115M performing par against 100M senior: 1.15, below the 1.20 trigger.
    let without_claims = CoverageTest::new_oc(1.20)
        .calculate(&context_for_tranche(&pool, &tranches, "SENIOR", zero, zero))
        .expect("coverage calculation");
    assert!(!without_claims.is_passing);
    assert!((without_claims.current_ratio - 1.15).abs() < 1e-9);

    // A 5M pending recovery claim (12.5M defaulted at 40%) is collateral.
    let mut context = context_for_tranche(&pool, &tranches, "SENIOR", zero, zero);
    context.defaulted_collateral_value =
        Money::new(5_000_000.0, Currency::USD).expect("valid money fixture");
    let with_claims = CoverageTest::new_oc(1.20)
        .calculate(&context)
        .expect("coverage calculation");
    assert!(
        (with_claims.current_ratio - 1.20).abs() < 1e-9,
        "pending recovery claims raise the OC numerator by their value: got {}",
        with_claims.current_ratio
    );
    assert!(with_claims.is_passing);
}

#[test]
fn test_oc_test_cure_amount_calculation() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(115_000_000.0, Currency::USD).expect("valid money fixture"),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Cure amount = interest diverted to pay down notes; interest is not in
    // the OC numerator, so only the denominator moves:
    // 115M / (100M - X) = 1.25 => X = 8M.
    assert!(!result.is_passing);
    assert!(result.cure_amount.is_some());
    assert!((result.cure_amount.unwrap().amount() - 8_000_000.0).abs() < 1.0);
}

// IC Test Calculation Tests

#[test]
fn test_ic_test_passing_scenario() {
    // Arrange: Interest collections > required multiple of interest due
    let pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 }, // 5% = 1.25M quarterly
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(1_500_000.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_ic(1.20);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: 1.5M / 1.25M = 1.20 (passing)
    assert!(result.is_passing);
    assert!((result.current_ratio - 1.20).abs() < 0.01);
}

#[test]
fn test_ic_test_failing_scenario() {
    // Arrange: Interest collections < required
    let pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_ic(1.20);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: 1M / 1.25M = 0.80 < 1.20 (failing)
    assert!(!result.is_passing);
}

#[test]
fn test_ic_test_no_cure_amount() {
    // Arrange
    let pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_ic(1.20);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // SC-M08: the cure is a PRINCIPAL PAYDOWN, because that is how the
    // diversion applies it — paying down senior principal adds nothing to
    // interest collections, so a cash-shortfall cure cured nothing.
    //
    // De-levering: I_coll / (I_due - X*r*tau) >= R  =>  X >= (I_due - I_coll/R)/(r*tau).
    // I_due = 100M * 5% / 4 = 1.25M; I_coll = 1.0M; R = 1.20; r*tau = 0.05/4.
    //   X = (1.25M - 1.0M/1.20) / 0.0125 = (1.25M - 833,333) / 0.0125
    //     = 416,667 / 0.0125 = 33,333,333
    //
    // This test previously asserted 500,000 — the cash shortfall
    // `1.20*1.25M - 1.0M`. That is the right answer to a different question
    // ("how much extra interest cash would clear the test") and under-cured
    // the breach by ~67x.
    let cure = result
        .cure_amount
        .expect("IC breach should calculate a cure amount");
    let expected = (1_250_000.0 - 1_000_000.0 / 1.20) / (0.05 / 4.0);
    assert!(
        (cure.amount() - expected).abs() < 1.0,
        "IC cure must be the de-levering paydown {expected:.2}, got {:.2}. \
         500,000 would be the pre-SC-M08 cash shortfall.",
        cure.amount()
    );

    // Sanity: paying down exactly the cure must clear the test.
    let due_after = (100_000_000.0 - cure.amount()) * 0.05 / 4.0;
    assert!(
        1_000_000.0 / due_after >= 1.20 - 1e-9,
        "after the cure the IC ratio {:.4} must meet the 1.20 requirement",
        1_000_000.0 / due_after
    );
}

// Edge Cases Tests

#[test]
fn test_oc_test_empty_pool() {
    // Arrange: Empty pool
    let pool = AssetPool::new("EMPTY", DealType::Clo, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: Should fail with 0 ratio
    assert!(!result.is_passing);
    assert_eq!(result.current_ratio, 0.0);
}

#[test]
fn test_ic_test_no_interest_collections() {
    // Arrange
    let pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_ic(1.20);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: Should fail
    assert!(!result.is_passing);
}

#[test]
fn test_oc_test_infinity_ratio_zero_debt() {
    // Arrange: Edge case with zero tranche balance
    let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    // Fully paid-down senior: consistent original structure (90% of the
    // stack) whose CURRENT balance is zero, so the OC denominator is zero.
    let mut senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(99_999_999.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();
    senior.current_balance = Money::new(0.0, Currency::USD).expect("valid money fixture");

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: Should pass with infinite ratio
    assert!(result.is_passing);
    assert_eq!(result.current_ratio, f64::INFINITY);
}

// IC claims from the waterfall spec (F3)

/// Two-tranche stack used by the claim-cap IC tests: SENIOR (100M @ 5%) over
/// EQUITY, quarterly accrual, so the legacy IC due is 1.25M.
fn claim_test_tranches() -> TrancheStructure {
    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();
    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();
    TrancheStructure::new(vec![equity, senior]).unwrap()
}

#[test]
fn ic_measures_coverage_of_the_capped_claim() {
    let pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    let tranches = claim_test_tranches();

    // The waterfall caps SENIOR's claim at 2%: due = 100M * 0.02 * 0.25 = 0.5M.
    let mut caps: finstack_quant_core::HashMap<&str, Option<f64>> =
        finstack_quant_core::HashMap::default();
    caps.insert("SENIOR", Some(0.02));

    let mut context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
    );
    context.interest_claim_caps = &caps;

    let result = CoverageTest::new_ic(1.20)
        .calculate(&context)
        .expect("coverage calculation");

    // 1.0M collections / 0.5M capped due = 2.0 (passing). The legacy uncapped
    // due of 1.25M would read 0.8 and breach — the structure never owes it.
    assert!(result.is_passing);
    assert!(
        (result.current_ratio - 2.0).abs() < 0.01,
        "IC must cover the capped claim, got {}",
        result.current_ratio
    );
}

#[test]
fn ic_treats_a_tranche_without_interest_recipient_as_owing_nothing() {
    let pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
    let tranches = claim_test_tranches();

    // The waterfall defines no interest claim for SENIOR at all.
    let caps: finstack_quant_core::HashMap<&str, Option<f64>> =
        finstack_quant_core::HashMap::default();

    let mut context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
    );
    context.interest_claim_caps = &caps;

    let result = CoverageTest::new_ic(1.20)
        .calculate(&context)
        .expect("coverage calculation");

    assert!(result.is_passing);
    assert_eq!(
        result.current_ratio,
        f64::INFINITY,
        "no claim means nothing to cover"
    );
}
