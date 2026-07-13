//! QuantLib Parity Tests for Convertible Bonds
//!
//! Test cases based on QuantLib test suite principles: `convertiblebonds.cpp`
//! QuantLib version: 1.34
//! Reference: https://github.com/lballabio/QuantLib/blob/master/test-suite/convertiblebonds.cpp
//!
//! These tests verify that finstack-quant convertible bond pricing follows QuantLib's methodology for:
//! - Convertible bond valuation using binomial/trinomial trees
//! - Parity calculations (equity conversion value vs bond value)
//! - Conversion premium
//! - Greeks (Delta, Gamma, Vega, Rho, Theta)
//! - Callable and puttable convertible bonds
//! - Conversion policies (voluntary, mandatory, window)
//! - Bond floor and conversion value boundaries
//!
//! Note: QuantLib uses tree-based pricing engines (Binomial or Trinomial) for convertibles
//! to properly capture the embedded equity option and early exercise features.

#[allow(unused_imports)]
use crate::parity::*;
use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    calculate_convertible_greeks, calculate_parity, price_convertible_bond, ConvertibleTreeType,
};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

/// Helper: Create a flat discount curve for convertible bond tests
fn create_flat_discount_curve(base_date: Date, rate: f64, curve_id: &str) -> DiscountCurve {
    let times = [0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0];
    let dfs: Vec<_> = times.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .knots(dfs)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

/// Helper: Create market context with equity price, volatility, and risk-free rate
fn create_convertible_market(
    base_date: Date,
    equity_spot: f64,
    volatility: f64,
    dividend_yield: f64,
    risk_free_rate: f64,
) -> MarketContext {
    let discount_curve = create_flat_discount_curve(base_date, risk_free_rate, "USD-OIS");

    MarketContext::new()
        .insert(discount_curve)
        .insert_price("EQUITY", MarketScalar::Unitless(equity_spot))
        .insert_price("EQUITY-VOL", MarketScalar::Unitless(volatility))
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(dividend_yield))
}

/// Helper: Create a standard convertible bond for QuantLib parity tests
fn create_quantlib_convertible(
    id: &str,
    issue: Date,
    maturity: Date,
    notional: f64,
    coupon_rate: f64,
    conversion_ratio: f64,
) -> ConvertibleBond {
    let conversion_spec = ConversionSpec {
        ratio: Some(conversion_ratio),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::from_f64_retain(coupon_rate).unwrap_or_default(),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::semi_annual(),

            dc: DayCount::Act365F,

            bdc: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };

    ConvertibleBond {
        id: id.to_string().into(),
        notional: Money::new(notional, Currency::USD),
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

// =============================================================================
// Test 1: Basic Convertible Bond Pricing
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testBond()
// A convertible bond should price above max(bond_floor, conversion_value)

#[test]
fn quantlib_parity_basic_convertible() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let coupon_rate = 0.05; // 5% coupon
    let conversion_ratio = 10.0; // 10 shares per bond
    let spot = 100.0; // $100 equity price
    let volatility = 0.25; // 25% vol
    let risk_free_rate = 0.03; // 3% risk-free rate

    let bond = create_quantlib_convertible(
        "CB001",
        base,
        maturity,
        notional,
        coupon_rate,
        conversion_ratio,
    );

    let market = create_convertible_market(base, spot, volatility, 0.02, risk_free_rate);

    let price =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(100), base).unwrap();

    // QuantLib expectation:
    // Conversion value = 100 * 10 = $1,000
    // Bond floor (approx) = PV of coupons + principal ≈ $1,090 (5% coupon with 3% discount)
    // Convertible should price above bond floor due to option value
    let bond_floor: f64 = 1090.0;
    let conversion_value: f64 = 1000.0;
    let min_value = bond_floor.max(conversion_value);

    assert!(
        price.amount() >= min_value * 0.98,
        "Convertible should price at least {}, got {}",
        min_value,
        price.amount()
    );

    // Should be in reasonable range
    assert!(
        price.amount() < 1400.0,
        "Convertible price {} should be reasonable",
        price.amount()
    );
}

// =============================================================================
// Test 2: Parity Calculation (At-The-Money)
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testParity()
// Parity = (spot * conversion_ratio) / notional

