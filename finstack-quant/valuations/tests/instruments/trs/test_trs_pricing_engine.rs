//! Unit tests for TRS shared pricing engine logic.
//!
//! Tests the common TrsEngine methods used by both equity and FI index TRS.

use super::test_utils::*;
use finstack_quant_core::currency::Currency::USD;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::TrsEngine;

// ================================================================================================
// Financing Leg PV Tests
// ================================================================================================

#[test]
fn test_financing_leg_pv_with_zero_spread() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().spread_bp(0.0).build();

    // Act
    let fin_pv =
        TrsEngine::pv_financing_leg(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    // Assert - With zero spread, financing leg PV = PV of forward rates only
    assert_eq!(fin_pv.currency(), USD);
    assert!(
        fin_pv.amount() > 0.0,
        "Financing leg should have positive PV"
    );
}

#[test]
fn test_financing_leg_pv_increases_with_spread() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_low = TestEquityTrsBuilder::new().spread_bp(10.0).build();

    let trs_high = TestEquityTrsBuilder::new().spread_bp(100.0).build();

    // Act
    let pv_low = TrsEngine::pv_financing_leg(
        &trs_low.financing,
        &trs_low.schedule,
        trs_low.notional,
        &market,
        as_of,
    )
    .unwrap();

    let pv_high = TrsEngine::pv_financing_leg(
        &trs_high.financing,
        &trs_high.schedule,
        trs_high.notional,
        &market,
        as_of,
    )
    .unwrap();

    // Assert
    assert!(
        pv_high.amount() > pv_low.amount(),
        "Higher spread should result in higher financing leg PV"
    );
}

#[test]
fn test_financing_leg_pv_scales_with_notional() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_1m = TestEquityTrsBuilder::new()
        .notional(Money::new(1_000_000.0, USD))
        .build();

    let trs_5m = TestEquityTrsBuilder::new()
        .notional(Money::new(5_000_000.0, USD))
        .build();

    // Act
    let pv_1m = TrsEngine::pv_financing_leg(
        &trs_1m.financing,
        &trs_1m.schedule,
        trs_1m.notional,
        &market,
        as_of,
    )
    .unwrap();

    let pv_5m = TrsEngine::pv_financing_leg(
        &trs_5m.financing,
        &trs_5m.schedule,
        trs_5m.notional,
        &market,
        as_of,
    )
    .unwrap();

    // Assert - Should scale linearly
    assert_approx_eq(
        pv_5m.amount() / pv_1m.amount(),
        5.0,
        0.01, // 1% tolerance
        "Financing leg PV should scale linearly with notional",
    );
}

#[test]
fn test_financing_leg_pv_increases_with_tenor() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_6m = TestEquityTrsBuilder::new().tenor_months(6).build();

    let trs_24m = TestEquityTrsBuilder::new().tenor_months(24).build();

    // Act
    let pv_6m = TrsEngine::pv_financing_leg(
        &trs_6m.financing,
        &trs_6m.schedule,
        trs_6m.notional,
        &market,
        as_of,
    )
    .unwrap();

    let pv_24m = TrsEngine::pv_financing_leg(
        &trs_24m.financing,
        &trs_24m.schedule,
        trs_24m.notional,
        &market,
        as_of,
    )
    .unwrap();

    // Assert
    assert!(
        pv_24m.amount() > pv_6m.amount(),
        "Longer tenor should result in higher financing leg PV"
    );
}

// ================================================================================================
// Financing Annuity Tests
// ================================================================================================

#[test]
fn test_financing_annuity_positive() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().build();

    // Act
    let annuity =
        TrsEngine::financing_annuity(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    // Assert
    assert!(annuity > 0.0, "Financing annuity should be positive");
}

#[test]
fn test_financing_annuity_scales_with_notional() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_1m = TestEquityTrsBuilder::new()
        .notional(Money::new(1_000_000.0, USD))
        .build();

    let trs_10m = TestEquityTrsBuilder::new()
        .notional(Money::new(10_000_000.0, USD))
        .build();

    // Act
    let annuity_1m = TrsEngine::financing_annuity(
        &trs_1m.financing,
        &trs_1m.schedule,
        trs_1m.notional,
        &market,
        as_of,
    )
    .unwrap();

    let annuity_10m = TrsEngine::financing_annuity(
        &trs_10m.financing,
        &trs_10m.schedule,
        trs_10m.notional,
        &market,
        as_of,
    )
    .unwrap();

    // Assert
    assert_approx_eq(
        annuity_10m / annuity_1m,
        10.0,
        0.01, // 1% tolerance
        "Annuity should scale linearly with notional",
    );
}

#[test]
fn test_financing_annuity_independent_of_spread() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_low = TestEquityTrsBuilder::new().spread_bp(10.0).build();

    let trs_high = TestEquityTrsBuilder::new().spread_bp(200.0).build();

    // Act
    let annuity_low = TrsEngine::financing_annuity(
        &trs_low.financing,
        &trs_low.schedule,
        trs_low.notional,
        &market,
        as_of,
    )
    .unwrap();

    let annuity_high = TrsEngine::financing_annuity(
        &trs_high.financing,
        &trs_high.schedule,
        trs_high.notional,
        &market,
        as_of,
    )
    .unwrap();

    // Assert - Annuity should not depend on spread (only on schedule and discounting)
    assert_approx_eq(
        annuity_low,
        annuity_high,
        TOLERANCE_CENTS,
        "Annuity should be independent of financing spread",
    );
}

