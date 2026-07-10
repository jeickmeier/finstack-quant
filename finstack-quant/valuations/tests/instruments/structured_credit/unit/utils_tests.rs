//! Unit tests for utility functions.
//!
//! Tests cover:
//! - Date utilities (months_between)
//! - Rating factor tables and lookups
//! - Reinvestment manager logic

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{moodys_warf_factor, CreditRating, RatingFactorTable};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, AssetType, DealType, PoolAsset, ReinvestmentManager,
};
use time::Month;

// ============================================================================
// Rating Factor Tests
// ============================================================================

#[test]
fn test_moodys_warf_factor_aaa() {
    // Act
    let factor = moodys_warf_factor(CreditRating::AAA).unwrap();

    // Assert: AAA should be 1 (best rating)
    assert_eq!(factor, 1.0);
}

#[test]
fn test_moodys_warf_factor_a() {
    // Act
    let factor = moodys_warf_factor(CreditRating::A).unwrap();

    // Assert: A (flat notch / A2) should be 120
    assert_eq!(factor, 120.0);
}

#[test]
fn test_moodys_warf_factor_bb() {
    // Act
    let factor = moodys_warf_factor(CreditRating::BB).unwrap();

    // Assert: BB should be 1350
    assert_eq!(factor, 1350.0);
}

#[test]
fn test_moodys_warf_factor_b() {
    // Act
    let factor = moodys_warf_factor(CreditRating::B).unwrap();

    // Assert: B should be 2720
    assert_eq!(factor, 2720.0);
}

#[test]
fn test_moodys_warf_factor_ccc() {
    // Act
    let factor = moodys_warf_factor(CreditRating::CCC).unwrap();

    // Assert: CCC should be 6500
    assert_eq!(factor, 6500.0);
}

#[test]
fn test_moodys_warf_factor_nr() {
    // Act
    let factor = moodys_warf_factor(CreditRating::NR).unwrap();

    // Assert: Not rated should be 3650 (B-/CCC+ equivalent)
    assert_eq!(factor, 3650.0);
}

#[test]
fn test_rating_factor_table_creation() {
    // Arrange & Act
    let table = RatingFactorTable::moodys_standard().expect("registry table");

    // Assert
    assert_eq!(table.agency(), "Moody's");
    assert_eq!(table.methodology(), "IDEALIZED DEFAULT RATES");
    assert_eq!(table.get_factor(CreditRating::AAA).unwrap(), 1.0);
    assert_eq!(table.get_factor(CreditRating::BB).unwrap(), 1350.0);
    assert_eq!(table.get_factor(CreditRating::AAPlus).unwrap(), 10.0);
}

#[test]
fn test_rating_factor_monotonicity() {
    // Arrange: Better ratings should have lower factors
    let ratings = [
        (CreditRating::AAA, 1.0),
        (CreditRating::AA, 20.0),
        (CreditRating::A, 120.0),
        (CreditRating::BBB, 360.0),
        (CreditRating::BB, 1350.0),
        (CreditRating::B, 2720.0),
        (CreditRating::CCC, 6500.0),
    ];

    // Act & Assert: Factors should increase with worse ratings
    for i in 1..ratings.len() {
        let prev_factor = moodys_warf_factor(ratings[i - 1].0).unwrap();
        let curr_factor = moodys_warf_factor(ratings[i].0).unwrap();
        assert!(
            curr_factor > prev_factor,
            "Rating factors not monotonic: {:?} ({}), {:?} ({})",
            ratings[i - 1].0,
            prev_factor,
            ratings[i].0,
            curr_factor
        );
    }
}

#[test]
fn test_moodys_warf_factor_notches() {
    assert_eq!(moodys_warf_factor(CreditRating::BBPlus).unwrap(), 940.0);
    assert_eq!(moodys_warf_factor(CreditRating::BBMinus).unwrap(), 1766.0);
    assert_eq!(moodys_warf_factor(CreditRating::BBB).unwrap(), 360.0);
}

