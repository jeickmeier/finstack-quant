//! Edge case and error handling tests for Inflation-Linked Bonds
//!
//! Tests cover:
//! - Matured bonds
//! - Invalid parameters
//! - Missing market data
//! - Extreme values
//! - Boundary conditions
//! - Error propagation

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::instruments::Instrument;
use rust_decimal::Decimal;

#[test]
fn test_valuation_after_maturity() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.maturity = d(2025, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2026, 1, 1); // After maturity

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - matured bond has zero value
    assert_eq!(pv.amount(), 0.0);
}

#[test]
fn test_valuation_at_maturity() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.maturity = d(2025, 1, 2);
    ilb.issue_date = d(2024, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = ilb.maturity;

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - holder-view position value: the principal payment settles on
    // `as_of` (= maturity) and is excluded, so the matured position is worth 0.
    assert_eq!(pv.amount(), 0.0);
}

#[test]
fn test_valuation_before_issue() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2024, 1, 1); // Before issue

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - implementation may return zero or small value
    // (no cashflows before issue)
    assert!(pv.amount() >= 0.0);
}

#[test]
fn test_zero_coupon_ilb() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.real_coupon = Decimal::try_from(0.0).expect("valid decimal"); // Zero coupon
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - should still have value from principal
    assert!(pv.amount() > 0.0);
}

#[test]
fn test_very_high_coupon() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.real_coupon = Decimal::try_from(0.50).expect("valid decimal"); // 50% coupon (extreme)
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2027, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - should still calculate without error
    assert!(pv.amount() > 0.0);
    assert!(pv.amount() > ilb.notional.amount()); // Premium bond
}

#[test]
fn test_very_small_notional() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.notional =
        finstack_quant_core::money::Money::new(1.0, Currency::USD).expect("valid money fixture"); // $1 notional

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 0.0);
    assert!(pv.amount() < 10.0); // Small value
}

#[test]
fn test_very_large_notional() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.notional = finstack_quant_core::money::Money::new(1_000_000_000_000.0, Currency::USD)
        .expect("valid money fixture"); // $1T

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 100_000_000_000.0); // Should be very large
}

#[test]
fn test_missing_discount_curve() {
    // Arrange
    let ilb = sample_tips();
    let as_of = d(2025, 1, 2);

    // Context without discount curve
    let ctx = finstack_quant_core::market_data::context::MarketContext::new();

    // Act & Assert
    let result = ilb.value(&ctx, as_of);
    assert!(result.is_err());
}

#[test]
fn test_missing_inflation_data() {
    // Arrange
    let ilb = sample_tips();
    let as_of = d(2025, 1, 2);

    // Context with discount but no inflation
    let disc = finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.95)])
        .build()
        .unwrap();

    let ctx = finstack_quant_core::market_data::context::MarketContext::new().insert(disc);

    // Act & Assert
    let result = ilb.value(&ctx, as_of);
    assert!(result.is_err());
}

#[test]
fn test_wrong_discount_curve_id() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.discount_curve_id = finstack_quant_core::types::CurveId::new("NONEXISTENT");

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act & Assert
    let result = ilb.value(&ctx, as_of);
    assert!(result.is_err());
}

#[test]
fn test_wrong_inflation_id() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.inflation_index_id = finstack_quant_core::types::CurveId::new("NONEXISTENT");

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act & Assert
    let result = ilb.value(&ctx, as_of);
    assert!(result.is_err());
}

#[test]
fn test_extreme_deflation() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.base_index = 300.0;
    ilb.deflation_protection =
        finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::DeflationProtection::None;
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2026, 1, 2);

    let (mut ctx, _) = market_context_with_index();

    // Extreme deflation scenario
    let observations = (1..=12).map(|month| (d(2025, month, 1), 100.0)).collect(); // 67% deflation
    let index = finstack_quant_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        Currency::USD,
    )
    .unwrap()
    .with_interpolation(finstack_quant_core::market_data::scalars::InflationInterpolation::Linear);
    ctx = ctx.insert_inflation_index("US-CPI-U", index);

    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - with no deflation protection, value should be significantly reduced
    assert!(pv.amount() > 0.0);
    assert!(pv.amount() < ilb.notional.amount() * 0.5);
}

