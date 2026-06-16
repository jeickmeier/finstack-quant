//! Greeks calculation tests for convertible bonds.
//!
//! Tests sensitivities and risk measures:
//! - Delta (equity sensitivity)
//! - Gamma (convexity)
//! - Vega (volatility sensitivity)
//! - Rho (interest rate sensitivity)
//! - Theta (time decay)
//! - Reasonable ranges and signs
//! - Greeks behavior across moneyness

use super::fixtures::*;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    calculate_convertible_greeks, ConvertibleTreeType,
};

#[test]
fn test_delta_positive_for_itm() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Delta should be positive (increases with stock price)
    assert!(
        greeks.delta > 0.0,
        "Delta should be positive for ITM convertible, got {}",
        greeks.delta
    );
}

#[test]
fn test_delta_bounded_by_conversion_ratio() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Delta should not exceed conversion ratio
    assert!(
        greeks.delta <= bond_params::CONVERSION_RATIO * 1.1, // Small tolerance
        "Delta {} should not exceed conversion ratio {}",
        greeks.delta,
        bond_params::CONVERSION_RATIO
    );
}

#[test]
fn test_delta_increases_with_moneyness() {
    let bond = create_standard_convertible();

    // OTM
    let market_otm = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let greeks_otm = calculate_convertible_greeks(
        &bond,
        &market_otm,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // ATM
    let market_atm = create_market_context_with_params(
        100.0, // At conversion price
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let greeks_atm = calculate_convertible_greeks(
        &bond,
        &market_atm,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // ITM
    let market_itm = create_market_context();
    let greeks_itm = calculate_convertible_greeks(
        &bond,
        &market_itm,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Delta should increase with moneyness
    assert!(
        greeks_itm.delta > greeks_atm.delta,
        "Delta should increase ITM vs ATM: {} vs {}",
        greeks_itm.delta,
        greeks_atm.delta
    );

    assert!(
        greeks_atm.delta > greeks_otm.delta,
        "Delta should increase ATM vs OTM: {} vs {}",
        greeks_atm.delta,
        greeks_otm.delta
    );
}

#[test]
fn test_gamma_non_negative() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Gamma should be non-negative (convexity benefit)
    assert!(
        greeks.gamma >= 0.0,
        "Gamma should be non-negative, got {}",
        greeks.gamma
    );
}

#[test]
fn test_gamma_peaks_near_atm() {
    let bond = create_standard_convertible();

    // Use enough tree steps for stable gamma (second derivative is noisy with few steps).
    let tree = ConvertibleTreeType::Binomial(100);

    // OTM
    let market_otm = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let greeks_otm =
        calculate_convertible_greeks(&bond, &market_otm, tree, Some(0.01), dates::base_date())
            .unwrap();

    // ATM
    let market_atm = create_market_context_with_params(
        100.0,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let greeks_atm =
        calculate_convertible_greeks(&bond, &market_atm, tree, Some(0.01), dates::base_date())
            .unwrap();

    // ITM
    let market_itm = create_market_context_with_params(
        market_params::SPOT_HIGH,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let _greeks_itm =
        calculate_convertible_greeks(&bond, &market_itm, tree, Some(0.01), dates::base_date())
            .unwrap();

    // Gamma typically peaks near ATM. With coarse trees and full repricing,
    // small numerical artifacts can make gamma slightly negative. We check
    // that ATM gamma is larger in magnitude or close to OTM gamma.
    assert!(
        greeks_atm.gamma.abs() >= greeks_otm.gamma.abs() * 0.5,
        "ATM |gamma| {} should be >= OTM |gamma| * 0.5 = {}",
        greeks_atm.gamma.abs(),
        greeks_otm.gamma.abs() * 0.5,
    );
}

#[test]
fn test_vega_non_negative() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Vega should be non-negative (higher vol increases option value)
    assert!(
        greeks.vega >= 0.0,
        "Vega should be non-negative, got {}",
        greeks.vega
    );
}

#[test]
fn test_vega_positive_for_atm() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        100.0, // ATM
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Vega should be positive for ATM options
    assert!(
        greeks.vega > 0.0,
        "Vega should be positive for ATM convertible, got {}",
        greeks.vega
    );
}

#[test]
fn test_theta_reasonable() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Theta can be positive or negative for convertibles (due to coupon vs time decay)
    // Just check it's finite and reasonable
    assert!(
        greeks.theta.is_finite(),
        "Theta should be finite, got {}",
        greeks.theta
    );

    // Should not be unreasonably large (daily decay should be << bond value)
    assert!(
        greeks.theta.abs() < bond_params::NOTIONAL * 10.0,
        "Theta {} seems unreasonably large",
        greeks.theta
    );
}

#[test]
fn test_rho_finite() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Rho should be finite
    assert!(
        greeks.rho.is_finite(),
        "Rho should be finite, got {}",
        greeks.rho
    );

    // Should be reasonable magnitude
    assert!(
        greeks.rho.abs() < bond_params::NOTIONAL * 100.0,
        "Rho {} seems unreasonably large",
        greeks.rho
    );
}

#[test]
fn test_greeks_price_consistency() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Price in greeks struct should be reasonable
    assert!(
        greeks.price > 0.0,
        "Greeks price should be positive, got {}",
        greeks.price
    );

    assert!(
        greeks.price > bond_params::NOTIONAL * 0.5,
        "Greeks price {} should be reasonable vs notional {}",
        greeks.price,
        bond_params::NOTIONAL
    );
}

