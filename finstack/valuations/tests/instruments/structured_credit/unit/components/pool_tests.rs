//! Unit tests for AssetPool and PoolAsset components.
//!
//! Tests cover:
//! - Asset creation and builder patterns
//! - AssetPool statistics (WAC, WAS, WAM, diversity)
//! - Balance calculations
//! - Default marking and recovery
//! - Edge cases and boundary conditions

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::money::Money;
use finstack_core::types::CreditRating;
use finstack_valuations::instruments::fixed_income::structured_credit::{
    calculate_pool_stats, AssetPool, AssetType, DealType, PoolAsset,
};
use time::Month;

// ============================================================================
// Test Helpers
// ============================================================================

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

// ============================================================================
// PoolAsset Creation Tests
// ============================================================================

#[test]
fn test_pool_asset_floating_rate_loan_creation() {
    // Arrange & Act
    let asset = PoolAsset::floating_rate_loan(
        "LOAN001",
        Money::new(10_000_000.0, Currency::USD),
        "SOFR-3M",
        450.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    );

    // Assert
    assert_eq!(asset.id.as_str(), "LOAN001");
    assert_eq!(asset.balance.amount(), 10_000_000.0);
    assert_eq!(asset.spread_bps(), 450.0);
    assert!(asset.index_id.is_some());
    assert_eq!(asset.index_id.as_ref().unwrap(), "SOFR-3M");
    assert!(!asset.is_defaulted);
}

#[test]
fn test_pool_asset_fixed_rate_bond_creation() {
    // Arrange & Act
    let asset = PoolAsset::fixed_rate_bond(
        "BOND001",
        Money::new(5_000_000.0, Currency::USD),
        0.07,
        maturity_date(),
        finstack_core::dates::DayCount::Thirty360,
    );

    // Assert
    assert_eq!(asset.id.as_str(), "BOND001");
    assert_eq!(asset.balance.amount(), 5_000_000.0);
    assert_eq!(asset.rate, 0.07);
    assert!(asset.spread_bps.is_none()); // Fixed rate has no separate spread
    assert!(asset.index_id.is_none());
}

#[test]
fn test_pool_asset_builder_methods() {
    // Arrange & Act
    let asset = PoolAsset::floating_rate_loan(
        "LOAN002",
        Money::new(15_000_000.0, Currency::USD),
        "SOFR-3M",
        500.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    )
    .with_rating(CreditRating::BB)
    .with_industry("Technology")
    .with_obligor("OBLIGOR001");

    // Assert
    assert_eq!(asset.credit_quality, Some(CreditRating::BB));
    assert_eq!(asset.industry.as_deref(), Some("Technology"));
    assert_eq!(asset.obligor_id.as_deref(), Some("OBLIGOR001"));
}

#[test]
fn test_pool_asset_spread_bps_fallback_to_rate() {
    // Arrange: Fixed rate bond without explicit spread
    let asset = PoolAsset::fixed_rate_bond(
        "BOND002",
        Money::new(10_000_000.0, Currency::USD),
        0.06,
        maturity_date(),
        finstack_core::dates::DayCount::Thirty360,
    );

    // Act
    let spread = asset.spread_bps();

    // Assert: Should return rate * 10000 when spread_bps is None
    assert_eq!(spread, 600.0); // 6% = 600bps
}

#[test]
fn test_pool_asset_default_with_recovery() {
    // Arrange
    let mut asset = PoolAsset::fixed_rate_bond(
        "BOND003",
        Money::new(1_000_000.0, Currency::USD),
        0.08,
        maturity_date(),
        finstack_core::dates::DayCount::Thirty360,
    );

    // Act
    asset.default_with_recovery(Money::new(400_000.0, Currency::USD), test_date());

    // Assert
    assert!(asset.is_defaulted);
    assert_eq!(asset.recovery_amount.unwrap().amount(), 400_000.0);
}

