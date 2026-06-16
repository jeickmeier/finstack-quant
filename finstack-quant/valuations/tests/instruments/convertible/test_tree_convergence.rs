//! Tree convergence tests for convertible bond pricing.
//!
//! Verifies that the binomial tree pricer converges to a stable value as
//! the number of tree steps increases.
//!
//! **Market Standards Review (Priority 1 - CRITICAL)**
//!
//! Convergence properties tested:
//! - Error decreases as step count increases (monotonic convergence)
//! - Final price stabilizes within tolerance (asymptotic stability)
//! - Trinomial tree achieves similar convergence behavior
//! - Richardson extrapolation test for O(1/N) convergence rate

use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleTreeType,
};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};
use time::Month;

use crate::common::test_helpers::tolerances;

/// Create a simple convertible bond for convergence testing.
///
/// Parameters chosen for numerical stability:
/// - 5Y maturity (sufficient for tree to demonstrate convergence)
/// - 5% coupon (typical for convertible)
/// - 10:1 conversion ratio (ATM conversion at $100 spot)
fn create_test_convertible() -> ConvertibleBond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let conversion_spec = ConversionSpec {
        ratio: Some(10.0), // 10 shares per $1000 bond → conversion price = $100
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::try_from(0.05).expect("valid"), // 5% coupon
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    ConvertibleBond {
        id: "CONV-TEST".to_string().into(),
        notional: Money::new(1000.0, Currency::USD),
        issue_date: issue,
        maturity,
        settlement_days: None,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("EQUITY".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

/// Create a market context with flat curves for deterministic testing.
///
/// Parameters:
/// - 5% discount rate (flat curve)
/// - $100 spot price (ATM for 10:1 conversion ratio)
/// - 25% implied volatility
/// - 2% dividend yield
fn create_test_market(base_date: Date) -> MarketContext {
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.05_f64).exp()),
            (5.0, (-0.05 * 5.0_f64).exp()),
            (10.0, (-0.05 * 10.0_f64).exp()),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .build()
        .expect("should build curve");

    MarketContext::new()
        .insert(discount_curve)
        .insert_price("EQUITY", MarketScalar::Unitless(100.0)) // ATM
        .insert_price("EQUITY-VOL", MarketScalar::Unitless(0.25)) // 25% vol
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.02)) // 2% div yield
}

#[test]
fn test_tree_convergence_binomial() {
    let bond = create_test_convertible();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let market = create_test_market(as_of);

    // Price with increasing tree steps
    let price_100 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(100), as_of)
            .expect("pricing should succeed")
            .amount();

    let price_500 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(500), as_of)
            .expect("pricing should succeed")
            .amount();

    let price_1000 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(1000), as_of)
            .expect("pricing should succeed")
            .amount();

    // Verify all prices are finite and positive
    assert!(
        price_100.is_finite() && price_100 > 0.0,
        "N=100 price should be finite positive"
    );
    assert!(
        price_500.is_finite() && price_500 > 0.0,
        "N=500 price should be finite positive"
    );
    assert!(
        price_1000.is_finite() && price_1000 > 0.0,
        "N=1000 price should be finite positive"
    );

    // Convergence error should decrease
    let error_100_500 = (price_500 - price_100).abs();
    let error_500_1000 = (price_1000 - price_500).abs();

    assert!(
        error_500_1000 < error_100_500,
        "Convergence error should decrease: err(100→500)={:.4}, err(500→1000)={:.4}",
        error_100_500,
        error_500_1000
    );

    // Final price should be stable (within 0.1%)
    let stability = (price_1000 - price_500).abs() / price_1000;
    assert!(
        stability < 0.001,
        "Price should stabilize at N=1000: relative change={:.4}% (expected <0.1%)",
        stability * 100.0
    );
}