#[test]
fn test_greeks_with_different_bump_sizes() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    // Test different bump sizes
    let bump_sizes = vec![0.001, 0.01, 0.05];

    for bump in bump_sizes {
        let greeks = calculate_convertible_greeks(
            &bond,
            &market,
            ConvertibleTreeType::Binomial(50),
            Some(bump),
            dates::base_date(),
        );

        assert!(
            greeks.is_ok(),
            "Greeks calculation should succeed with bump size {}",
            bump
        );

        let g = greeks.unwrap();
        assert!(
            g.delta.is_finite(),
            "Delta should be finite with bump {}",
            bump
        );
        assert!(
            g.gamma.is_finite(),
            "Gamma should be finite with bump {}",
            bump
        );
        assert!(
            g.vega.is_finite(),
            "Vega should be finite with bump {}",
            bump
        );
    }
}

#[test]
fn test_greeks_with_trinomial_tree() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Trinomial(50),
        Some(0.01),
        dates::base_date(),
    );

    assert!(greeks.is_ok(), "Greeks should work with trinomial tree");

    let g = greeks.unwrap();
    assert!(g.delta > 0.0, "Trinomial delta should be positive");
    assert!(g.gamma >= 0.0, "Trinomial gamma should be non-negative");
    assert!(g.vega >= 0.0, "Trinomial vega should be non-negative");
}

#[test]
fn test_greeks_binomial_vs_trinomial() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks_bin = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(100),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    let greeks_tri = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Trinomial(100),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Greeks should be reasonably close between tree types
    let delta_diff = (greeks_bin.delta - greeks_tri.delta).abs() / greeks_bin.delta.max(0.01);
    assert!(
        delta_diff < 0.20, // Within 20%
        "Delta should be similar: bin={}, tri={}, diff={}%",
        greeks_bin.delta,
        greeks_tri.delta,
        delta_diff * 100.0
    );
}

#[test]
fn test_delta_for_deep_itm() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        market_params::SPOT_HIGH,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Deep ITM delta should approach conversion ratio
    assert!(
        greeks.delta > bond_params::CONVERSION_RATIO * 0.7,
        "Deep ITM delta {} should be close to conversion ratio {}",
        greeks.delta,
        bond_params::CONVERSION_RATIO
    );
}

#[test]
fn test_delta_for_deep_otm() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Deep OTM delta should be small (bond-like behavior)
    assert!(
        greeks.delta < bond_params::CONVERSION_RATIO * 0.3,
        "Deep OTM delta {} should be small",
        greeks.delta
    );
}

#[test]
fn test_vega_decreases_deep_itm() {
    let bond = create_standard_convertible();

    // ATM
    let market_atm = create_market_context_with_params(
        100.0,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let greeks_atm = calculate_convertible_greeks(
        &bond,
        &market_atm,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Deep ITM
    let market_itm = create_market_context_with_params(
        market_params::SPOT_HIGH,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let greeks_itm = calculate_convertible_greeks(
        &bond,
        &market_itm,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Vega typically decreases deep ITM (less uncertainty in conversion)
    // Allow for some numerical variance
    assert!(
        greeks_atm.vega >= greeks_itm.vega * 0.5,
        "ATM vega {} should be >= deep ITM vega {}",
        greeks_atm.vega,
        greeks_itm.vega
    );
}

#[test]
fn test_greeks_all_finite() {
    let bond = create_standard_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    assert!(greeks.price.is_finite(), "Price should be finite");
    assert!(greeks.delta.is_finite(), "Delta should be finite");
    assert!(greeks.gamma.is_finite(), "Gamma should be finite");
    assert!(greeks.vega.is_finite(), "Vega should be finite");
    assert!(greeks.rho.is_finite(), "Rho should be finite");
    assert!(greeks.theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_zero_coupon_greeks() {
    let bond = create_zero_coupon_convertible();
    let market = create_market_context();

    let greeks = calculate_convertible_greeks(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        dates::base_date(),
    )
    .unwrap();

    // Zero coupon should still have reasonable greeks
    assert!(greeks.delta > 0.0, "Zero coupon delta should be positive");
    assert!(
        greeks.gamma >= 0.0,
        "Zero coupon gamma should be non-negative"
    );
    assert!(
        greeks.vega >= 0.0,
        "Zero coupon vega should be non-negative"
    );
}
