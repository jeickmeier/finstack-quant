//! Unit tests for OC/IC coverage test calculations.
//!
//! Tests cover:
//! - OC test calculation logic
//! - IC test calculation logic
//! - Passing/failing scenarios
//! - Cure amount calculations
//! - Edge cases and boundary conditions

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CoverageTest, DealType, PoolAsset, TestContext, Tranche, TrancheCoupon,
    TrancheSeniority, TrancheStructure,
};
use time::Month;

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
        period_start: None,
        cash_balance,
        interest_collections,
        haircuts: None,
        par_value_threshold: None,
        market: None,
        tranche_balances: None,
        current_pool_balance: None,
    }
}

// ============================================================================
// OC Test Calculation Tests
// ============================================================================

#[test]
fn test_oc_test_passing_scenario() {
    // Arrange: AssetPool value > required multiple of tranche
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(125_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(0.0, Currency::USD),
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
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(100_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    let tranche = Tranche::new(
        "CLASS_A_1",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();
    let tranches = TrancheStructure::new(vec![tranche]).unwrap();
    let context = context_for_tranche(
        &pool,
        &tranches,
        "CLASS_A_1",
        Money::new(0.0, Currency::USD),
        Money::new(0.0, Currency::USD),
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
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(120_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(0.0, Currency::USD),
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
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(120_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(5_000_000.0, Currency::USD),
        Money::new(0.0, Currency::USD),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: (120M + 5M) / 100M = 1.25 (passing)
    assert!(result.is_passing);
}

#[test]
fn test_oc_test_cure_amount_calculation() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(115_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(0.0, Currency::USD),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Cure amount = diverted cash needed to restore OC ratio. Diverted cash
    // leaves the OC numerator at the same time it pays down notes:
    // (115M - X) / (100M - X) = 1.25 => X = 40M.
    assert!(!result.is_passing);
    assert!(result.cure_amount.is_some());
    assert!((result.cure_amount.unwrap().amount() - 40_000_000.0).abs() < 1.0);
}

// ============================================================================
// IC Test Calculation Tests
// ============================================================================

#[test]
fn test_ic_test_passing_scenario() {
    // Arrange: Interest collections > required multiple of interest due
    let pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 }, // 5% = 1.25M quarterly
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(1_500_000.0, Currency::USD),
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
    let pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(1_000_000.0, Currency::USD),
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
    let pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(1_000_000.0, Currency::USD),
    );

    let test = CoverageTest::new_ic(1.20);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: IC breaches report the senior interest shortfall as the cure.
    // interest due = 100M * 5% / 4 = 1.25M; cure = 1.20 * 1.25M - 1.0M = 0.5M.
    let cure = result
        .cure_amount
        .expect("IC breach should calculate a cure amount");
    assert!((cure.amount() - 500_000.0).abs() < 1.0);
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[test]
fn test_oc_test_empty_pool() {
    // Arrange: Empty pool
    let pool = AssetPool::new("EMPTY", DealType::CLO, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(0.0, Currency::USD),
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
    let pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(0.0, Currency::USD),
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
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(100_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(11_111_111.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    let senior = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(0.0, Currency::USD), // Zero balance
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![equity, senior]).unwrap();

    let context = context_for_tranche(
        &pool,
        &tranches,
        "SENIOR",
        Money::new(0.0, Currency::USD),
        Money::new(0.0, Currency::USD),
    );

    let test = CoverageTest::new_oc(1.25);

    // Act
    let result = test.calculate(&context).expect("coverage calculation");

    // Assert: Should pass with infinite ratio
    assert!(result.is_passing);
    assert_eq!(result.current_ratio, f64::INFINITY);
}