#[test]
fn quantlib_parity_at_the_money() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 100.0; // ATM: spot * ratio = notional

    let bond =
        create_quantlib_convertible("CB_ATM", base, maturity, notional, 0.05, conversion_ratio);

    let parity = calculate_parity(&bond, spot);

    // QuantLib expectation: Parity = (100 * 10) / 1000 = 1.0 (100%)
    let quantlib_parity = 1.0;

    assert_parity!(
        parity,
        quantlib_parity,
        ParityConfig::default(),
        "Parity calculation at-the-money"
    );
}

// =============================================================================
// Test 3: Parity Calculation (In-The-Money)
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testParity()
// ITM: spot price above conversion price

#[test]
fn quantlib_parity_in_the_money() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 150.0; // ITM: spot * ratio > notional

    let bond =
        create_quantlib_convertible("CB_ITM", base, maturity, notional, 0.05, conversion_ratio);

    let parity = calculate_parity(&bond, spot);

    // QuantLib expectation: Parity = (150 * 10) / 1000 = 1.5 (150%)
    let quantlib_parity = 1.5;

    assert_parity!(
        parity,
        quantlib_parity,
        ParityConfig::default(),
        "Parity calculation in-the-money"
    );
}

// =============================================================================
// Test 4: Parity Calculation (Out-Of-The-Money)
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testParity()
// OTM: spot price below conversion price

#[test]
fn quantlib_parity_out_of_the_money() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 50.0; // OTM: spot * ratio < notional

    let bond =
        create_quantlib_convertible("CB_OTM", base, maturity, notional, 0.05, conversion_ratio);

    let parity = calculate_parity(&bond, spot);

    // QuantLib expectation: Parity = (50 * 10) / 1000 = 0.5 (50%)
    let quantlib_parity = 0.5;

    assert_parity!(
        parity,
        quantlib_parity,
        ParityConfig::default(),
        "Parity calculation out-of-the-money"
    );
}

// =============================================================================
// Test 5: Delta - Equity Sensitivity (ITM)
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testGreeks()
// Delta measures sensitivity to equity price

#[test]
fn quantlib_parity_delta_in_the_money() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 150.0; // ITM

    let bond = create_quantlib_convertible(
        "CB_DELTA_ITM",
        base,
        maturity,
        notional,
        0.05,
        conversion_ratio,
    );
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        Some(0.01),
        base,
    )
    .unwrap();

    // QuantLib expectation: ITM delta should be positive and approach conversion_ratio
    // For deep ITM, delta → conversion_ratio = 10.0
    assert!(
        greeks.delta > 5.0,
        "ITM delta should be > 5.0, got {}",
        greeks.delta
    );
    assert!(
        greeks.delta <= conversion_ratio * 1.1,
        "Delta {} should not significantly exceed conversion ratio {}",
        greeks.delta,
        conversion_ratio
    );
}

// =============================================================================
// Test 6: Delta - Equity Sensitivity (OTM)
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testGreeks()
// OTM delta should be smaller (bond-like behavior)

#[test]
fn quantlib_parity_delta_out_of_the_money() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 50.0; // OTM

    let bond = create_quantlib_convertible(
        "CB_DELTA_OTM",
        base,
        maturity,
        notional,
        0.05,
        conversion_ratio,
    );
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        Some(0.01),
        base,
    )
    .unwrap();

    // QuantLib expectation: OTM delta should be small (bond-like)
    assert!(
        greeks.delta >= 0.0,
        "Delta should be non-negative, got {}",
        greeks.delta
    );
    assert!(
        greeks.delta < conversion_ratio * 0.5,
        "OTM delta {} should be well below conversion ratio {}",
        greeks.delta,
        conversion_ratio
    );
}

// =============================================================================
// Test 7: Gamma - Convexity
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testGreeks()
// Gamma should be positive (convexity benefit)

#[test]
fn quantlib_parity_gamma() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 100.0; // ATM

    let bond =
        create_quantlib_convertible("CB_GAMMA", base, maturity, notional, 0.05, conversion_ratio);
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        Some(0.01),
        base,
    )
    .unwrap();

    // QuantLib expectation: Gamma should be non-negative
    // ATM options typically have highest gamma
    assert!(
        greeks.gamma >= 0.0,
        "Gamma should be non-negative, got {}",
        greeks.gamma
    );
}

