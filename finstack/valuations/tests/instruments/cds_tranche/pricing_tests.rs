//! Core pricing functionality tests for CDS Tranche.
//!
//! Tests cover:
//! - Basic tranche pricing (PV calculation)
//! - Different tranche types (equity, mezzanine, senior)
//! - Buy vs sell protection
//! - Notional scaling
//! - Maturity edge cases
//!
//! Note: Internal implementation details like payment schedules, premium/protection
//! leg decomposition, and accrual-on-default mechanics are tested indirectly
//! through these end-to-end pricing tests and through public metric APIs.

use super::helpers::*;
use finstack_cashflows::builder::ScheduleParams;
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_valuations::instruments::credit_derivatives::cds_tranche::CDSTranche;
use finstack_valuations::instruments::credit_derivatives::cds_tranche::CDSTrancheParams;
use finstack_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricer;
use finstack_valuations::instruments::credit_derivatives::cds_tranche::TrancheSide;
use time::macros::date;

// ==================== Basic Pricing Tests ====================

#[test]
fn test_tranche_pricing_returns_valid_pv() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok(), "Pricing should succeed");
    let pv = result.unwrap();
    assert_eq!(pv.currency(), Currency::USD);
    assert!(pv.amount().is_finite(), "PV should be finite");
}

#[test]
fn test_equity_tranche_pricing() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = equity_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok());
    let pv = result.unwrap();
    assert!(pv.amount().is_finite());
}

#[test]
fn test_senior_tranche_pricing() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = senior_tranche();
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert
    assert!(result.is_ok());
    let pv = result.unwrap();
    assert!(pv.amount().is_finite());
}

#[test]
fn test_bespoke_seasoned_tranche_requires_effective_date_for_accrued_premium() {
    let market = standard_market_context();
    let as_of = date!(2025 - 02 - 01);

    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        3.0,
        7.0,
        Money::new(10_000_000.0, Currency::USD),
        date!(2030 - 01 - 01),
        500.0,
    );
    let schedule_params = ScheduleParams::quarterly_act360();

    let mut explicit = CDSTranche::new(
        "BESPOKE-SEASONED-EXPLICIT",
        &tranche_params,
        &schedule_params,
        "USD-OIS",
        "CDX.NA.IG.42",
        TrancheSide::SellProtection,
    )
    .expect("bespoke tranche");
    explicit.effective_date = Some(date!(2024 - 11 - 15));

    let accrued_explicit = explicit
        .accrued_premium(&market, as_of)
        .expect("explicit accrued premium");
    assert!(accrued_explicit > 0.0);

    let mut missing_effective = explicit;
    missing_effective.effective_date = None;

    let err = missing_effective
        .accrued_premium(&market, as_of)
        .expect_err("bespoke seasoned tranche without effective_date should be rejected");
    assert!(matches!(
        err,
        finstack_core::Error::Validation(_) | finstack_core::Error::Input(_)
    ));
}

// ==================== Buy vs Sell Protection Tests ====================

#[test]
fn test_buy_sell_protection_symmetry() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    let sell_tranche = custom_tranche(3.0, 7.0, 500.0, TrancheSide::SellProtection);
    let buy_tranche = custom_tranche(3.0, 7.0, 500.0, TrancheSide::BuyProtection);

    // Act
    let sell_pv = pricer
        .price_tranche(&sell_tranche, &market, as_of)
        .unwrap()
        .amount();
    let buy_pv = pricer
        .price_tranche(&buy_tranche, &market, as_of)
        .unwrap()
        .amount();

    // Assert
    assert_relative_eq(
        buy_pv,
        -sell_pv,
        0.001,
        "Buy and sell protection should have opposite sign PVs",
    );
}

#[test]
fn test_sell_protection_pv_components() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = custom_tranche(3.0, 7.0, 500.0, TrancheSide::SellProtection);
    let market = standard_market_context();
    let as_of = base_date();

    // Act
    let pv = pricer
        .price_tranche(&tranche, &market, as_of)
        .unwrap()
        .amount();

    // Assert: For sell protection with positive coupon, should receive premium
    // Net PV can be positive or negative depending on spread levels
    assert!(pv.is_finite(), "PV should be finite");
}

// ==================== Maturity Edge Cases ====================