#[test]
fn test_financing_annuity_increases_with_tenor() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();

    let trs_3m = TestEquityTrsBuilder::new().tenor_months(3).build();

    let trs_12m = TestEquityTrsBuilder::new().tenor_months(12).build();

    let trs_36m = TestEquityTrsBuilder::new().tenor_months(36).build();

    // Act
    let annuity_3m = TrsEngine::financing_annuity(
        &trs_3m.financing,
        &trs_3m.schedule,
        trs_3m.notional,
        &market,
        as_of,
    )
    .unwrap();

    let annuity_12m = TrsEngine::financing_annuity(
        &trs_12m.financing,
        &trs_12m.schedule,
        trs_12m.notional,
        &market,
        as_of,
    )
    .unwrap();

    let annuity_36m = TrsEngine::financing_annuity(
        &trs_36m.financing,
        &trs_36m.schedule,
        trs_36m.notional,
        &market,
        as_of,
    )
    .unwrap();

    // Assert - Should be monotonically increasing with tenor
    assert!(annuity_12m > annuity_3m);
    assert!(annuity_36m > annuity_12m);
}

#[test]
fn test_financing_annuity_bounded() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let notional = 10_000_000.0;
    let tenor_years = 1.0;

    let trs = TestEquityTrsBuilder::new()
        .notional(Money::new(notional, USD))
        .tenor_months(12)
        .build();

    // Act
    let annuity =
        TrsEngine::financing_annuity(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    // Assert - Annuity should be less than notional * tenor (with margin for discounting)
    assert!(
        annuity > 0.0 && annuity <= notional * tenor_years * 1.05,
        "Annuity {} should be in range [0, {}]",
        annuity,
        notional * tenor_years
    );
}

// ================================================================================================
// Relationship Tests: Annuity and Financing Leg PV
// ================================================================================================

#[test]
fn test_financing_leg_pv_equals_annuity_times_rate_plus_spread() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let spread_bp = 50.0;

    let trs = TestEquityTrsBuilder::new().spread_bp(spread_bp).build();

    // Act
    let fin_pv =
        TrsEngine::pv_financing_leg(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    let annuity =
        TrsEngine::financing_annuity(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    // Also compute PV with zero spread to get forward rate component
    let trs_zero = TestEquityTrsBuilder::new().spread_bp(0.0).build();

    let fin_pv_zero = TrsEngine::pv_financing_leg(
        &trs_zero.financing,
        &trs_zero.schedule,
        trs_zero.notional,
        &market,
        as_of,
    )
    .unwrap();

    // Assert - Spread contribution should approximately equal annuity * spread
    let spread_contribution = fin_pv.amount() - fin_pv_zero.amount();
    let expected_spread_contribution = annuity * (spread_bp / 10000.0);

    assert_approx_eq(
        spread_contribution,
        expected_spread_contribution,
        1.0, // $1 tolerance
        "Spread contribution should equal annuity * spread",
    );
}

// ================================================================================================
// Edge Case Tests for Pricing Engine
// ================================================================================================

#[test]
fn test_financing_leg_with_very_small_notional() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new()
        .notional(Money::new(100.0, USD))
        .build();

    // Act
    let fin_pv =
        TrsEngine::pv_financing_leg(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    // Assert
    assert!(fin_pv.amount() > 0.0);
    assert!(
        fin_pv.amount() < 10.0,
        "PV should be small for tiny notional"
    );
}

#[test]
fn test_financing_leg_with_large_notional() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new()
        .notional(Money::new(1_000_000_000.0, USD)) // $1B
        .build();

    // Act
    let fin_pv =
        TrsEngine::pv_financing_leg(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    // Assert
    assert!(fin_pv.amount().is_finite());
    assert!(
        fin_pv.amount() > 1_000_000.0,
        "PV should be significant for large notional"
    );
}

#[test]
fn test_financing_annuity_with_short_tenor() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new().tenor_months(1).build();

    // Act
    let annuity =
        TrsEngine::financing_annuity(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    // Assert
    assert!(annuity > 0.0);
    assert!(
        annuity < trs.notional.amount() * 0.1,
        "Short tenor annuity should be small"
    );
}

#[test]
fn test_financing_annuity_with_long_tenor() {
    // Arrange
    let market = create_market_context();
    let as_of = as_of_date();
    let trs = TestEquityTrsBuilder::new()
        .tenor_months(60) // 5 years
        .build();

    // Act
    let annuity =
        TrsEngine::financing_annuity(&trs.financing, &trs.schedule, trs.notional, &market, as_of)
            .unwrap();

    // Assert
    assert!(annuity > 0.0);
    assert!(
        annuity <= trs.notional.amount() * 5.0 * 1.1,
        "Long tenor annuity should be bounded"
    );
}