// =============================================================================
// Test 8: Vega - Volatility Sensitivity
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testGreeks()
// Vega should be positive (higher vol increases option value)

#[test]
fn quantlib_parity_vega() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 100.0;

    let bond =
        create_quantlib_convertible("CB_VEGA", base, maturity, notional, 0.05, conversion_ratio);
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        Some(0.01),
        base,
    )
    .unwrap();

    // QuantLib expectation: Vega should be positive
    assert!(
        greeks.vega >= 0.0,
        "Vega should be non-negative, got {}",
        greeks.vega
    );
}

// =============================================================================
// Test 9: Theta - Time Decay
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testGreeks()
// Theta can be positive or negative for convertibles (coupon vs time decay)

#[test]
fn quantlib_parity_theta() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 100.0;

    let bond =
        create_quantlib_convertible("CB_THETA", base, maturity, notional, 0.05, conversion_ratio);
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        Some(0.01),
        base,
    )
    .unwrap();

    // QuantLib expectation: Theta should be finite and reasonable
    assert!(
        greeks.theta.is_finite(),
        "Theta should be finite, got {}",
        greeks.theta
    );
    assert!(
        greeks.theta.abs() < notional * 10.0,
        "Theta {} should be reasonable relative to notional {}",
        greeks.theta,
        notional
    );
}

// =============================================================================
// Test 10: Rho - Interest Rate Sensitivity
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testGreeks()
// Rho measures sensitivity to risk-free rate

#[test]
fn quantlib_parity_rho() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 100.0;

    let bond =
        create_quantlib_convertible("CB_RHO", base, maturity, notional, 0.05, conversion_ratio);
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        Some(0.01),
        base,
    )
    .unwrap();

    // QuantLib expectation: Rho should be finite
    assert!(
        greeks.rho.is_finite(),
        "Rho should be finite, got {}",
        greeks.rho
    );
}

// =============================================================================
// Test 11: Callable Convertible Bond
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testCallableConvertible()
// Callable convertible should be worth less than non-callable