// ============================================================================
// Reinvestment Manager Tests
// ============================================================================

#[test]
fn test_reinvestment_manager_creation() {
    // Arrange & Act
    let end_date = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let manager = ReinvestmentManager::new(end_date);

    // Assert
    assert_eq!(manager.end_date, end_date);
    assert!(manager.reinvestment_allowed);
}

#[test]
fn test_reinvestment_manager_can_reinvest_before_end() {
    // Arrange
    let end_date = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let manager = ReinvestmentManager::new(end_date);

    // Act
    let can_reinvest =
        manager.can_reinvest(Date::from_calendar_date(2026, Month::January, 1).unwrap());

    // Assert
    assert!(can_reinvest);
}

#[test]
fn test_reinvestment_manager_cannot_reinvest_after_end() {
    // Arrange
    let end_date = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let manager = ReinvestmentManager::new(end_date);

    // Act
    let can_reinvest =
        manager.can_reinvest(Date::from_calendar_date(2028, Month::January, 1).unwrap());

    // Assert
    assert!(!can_reinvest);
}

#[test]
fn test_reinvestment_manager_selects_cheapest_first() {
    // Arrange
    let manager =
        ReinvestmentManager::new(Date::from_calendar_date(2027, Month::January, 1).unwrap());
    let pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();
    let base_asset = PoolAsset {
        day_count: finstack_quant_core::dates::DayCount::Act360,
        id: "BASE".to_string().into(),
        asset_type: AssetType::FirstLienLoan { industry: None },
        balance: Money::new(100.0, Currency::USD),
        rate: 0.07,
        spread_bps: Some(400.0),
        index_id: Some("SOFR-3M".to_string()),
        maturity,
        credit_quality: Some(CreditRating::BB),
        industry: None,
        obligor_id: None,
        is_defaulted: false,
        recovery_amount: None,
        purchase_price: None,
        acquisition_date: None,
        smm_override: None,
        mdr_override: None,
        contractual_payment: None,
    };

    // Create assets at different prices
    let asset_95 = PoolAsset {
        purchase_price: Some(Money::new(95.0, Currency::USD)), // Cheapest
        ..base_asset.clone()
    };
    let asset_98 = PoolAsset {
        purchase_price: Some(Money::new(98.0, Currency::USD)),
        ..base_asset.clone()
    };
    let asset_102 = PoolAsset {
        purchase_price: Some(Money::new(102.0, Currency::USD)), // Most expensive
        ..base_asset
    };

    let cash = Money::new(195.0, Currency::USD);

    // Act
    let selected = manager.select_assets(
        cash,
        vec![asset_102, asset_98, asset_95], // Unordered
        &pool,
        &MarketContext::default(),
        Date::from_calendar_date(2025, Month::January, 1).unwrap(),
    );

    // Assert: Should select 95 and 98 (total 193), skip 102
    assert_eq!(selected.len(), 2);
    let prices: Vec<f64> = selected
        .iter()
        .map(|a| a.purchase_price.unwrap().amount())
        .collect();
    assert!(prices.contains(&95.0));
    assert!(prices.contains(&98.0));
    assert!(!prices.contains(&102.0));
}

#[test]
fn test_reinvestment_manager_respects_budget() {
    // Arrange
    let manager =
        ReinvestmentManager::new(Date::from_calendar_date(2027, Month::January, 1).unwrap());
    let pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();
    let asset = PoolAsset::floating_rate_loan(
        "L1",
        Money::new(1_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity,
        finstack_quant_core::dates::DayCount::Act360,
    );

    let opportunities = vec![asset.clone(), asset.clone(), asset];
    let cash = Money::new(2_500_000.0, Currency::USD); // Only enough for 2.5 assets

    // Act
    let selected = manager.select_assets(
        cash,
        opportunities,
        &pool,
        &MarketContext::default(),
        Date::from_calendar_date(2025, Month::January, 1).unwrap(),
    );

    // Assert: Should only select 2 assets (not 3)
    assert_eq!(selected.len(), 2);
}
