//! Basic pricing tests for convertible bonds.
//!
//! Tests fundamental valuation concepts:
//! - Parity calculation
//! - Conversion value
//! - Bond floor (debt component value)
//! - Basic present value calculation
//! - Conversion ratio vs conversion price equivalence

use super::fixtures::*;
use finstack_core::currency::Currency;
use finstack_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleTreeType,
};

#[test]
fn test_conversion_value_itm() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let conversion_value =
        theoretical_conversion_value(market_params::SPOT_PRICE, bond_params::CONVERSION_RATIO);

    // Convertible should be worth at least conversion value
    assert!(
        price.amount() >= conversion_value * 0.99, // Small tolerance for discretization
        "Price {} should be at least conversion value {}",
        price.amount(),
        conversion_value
    );
}

#[test]
fn test_pricing_exceeds_bond_floor() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Bond floor approximation (straight bond value)
    // With 5% coupon and 3% risk-free rate, bond trades above par
    let approx_bond_floor = calculate_bond_floor(
        bond_params::COUPON_RATE,
        5.0, // 5 years
        market_params::RISK_FREE_RATE,
    ) * bond_params::NOTIONAL;

    // Convertible should be worth at least the bond floor
    assert!(
        price.amount() >= approx_bond_floor * 0.95, // Allow for approximation
        "Price {} should exceed bond floor {}",
        price.amount(),
        approx_bond_floor
    );
}

#[test]
fn test_pricing_respects_max_value() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // For ITM convertible, value should be close to max(bond_floor, conversion_value)
    let conversion_value =
        theoretical_conversion_value(market_params::SPOT_PRICE, bond_params::CONVERSION_RATIO);

    // Price should be in a reasonable range (conversion value + some time/volatility value)
    assert!(
        price.amount() < conversion_value * 1.2,
        "Price {} should not vastly exceed conversion value {}",
        price.amount(),
        conversion_value
    );
}

#[test]
fn test_conversion_ratio_vs_price_equivalence() {
    let bond_ratio = create_standard_convertible();
    let bond_price = create_convertible_with_conversion_price();
    let market = create_market_context();

    let price_ratio = price_convertible_bond(
        &bond_ratio,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let price_price = price_convertible_bond(
        &bond_price,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Should produce nearly identical prices since ratio = notional / price
    let diff_pct = (price_ratio.amount() - price_price.amount()).abs() / price_ratio.amount();
    assert!(
        diff_pct < PRICE_TOLERANCE_PCT,
        "Conversion ratio and price should yield equivalent values: {} vs {}, diff {}%",
        price_ratio.amount(),
        price_price.amount(),
        diff_pct * 100.0
    );
}

#[test]
fn test_zero_coupon_convertible_pricing() {
    let bond = create_zero_coupon_convertible();
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let conversion_value =
        theoretical_conversion_value(market_params::SPOT_PRICE, bond_params::CONVERSION_RATIO);

    // Zero coupon convertible should still be worth at least conversion value
    assert!(
        price.amount() >= conversion_value * 0.98,
        "Zero coupon price {} should be at least conversion value {}",
        price.amount(),
        conversion_value
    );

    // Should be less than coupon-bearing convertible (all else equal)
    let coupon_bond = create_standard_convertible();
    let coupon_price = price_convertible_bond(
        &coupon_bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() < coupon_price.amount(),
        "Zero coupon {} should be less valuable than coupon-bearing {}",
        price.amount(),
        coupon_price.amount()
    );
}

#[test]
fn test_deep_itm_convertible() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        market_params::SPOT_HIGH,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let conversion_value =
        theoretical_conversion_value(market_params::SPOT_HIGH, bond_params::CONVERSION_RATIO);

    // Deep ITM convertible should track equity closely
    assert!(
        price.amount() >= conversion_value * 0.98,
        "Deep ITM price {} should be close to conversion value {}",
        price.amount(),
        conversion_value
    );
}

#[test]
fn test_deep_otm_convertible() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Deep OTM convertible should trade closer to bond floor
    // Bond floor with 5% coupon and 3% rate trades above par
    let approx_bond_floor =
        calculate_bond_floor(bond_params::COUPON_RATE, 5.0, market_params::RISK_FREE_RATE)
            * bond_params::NOTIONAL;

    // Should be closer to bond floor than conversion value
    let conversion_value =
        theoretical_conversion_value(market_params::SPOT_LOW, bond_params::CONVERSION_RATIO);

    let distance_to_floor = (price.amount() - approx_bond_floor).abs();
    let distance_to_conversion = (price.amount() - conversion_value).abs();

    assert!(
        distance_to_floor < distance_to_conversion,
        "OTM convertible should be closer to bond floor than conversion value"
    );
}

#[test]
fn test_currency_consistency() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert_eq!(
        price.currency(),
        Currency::USD,
        "Price currency should match bond currency"
    );
}

#[test]
fn test_reasonable_price_range() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Price should be in a reasonable range
    // Min: ~bond floor or conversion value
    // Max: ~conversion value + significant time/volatility premium
    assert!(
        price.amount() >= 800.0 && price.amount() <= 3000.0,
        "Price {} should be in reasonable range",
        price.amount()
    );
}