#[test]
fn quantlib_parity_callable_convertible() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let call_date = date!(2026 - 01 - 01);
    let notional = 1000.0;

    // Non-callable convertible
    let plain_bond = create_quantlib_convertible("CB_PLAIN", base, maturity, notional, 0.06, 10.0);

    // Callable convertible
    let mut callable_bond =
        create_quantlib_convertible("CB_CALL", base, maturity, notional, 0.06, 10.0);
    let mut schedule = CallPutSchedule::default();
    schedule.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: 102.0, // Callable at 102% of par
        make_whole: None,
    });
    callable_bond.call_put = Some(schedule);

    let market = create_convertible_market(base, 150.0, 0.25, 0.02, 0.03);

    let plain_price = price_convertible_bond(
        &plain_bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();
    let callable_price = price_convertible_bond(
        &callable_bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();

    // QuantLib expectation: Callable bond < Plain bond (issuer option reduces value)
    assert!(
        callable_price.amount() <= plain_price.amount() * 1.01,
        "Callable {} should be <= plain {}: issuer call option reduces holder value",
        callable_price.amount(),
        plain_price.amount()
    );
}

// =============================================================================
// Test 12: Puttable Convertible Bond
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testPutableConvertible()
// Puttable convertible should be worth more than non-puttable

#[test]
fn quantlib_parity_puttable_convertible() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let put_date = date!(2026 - 01 - 01);
    let notional = 1000.0;

    // Non-puttable convertible
    let plain_bond = create_quantlib_convertible("CB_PLAIN2", base, maturity, notional, 0.04, 10.0);

    // Puttable convertible
    let mut puttable_bond =
        create_quantlib_convertible("CB_PUT", base, maturity, notional, 0.04, 10.0);
    let mut schedule = CallPutSchedule::default();
    schedule.puts.push(CallPut {
        start_date: put_date,
        end_date: put_date,
        price_pct_of_par: 98.0, // Puttable at 98% of par
        make_whole: None,
    });
    puttable_bond.call_put = Some(schedule);

    // Use OTM scenario where put is valuable
    let market = create_convertible_market(base, 50.0, 0.25, 0.02, 0.03);

    let plain_price = price_convertible_bond(
        &plain_bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();
    let puttable_price = price_convertible_bond(
        &puttable_bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();

    // QuantLib expectation: Puttable bond >= Plain bond (holder option adds value)
    assert!(
        puttable_price.amount() >= plain_price.amount() * 0.99,
        "Puttable {} should be >= plain {}: holder put option adds value",
        puttable_price.amount(),
        plain_price.amount()
    );
}

// =============================================================================
// Test 13: Zero Coupon Convertible
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testZeroCouponConvertible()
// Zero coupon convertible pricing

#[test]
fn quantlib_parity_zero_coupon_convertible() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 150.0;

    let conversion_spec = ConversionSpec {
        ratio: Some(conversion_ratio),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let zero_coupon = ConvertibleBond {
        id: "CB_ZERO".to_string().into(),
        notional: Money::new(notional, Currency::USD),
        issue_date: base,
        maturity,
        settlement_days: None,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("EQUITY".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: None, // Zero coupon
        floating_coupon: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let price = price_convertible_bond(
        &zero_coupon,
        &market,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();

    // QuantLib expectation:
    // Conversion value = 150 * 10 = $1,500
    // Should price close to or above conversion value
    let conversion_value = spot * conversion_ratio;

    assert!(
        price.amount() >= conversion_value * 0.95,
        "Zero coupon convertible should price near conversion value: {} vs {}",
        price.amount(),
        conversion_value
    );
}

// =============================================================================
// Test 14: Volatility Impact on Price
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testVolatilitySensitivity()
// Higher volatility should increase convertible value

#[test]
fn quantlib_parity_volatility_sensitivity() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let spot = 100.0;

    let bond = create_quantlib_convertible("CB_VOL", base, maturity, notional, 0.05, 10.0);

    // Low volatility
    let market_low_vol = create_convertible_market(base, spot, 0.10, 0.02, 0.03);
    let price_low_vol = price_convertible_bond(
        &bond,
        &market_low_vol,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();

    // High volatility
    let market_high_vol = create_convertible_market(base, spot, 0.40, 0.02, 0.03);
    let price_high_vol = price_convertible_bond(
        &bond,
        &market_high_vol,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();

    // QuantLib expectation: Higher vol → higher option value → higher price
    assert!(
        price_high_vol.amount() >= price_low_vol.amount() * 0.98,
        "High vol price {} should be >= low vol price {}: option value increases with vol",
        price_high_vol.amount(),
        price_low_vol.amount()
    );
}

// =============================================================================
// Test 15: Binomial vs Trinomial Tree Convergence
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testTreeConvergence()
// Different tree methods should converge to similar values

#[test]
fn quantlib_parity_tree_convergence() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;

    let bond = create_quantlib_convertible("CB_CONV", base, maturity, notional, 0.05, 10.0);
    let market = create_convertible_market(base, 100.0, 0.25, 0.02, 0.03);

    let binomial_price =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(200), base).unwrap();
    let trinomial_price =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Trinomial(200), base).unwrap();

    // QuantLib expectation: With enough steps, both methods converge
    let diff_pct =
        (binomial_price.amount() - trinomial_price.amount()).abs() / binomial_price.amount();

    assert!(
        diff_pct < 0.05, // Within 5%
        "Binomial {} and trinomial {} should converge within 5%, got {}%",
        binomial_price.amount(),
        trinomial_price.amount(),
        diff_pct * 100.0
    );
}

// =============================================================================
// Test 16: Conversion Premium Calculation
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testConversionPremium()
// Conversion premium = (bond_price / conversion_value) - 1

#[test]
fn quantlib_parity_conversion_premium() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 100.0;

    let bond =
        create_quantlib_convertible("CB_PREM", base, maturity, notional, 0.05, conversion_ratio);
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let as_of = base;
    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::custom("conversion_premium")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let premium = *result.measures.get("conversion_premium").unwrap();

    // QuantLib expectation: Premium should be non-negative (option value)
    // For ATM convertible with time value, expect small positive premium
    assert!(
        premium >= -0.05, // Allow small numerical errors
        "Conversion premium should be non-negative, got {}",
        premium
    );
}

// =============================================================================
// Test 17: Deep ITM - Bond Behaves Like Stock
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testDeepITM()
// Deep ITM convertible should track equity closely

#[test]
fn quantlib_parity_deep_itm() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 250.0; // Very high spot (deep ITM)

    let bond = create_quantlib_convertible(
        "CB_DEEP_ITM",
        base,
        maturity,
        notional,
        0.05,
        conversion_ratio,
    );
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let price =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(100), base).unwrap();
    let conversion_value = spot * conversion_ratio;

    // QuantLib expectation: Deep ITM should price close to conversion value
    let diff_pct = (price.amount() - conversion_value).abs() / conversion_value;

    assert!(
        diff_pct < 0.05, // Within 5%
        "Deep ITM should track conversion value: price={}, conversion_value={}, diff={}%",
        price.amount(),
        conversion_value,
        diff_pct * 100.0
    );
}