#[test]
fn test_extreme_inflation() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.base_index = 100.0;
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2026, 1, 2);

    let (mut ctx, _) = market_context_with_index();

    // Extreme inflation scenario
    let observations = (1..=12).map(|month| (d(2025, month, 1), 1000.0)).collect(); // 900% inflation
    let index = finstack_quant_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        Currency::USD,
    )
    .unwrap()
    .with_interpolation(finstack_quant_core::market_data::scalars::InflationInterpolation::Linear);
    ctx = ctx.insert_inflation_index("US-CPI-U", index);

    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - value should be much higher due to inflation adjustment
    assert!(pv.amount() > ilb.notional.amount() * 2.0);
}

#[test]
fn test_very_short_maturity() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2025, 1, 10); // 8 days

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 0.0);
}

#[test]
fn test_very_long_maturity() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2075, 1, 2); // 50 years

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 0.0);
}

#[test]
fn test_attributes_mutable() {
    // Arrange
    let mut ilb = sample_tips();

    // Act
    ilb.attributes_mut()
        .meta
        .insert("test_key".to_string(), "test_value".to_string());

    // Assert
    assert_eq!(
        ilb.attributes().meta.get("test_key"),
        Some(&"test_value".to_string())
    );
}

#[test]
fn test_negative_inflation_lag_days() {
    // Arrange
    let mut ilb = sample_tips();
    // Technically invalid but testing robustness
    ilb.lag = finstack_quant_core::market_data::scalars::InflationLag::Days(0);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act - should handle gracefully
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 0.0);
}

#[test]
fn test_real_yield_with_empty_schedule() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2025, 1, 2); // Degenerate

    let as_of = d(2025, 1, 2);

    // Act & Assert - should error gracefully
    let result = ilb.real_yield(100.0, as_of);
    assert!(result.is_err());
}

#[test]
fn test_calendar_id_none() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.calendar_id = None;

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 0.0);
}

#[test]
fn test_business_day_convention_variants() {
    // Arrange
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    for business_day_convention in [
        finstack_quant_core::dates::BusinessDayConvention::Following,
        finstack_quant_core::dates::BusinessDayConvention::Preceding,
        finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
    ] {
        let mut ilb = sample_tips();
        ilb.business_day_convention = business_day_convention;

        // Act
        let pv = ilb.value(&ctx, as_of).unwrap();

        // Assert
        assert!(pv.amount() > 0.0);
    }
}

#[test]
fn test_stub_convention_variants() {
    // Arrange
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    for stub in [
        finstack_quant_core::dates::StubKind::ShortFront,
        finstack_quant_core::dates::StubKind::ShortBack,
    ] {
        let mut ilb = sample_tips();
        ilb.stub = stub;
        ilb.issue_date = d(2025, 1, 5); // Slightly off standard date
        ilb.maturity = d(2027, 7, 10);

        // Act
        let pv = ilb.value(&ctx, as_of).unwrap();

        // Assert
        assert!(pv.amount() > 0.0);
    }
}

#[test]
fn test_discount_curve_dependency() {
    // Arrange
    let ilb = sample_tips();

    // Act
    let curve_id = ilb
        .market_dependencies()
        .expect("market_dependencies")
        .curves
        .discount_curves
        .first()
        .cloned()
        .expect("ILB should declare a discount curve");

    // Assert
    assert_eq!(curve_id.as_str(), "USD-OIS");
}

#[test]
fn test_cashflow_provider_trait() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let flows =
        finstack_quant_cashflows::CashflowProvider::dated_cashflows(&ilb, &ctx, as_of).unwrap();

    // Assert
    assert!(!flows.is_empty());
}