#[test]
fn test_pool_asset_remaining_term_calculation() {
    // Arrange
    let asset = PoolAsset::fixed_rate_bond(
        "BOND004",
        Money::new(5_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_core::dates::DayCount::Thirty360,
    );

    // Act
    let remaining = asset
        .remaining_term(
            Date::from_calendar_date(2025, Month::January, 1).unwrap(),
            DayCount::Act365F,
        )
        .unwrap();

    // Assert: 5 years remaining
    assert!((remaining - 5.0).abs() < 0.01);
}

// ============================================================================
// AssetPool Creation and Basic Operations
// ============================================================================

#[test]
fn test_asset_pool_creation() {
    // Arrange & Act
    let pool = AssetPool::new("TEST_POOL", DealType::CLO, Currency::USD);

    // Assert
    assert_eq!(pool.id.as_str(), "TEST_POOL");
    assert_eq!(pool.deal_type, DealType::CLO);
    assert_eq!(pool.base_currency(), Currency::USD);
    assert_eq!(pool.assets.len(), 0);
    assert_eq!(pool.total_balance().unwrap().amount(), 0.0);
}

#[test]
fn test_asset_pool_total_balance_calculation() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(10_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L2",
        Money::new(15_000_000.0, Currency::USD),
        "SOFR-3M",
        450.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    // Act
    let total = pool.total_balance().unwrap();

    // Assert
    assert_eq!(total.amount(), 25_000_000.0);
}

#[test]
fn test_asset_pool_empty_pool_balance() {
    // Arrange
    let pool = AssetPool::new("EMPTY", DealType::ABS, Currency::EUR);

    // Act
    let total = pool.total_balance().unwrap();

    // Assert
    assert_eq!(total.amount(), 0.0);
    assert_eq!(total.currency(), Currency::EUR);
    assert_eq!(pool.base_currency(), Currency::EUR);
}

#[test]
fn test_asset_pool_rejects_asset_currency_mismatch() {
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(10_000_000.0, Currency::EUR),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    let err = pool
        .total_balance()
        .expect_err("mixed asset currencies should be rejected");

    assert!(matches!(err, finstack_core::Error::CurrencyMismatch { .. }));
}

#[test]
fn test_asset_pool_performing_balance_excludes_defaults() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    // Add performing asset
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(10_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    // Add defaulted asset
    let mut defaulted = PoolAsset::floating_rate_loan(
        "L2",
        Money::new(5_000_000.0, Currency::USD),
        "SOFR-3M",
        450.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    );
    defaulted.default_with_recovery(Money::new(2_000_000.0, Currency::USD), test_date());
    pool.assets.push(defaulted);

    // Act
    let performing = pool.performing_balance().unwrap();
    let total = pool.total_balance().unwrap();

    // Assert
    assert_eq!(performing.amount(), 10_000_000.0); // Only performing asset
    assert_eq!(total.amount(), 15_000_000.0); // Both assets
}

// ============================================================================
// AssetPool Statistics Tests (WAC, WAS, WAM)
// ============================================================================

#[test]
fn test_pool_weighted_avg_coupon_single_asset() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "B1",
        Money::new(10_000_000.0, Currency::USD),
        0.06,
        maturity_date(),
        finstack_core::dates::DayCount::Thirty360,
    ));

    // Act
    let wac = pool.weighted_avg_coupon();

    // Assert
    assert_eq!(wac, 0.06);
}

#[test]
fn test_pool_weighted_avg_coupon_multiple_assets() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "B1",
        Money::new(10_000_000.0, Currency::USD),
        0.06, // 6%
        maturity_date(),
        finstack_core::dates::DayCount::Thirty360,
    ));
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "B2",
        Money::new(20_000_000.0, Currency::USD),
        0.09, // 9%
        maturity_date(),
        finstack_core::dates::DayCount::Thirty360,
    ));

    // Act
    let wac = pool.weighted_avg_coupon();

    // Assert: (10M * 6% + 20M * 9%) / 30M = (0.6M + 1.8M) / 30M = 8%
    assert!((wac - 0.08).abs() < 0.0001);
}

#[test]
fn test_pool_weighted_avg_coupon_empty_pool() {
    // Arrange
    let pool = AssetPool::new("EMPTY", DealType::CLO, Currency::USD);

    // Act
    let wac = pool.weighted_avg_coupon();

    // Assert
    assert_eq!(wac, 0.0);
}

#[test]
fn test_pool_weighted_avg_spread_floating_rate_assets() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(10_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0, // 400bps
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L2",
        Money::new(20_000_000.0, Currency::USD),
        "SOFR-3M",
        500.0, // 500bps
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    // Act
    let was = pool.weighted_avg_spread();

    // Assert: (10M * 400 + 20M * 500) / 30M = (4,000M + 10,000M) / 30M = 466.67bps
    assert!((was - 466.666667).abs() < 0.01);
}