#[test]
fn test_at_maturity_pricing() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();

    // Set as_of to maturity date
    let as_of = tranche.maturity;

    // Act
    let result = pricer.price_tranche(&tranche, &market, as_of);

    // Assert: PV should be zero or minimal at maturity
    assert!(result.is_ok());
    let pv = result.unwrap();
    assert_absolute_eq(
        pv.amount(),
        0.0,
        1e-2,
        "At maturity tranche should have minimal PV",
    );
}

#[test]
fn test_missing_credit_index_errors_for_price_tranche() {
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = MarketContext::new().insert(standard_discount_curve());
    let as_of = base_date();

    let err = pricer
        .price_tranche(&tranche, &market, as_of)
        .expect_err("missing credit index should be rejected");
    let err_text = err.to_string();
    assert!(err_text.contains("CDX.NA.IG.42"));
    assert!(err_text.contains(&tranche.id.to_string()));
}

#[test]
fn test_fully_wiped_tranche_prices_to_zero() {
    let pricer = CDSTranchePricer::new();
    let mut tranche = custom_tranche(3.0, 7.0, 500.0, TrancheSide::SellProtection);
    tranche.accumulated_loss = tranche.detach_pct / 100.0;

    let pv = pricer
        .price_tranche(&tranche, &standard_market_context(), base_date())
        .expect("fully wiped tranche should price")
        .amount();

    assert_absolute_eq(pv, 0.0, 1e-12, "fully wiped tranche should have zero PV");
}

#[test]
fn test_same_day_upfront_has_opposite_sign_for_buy_and_sell() {
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();
    let upfront = Money::new(150_000.0, Currency::USD);

    let sell_base = custom_tranche(3.0, 7.0, 500.0, TrancheSide::SellProtection);
    let buy_base = custom_tranche(3.0, 7.0, 500.0, TrancheSide::BuyProtection);
    let mut sell = custom_tranche(3.0, 7.0, 500.0, TrancheSide::SellProtection);
    let mut buy = custom_tranche(3.0, 7.0, 500.0, TrancheSide::BuyProtection);
    sell.upfront = Some((as_of, upfront));
    buy.upfront = Some((as_of, upfront));

    let sell_base_pv = pricer
        .price_tranche(&sell_base, &market, as_of)
        .expect("sell tranche base PV")
        .amount();
    let buy_base_pv = pricer
        .price_tranche(&buy_base, &market, as_of)
        .expect("buy tranche base PV")
        .amount();
    let sell_pv = pricer
        .price_tranche(&sell, &market, as_of)
        .expect("sell tranche PV")
        .amount();
    let buy_pv = pricer
        .price_tranche(&buy, &market, as_of)
        .expect("buy tranche PV")
        .amount();

    assert_absolute_eq(
        sell_pv - sell_base_pv,
        upfront.amount(),
        1e-6,
        "same-day upfront should add directly to sell-protection PV",
    );
    assert_absolute_eq(
        buy_pv - buy_base_pv,
        -upfront.amount(),
        1e-6,
        "same-day upfront should subtract directly from buy-protection PV",
    );
}

#[test]
fn test_after_maturity_pricing_returns_zero() {
    let pricer = CDSTranchePricer::new();
    let tranche = mezzanine_tranche();
    let market = standard_market_context();
    let as_of = tranche.maturity + time::Duration::days(1);

    let pv = pricer
        .price_tranche(&tranche, &market, as_of)
        .expect("post-maturity pricing should succeed")
        .amount();

    assert_absolute_eq(pv, 0.0, 1e-12, "post-maturity tranche should have zero PV");
}

// ==================== Notional Scaling Tests ====================

#[test]
fn test_pv_scales_with_notional() {
    // Arrange
    let pricer = CDSTranchePricer::new();
    let market = standard_market_context();
    let as_of = base_date();

    let mut tranche_10mm = mezzanine_tranche();
    tranche_10mm.notional = finstack_core::money::Money::new(10_000_000.0, Currency::USD);

    let mut tranche_20mm = mezzanine_tranche();
    tranche_20mm.notional = finstack_core::money::Money::new(20_000_000.0, Currency::USD);

    // Act
    let pv_10 = pricer
        .price_tranche(&tranche_10mm, &market, as_of)
        .unwrap()
        .amount();
    let pv_20 = pricer
        .price_tranche(&tranche_20mm, &market, as_of)
        .unwrap()
        .amount();

    // Assert
    assert_relative_eq(
        pv_20 / pv_10,
        2.0,
        0.001,
        "PV should scale linearly with notional",
    );
}