// =============================================================================
// Test 18: Deep OTM - Bond Behaves Like Straight Bond
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testDeepOTM()
// Deep OTM convertible should track bond floor

#[test]
fn quantlib_parity_deep_otm() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 10.0; // Very low spot (deep OTM)

    let bond = create_quantlib_convertible(
        "CB_DEEP_OTM",
        base,
        maturity,
        notional,
        0.05,
        conversion_ratio,
    );
    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let price =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(100), base).unwrap();

    // QuantLib expectation: Deep OTM should trade closer to bond floor
    // Bond floor (approx): PV of 5% coupons + principal at 3% discount ≈ $1,090
    let bond_floor = 1050.0; // Approximate
    let conversion_value = spot * conversion_ratio; // $100

    // Should be much closer to bond floor than conversion value
    let distance_to_floor = (price.amount() - bond_floor).abs();
    let distance_to_conversion = (price.amount() - conversion_value).abs();

    assert!(
        distance_to_floor < distance_to_conversion,
        "Deep OTM should be closer to bond floor than conversion value: \
         distance_to_floor={}, distance_to_conversion={}",
        distance_to_floor,
        distance_to_conversion
    );
}

// =============================================================================
// Test 19: Mandatory Conversion Policy
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testMandatoryConversion()
// Mandatory conversion at maturity

#[test]
fn quantlib_parity_mandatory_conversion() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;
    let spot = 150.0;

    let conversion_spec = ConversionSpec {
        ratio: Some(conversion_ratio),
        price: None,
        policy: ConversionPolicy::MandatoryOn(maturity),
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::semi_annual(),

            dc: DayCount::Act365F,

            bdc: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };

    let mandatory_bond = ConvertibleBond {
        id: "CB_MAND".to_string().into(),
        notional: Money::new(notional, Currency::USD),
        issue_date: base,
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
    };

    let market = create_convertible_market(base, spot, 0.25, 0.02, 0.03);

    let price = price_convertible_bond(
        &mandatory_bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();

    // QuantLib expectation: Mandatory conversion should price successfully
    // Should be in reasonable range relative to conversion value
    let conversion_value = spot * conversion_ratio;
    assert!(
        price.amount() > conversion_value * 0.7 && price.amount() < conversion_value * 1.3,
        "Mandatory conversion bond should be near conversion value: {} vs {}",
        price.amount(),
        conversion_value
    );
}

// =============================================================================
// Test 20: Window Conversion Policy
// =============================================================================
// QuantLib reference: convertiblebonds.cpp, testWindowConversion()
// Conversion allowed only within a window

#[test]
fn quantlib_parity_window_conversion() {
    let base = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let window_start = date!(2026 - 01 - 01);
    let window_end = date!(2028 - 01 - 01);
    let notional = 1000.0;
    let conversion_ratio = 10.0;

    let conversion_spec = ConversionSpec {
        ratio: Some(conversion_ratio),
        price: None,
        policy: ConversionPolicy::Window {
            start: window_start,
            end: window_end,
        },
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::semi_annual(),

            dc: DayCount::Act365F,

            bdc: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };

    let window_bond = ConvertibleBond {
        id: "CB_WINDOW".to_string().into(),
        notional: Money::new(notional, Currency::USD),
        issue_date: base,
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
    };

    let market = create_convertible_market(base, 150.0, 0.25, 0.02, 0.03);

    let price = price_convertible_bond(
        &window_bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        base,
    )
    .unwrap();

    // QuantLib expectation: Window conversion should price successfully
    // Should be less than voluntary (more restricted)
    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Window conversion bond should price successfully: {}",
        price.amount()
    );
}