#[test]
fn test_pool_weighted_avg_spread_mixed_assets() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    // Floating rate with explicit spread
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(10_000_000.0, Currency::USD),
        "SOFR-3M",
        450.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    // Fixed rate: carries no explicit spread, so it is EXCLUDED from WAS
    // (the all-in coupon is not a spread; no rate × 10⁴ fallback).
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "B1",
        Money::new(10_000_000.0, Currency::USD),
        0.07, // 700bps all-in coupon, no spread component
        maturity_date(),
        finstack_core::dates::DayCount::Thirty360,
    ));

    // Act
    let was = pool.weighted_avg_spread();

    // Assert: only the floating-rate loan contributes → 450bps
    assert!((was - 450.0).abs() < 0.01);
}

#[test]
fn test_pool_weighted_avg_spread_excludes_defaulted() {
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(10_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));
    let mut defaulted = PoolAsset::floating_rate_loan(
        "L2",
        Money::new(10_000_000.0, Currency::USD),
        "SOFR-3M",
        900.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    );
    defaulted.is_defaulted = true;
    pool.assets.push(defaulted);

    // Defaulted assets are excluded from numerator AND denominator.
    let was = pool.weighted_avg_spread();
    assert!((was - 400.0).abs() < 0.01);
}

#[test]
fn test_pool_weighted_avg_maturity() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "B1",
        Money::new(10_000_000.0, Currency::USD),
        0.06,
        Date::from_calendar_date(2028, Month::January, 1).unwrap(), // 3 years
        finstack_core::dates::DayCount::Thirty360,
    ));
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "B2",
        Money::new(10_000_000.0, Currency::USD),
        0.06,
        Date::from_calendar_date(2032, Month::January, 1).unwrap(), // 7 years
        finstack_core::dates::DayCount::Thirty360,
    ));

    // Act
    let wam = pool.weighted_avg_maturity(test_date());

    // Assert: (10M * 3 + 10M * 7) / 20M = 5 years
    assert!((wam - 5.0).abs() < 0.1);
}

// ============================================================================
// AssetPool Diversity Score Tests
// ============================================================================

#[test]
fn test_pool_diversity_score_single_obligor() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L1",
            Money::new(10_000_000.0, Currency::USD),
            "SOFR-3M",
            400.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_obligor("OB1"),
    );
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L2",
            Money::new(10_000_000.0, Currency::USD),
            "SOFR-3M",
            450.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_obligor("OB1"),
    );

    // Act
    let diversity = pool.diversity_score();

    // Assert: Same obligor = score of 1
    assert!((diversity - 1.0).abs() < 0.01);
}

#[test]
fn test_pool_diversity_score_multiple_obligors() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L1",
            Money::new(10_000_000.0, Currency::USD),
            "SOFR-3M",
            400.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_obligor("OB1"),
    );
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L2",
            Money::new(10_000_000.0, Currency::USD),
            "SOFR-3M",
            450.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_obligor("OB2"),
    );

    // Act
    let diversity = pool.diversity_score();

    // Assert: Two equal obligors = score of 2
    assert!((diversity - 2.0).abs() < 0.01);
}

#[test]
fn test_pool_diversity_score_empty_pool() {
    // Arrange
    let pool = AssetPool::new("EMPTY", DealType::CLO, Currency::USD);

    // Act
    let diversity = pool.diversity_score();

    // Assert
    assert_eq!(diversity, 0.0);
}

// ============================================================================
// AssetPool Filtering and Grouping Tests
// ============================================================================

#[test]
fn test_pool_assets_by_industry() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L1",
            Money::new(10_000_000.0, Currency::USD),
            "SOFR-3M",
            400.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_industry("Technology"),
    );
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L2",
            Money::new(15_000_000.0, Currency::USD),
            "SOFR-3M",
            450.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_industry("Healthcare"),
    );
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L3",
            Money::new(20_000_000.0, Currency::USD),
            "SOFR-3M",
            500.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_industry("Technology"),
    );

    // Act
    let tech_assets = pool.assets_by_industry("Technology");

    // Assert
    assert_eq!(tech_assets.len(), 2);
    assert_eq!(
        tech_assets[0].balance.amount() + tech_assets[1].balance.amount(),
        30_000_000.0
    );
}

#[test]
fn test_pool_assets_by_obligor() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L1",
            Money::new(10_000_000.0, Currency::USD),
            "SOFR-3M",
            400.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_obligor("OB1"),
    );
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L2",
            Money::new(15_000_000.0, Currency::USD),
            "SOFR-3M",
            450.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_obligor("OB1"),
    );

    // Act
    let ob1_assets = pool.assets_by_obligor("OB1");

    // Assert
    assert_eq!(ob1_assets.len(), 2);
}