#[test]
fn test_tree_convergence_trinomial() {
    let bond = create_test_convertible();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let market = create_test_market(as_of);

    // Price with increasing tree steps
    let price_50 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Trinomial(50), as_of)
            .expect("pricing should succeed")
            .amount();

    let price_200 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Trinomial(200), as_of)
            .expect("pricing should succeed")
            .amount();

    let price_500 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Trinomial(500), as_of)
            .expect("pricing should succeed")
            .amount();

    // Verify all prices are finite and positive
    assert!(
        price_50.is_finite() && price_50 > 0.0,
        "N=50 price should be finite positive"
    );
    assert!(
        price_200.is_finite() && price_200 > 0.0,
        "N=200 price should be finite positive"
    );
    assert!(
        price_500.is_finite() && price_500 > 0.0,
        "N=500 price should be finite positive"
    );

    // Convergence error should decrease
    let error_50_200 = (price_200 - price_50).abs();
    let error_200_500 = (price_500 - price_200).abs();

    assert!(
        error_200_500 < error_50_200,
        "Trinomial convergence error should decrease: err(50→200)={:.4}, err(200→500)={:.4}",
        error_50_200,
        error_200_500
    );

    // Final price should be stable
    let stability = (price_500 - price_200).abs() / price_500;
    assert!(
        stability < 0.005, // 0.5% for trinomial (fewer steps tested)
        "Trinomial price should stabilize: relative change={:.4}%",
        stability * 100.0
    );
}

#[test]
fn test_binomial_trinomial_consistency() {
    let bond = create_test_convertible();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let market = create_test_market(as_of);

    // Price with high step count for both tree types
    let price_binomial =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(500), as_of)
            .expect("binomial pricing should succeed")
            .amount();

    let price_trinomial =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Trinomial(500), as_of)
            .expect("trinomial pricing should succeed")
            .amount();

    // Both trees should converge to similar values (within 1%)
    let relative_diff = (price_binomial - price_trinomial).abs() / price_binomial;
    assert!(
        relative_diff < tolerances::STATISTICAL, // 1%
        "Binomial ({:.2}) and Trinomial ({:.2}) should agree within 1%, diff={:.2}%",
        price_binomial,
        price_trinomial,
        relative_diff * 100.0
    );
}

#[test]
fn test_convergence_rate_order_one() {
    // Richardson extrapolation test: For binomial trees, convergence is O(1/N).
    // Error(N) ≈ C/N, so Error(N)/Error(2N) ≈ 2
    let bond = create_test_convertible();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let market = create_test_market(as_of);

    let price_100 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(100), as_of)
            .expect("pricing should succeed")
            .amount();

    let price_200 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(200), as_of)
            .expect("pricing should succeed")
            .amount();

    let price_400 =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(400), as_of)
            .expect("pricing should succeed")
            .amount();

    // Use price_400 as reference for true value
    let error_100 = (price_100 - price_400).abs();
    let error_200 = (price_200 - price_400).abs();

    // For O(1/N) convergence: Error(100)/Error(200) ≈ 2
    // Allow some tolerance due to finite precision
    if error_200 > 1e-6 {
        // Only test if error is meaningful
        let ratio = error_100 / error_200;
        assert!(
            ratio > 1.2 && ratio < 6.0,
            "Convergence ratio should be ~2 for O(1/N): err(100)={:.6}, err(200)={:.6}, ratio={:.2}",
            error_100,
            error_200,
            ratio
        );
    }
}

#[test]
fn test_price_bounds_validity() {
    // Verify price respects economic bounds
    let bond = create_test_convertible();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let market = create_test_market(as_of);

    let price = price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(200), as_of)
        .expect("pricing should succeed")
        .amount();

    // Conversion value: 10 shares * $100 = $1000
    let conversion_value = 10.0 * 100.0;

    // Price should be at least conversion value (no-arbitrage)
    assert!(
        price >= conversion_value - 1.0, // Allow small numerical tolerance
        "Price ({:.2}) should be >= conversion value ({:.2})",
        price,
        conversion_value
    );

    // Price should be reasonable (not more than conversion value + bond value)
    // Bond floor ≈ notional ≈ $1000, so max ≈ $2000
    assert!(
        price < 2500.0,
        "Price ({:.2}) should be reasonable (< $2500)",
        price
    );
}