// ============================================================================
// calculate_pool_stats Function Tests
// ============================================================================

#[test]
fn test_calculate_pool_stats_comprehensive() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    // Add diverse assets
    pool.assets.push(
        PoolAsset::floating_rate_loan(
            "L1",
            Money::new(10_000_000.0, Currency::USD),
            "SOFR-3M",
            400.0,
            maturity_date(),
            finstack_core::dates::DayCount::Act360,
        )
        .with_rating(CreditRating::BB)
        .with_industry("Technology")
        .with_obligor("OB1"),
    );

    pool.assets.push(
        PoolAsset::fixed_rate_bond(
            "B1",
            Money::new(5_000_000.0, Currency::USD),
            0.07,
            maturity_date(),
            finstack_core::dates::DayCount::Thirty360,
        )
        .with_rating(CreditRating::B)
        .with_industry("Healthcare")
        .with_obligor("OB2"),
    );

    // Act
    let stats = calculate_pool_stats(&pool, test_date());

    // Assert
    assert!(stats.weighted_avg_coupon > 0.0);
    assert!(stats.weighted_avg_spread > 0.0);
    assert!(stats.weighted_avg_maturity > 0.0);
    assert_eq!(stats.num_obligors, 2);
    assert_eq!(stats.num_industries, 2);
    assert_eq!(stats.cumulative_default_rate, 0.0); // No defaults
}

#[test]
fn test_calculate_pool_stats_with_defaults() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    // Add performing asset
    pool.assets.push(PoolAsset::floating_rate_loan(
        "L1",
        Money::new(9_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    ));

    // Add defaulted asset
    let mut defaulted = PoolAsset::floating_rate_loan(
        "L2",
        Money::new(1_000_000.0, Currency::USD),
        "SOFR-3M",
        450.0,
        maturity_date(),
        finstack_core::dates::DayCount::Act360,
    );
    defaulted.default_with_recovery(Money::new(400_000.0, Currency::USD), test_date());
    pool.assets.push(defaulted);

    // Act
    let stats = calculate_pool_stats(&pool, test_date());

    // Assert: 1M / 10M = 10% default rate
    assert!((stats.cumulative_default_rate - 10.0).abs() < 0.01);
}

// ============================================================================
// Edge Cases and Boundary Conditions
// ============================================================================

#[test]
fn test_pool_zero_balance_asset() {
    // Arrange
    let mut pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "B1",
        Money::new(0.0, Currency::USD), // Zero balance
        0.06,
        maturity_date(),
        finstack_core::dates::DayCount::Thirty360,
    ));

    // Act
    let wac = pool.weighted_avg_coupon();
    let was = pool.weighted_avg_spread();

    // Assert: Should handle gracefully
    assert_eq!(wac, 0.0);
    assert_eq!(was, 0.0);
}

#[test]
fn test_pool_negative_days_remaining_term() {
    // Arrange
    let asset = PoolAsset::fixed_rate_bond(
        "B1",
        Money::new(5_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2020, Month::January, 1).unwrap(), // Past maturity
        finstack_core::dates::DayCount::Thirty360,
    );

    // Act
    let remaining = asset.remaining_term(test_date(), DayCount::Act365F);

    // Assert: Should handle past maturity
    assert!(remaining.is_ok());
}

#[test]
fn test_pool_asset_type_classification() {
    // Arrange & Act
    let first_lien = PoolAsset {
        day_count: finstack_core::dates::DayCount::Act360,
        id: "L1".to_string().into(),
        asset_type: AssetType::FirstLienLoan {
            industry: Some("Tech".to_string()),
        },
        balance: Money::new(10_000_000.0, Currency::USD),
        rate: 0.07,
        spread_bps: Some(450.0),
        index_id: Some("SOFR-3M".to_string()),
        maturity: maturity_date(),
        credit_quality: Some(CreditRating::BB),
        industry: Some("Technology".to_string()),
        obligor_id: Some("OB1".to_string()),
        is_defaulted: false,
        recovery_amount: None,
        purchase_price: None,
        acquisition_date: None,
        smm_override: None,
        mdr_override: None,
    };

    // Assert
    match first_lien.asset_type {
        AssetType::FirstLienLoan { industry } => {
            assert_eq!(industry.as_deref(), Some("Tech"));
        }
        _ => panic!("Expected FirstLienLoan"),
    }
}
