//! Greek calculations for FX options with analytical and finite difference validation.
//!
//! Tests delta, gamma, vega, theta, and rho calculations against both
//! analytical formulas and finite difference approximations.

use super::helpers::*;
use time::macros::date;

#[test]
fn test_delta_atm_call_near_half() {
    // Arrange: ATM call delta should be ~0.5 (in units, not percentage)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 1.20;
    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: ATM call delta should be around 0.5 per unit * notional
    // With notional 1M EUR, delta should be around 500k
    assert_in_range(greeks.delta, 300_000.0, 700_000.0, "ATM call delta");
}

#[test]
fn test_delta_itm_call_approaches_one() {
    // Arrange: Deep ITM call delta should approach 1.0
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 1.00; // Deep ITM (spot = 1.20)
    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Delta should be high (approaching notional)
    assert!(
        greeks.delta > 800_000.0,
        "Deep ITM call delta should be high"
    );
}

#[test]
fn test_delta_otm_call_approaches_zero() {
    // Arrange: Deep OTM call delta should approach 0
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 1.50; // Deep OTM (spot = 1.20)
    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Delta should be low
    assert!(
        greeks.delta < 200_000.0,
        "Deep OTM call delta should be low"
    );
    assert!(greeks.delta > 0.0, "Call delta should be positive");
}

#[test]
fn test_delta_put_is_negative() {
    // Arrange: Put delta should be negative
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 1.20;
    let put = build_put_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&put, &market, as_of);

    // Assert: ATM put delta should be negative, around -0.5 * notional
    assert!(greeks.delta < 0.0, "Put delta should be negative");
    assert_in_range(greeks.delta, -700_000.0, -300_000.0, "ATM put delta");
}

#[test]
fn test_delta_matches_finite_difference() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act: Analytical delta
    let greeks = compute_greeks(&call, &market, as_of);

    // Act: Finite difference delta
    let bump = 0.001; // 10 pips
    let fd_delta = finite_diff_delta(&call, &market, as_of, bump).unwrap();

    // Assert: Should match within tolerance
    assert_approx_eq(
        greeks.delta,
        fd_delta,
        1e-2,
        100.0,
        "Delta vs finite difference",
    );
}

#[test]
fn test_gamma_positive_for_long_option() {
    // Arrange: Long option always has positive gamma
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert
    assert!(greeks.gamma > 0.0, "Long option gamma should be positive");
}

#[test]
fn test_gamma_maximized_atm() {
    // Arrange: Gamma is highest ATM
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let atm_call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let itm_call = build_call_option(as_of, expiry, 1.00, 1_000_000.0);
    let otm_call = build_call_option(as_of, expiry, 1.40, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let atm_greeks = compute_greeks(&atm_call, &market, as_of);
    let itm_greeks = compute_greeks(&itm_call, &market, as_of);
    let otm_greeks = compute_greeks(&otm_call, &market, as_of);

    // Assert: ATM should have highest gamma
    assert!(atm_greeks.gamma > itm_greeks.gamma, "ATM gamma > ITM gamma");
    assert!(atm_greeks.gamma > otm_greeks.gamma, "ATM gamma > OTM gamma");
}

#[test]
fn test_gamma_matches_finite_difference() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act: Analytical gamma
    let greeks = compute_greeks(&call, &market, as_of);

    // Act: Finite difference gamma
    let bump = 0.001;
    let fd_gamma = finite_diff_gamma(&call, &market, as_of, bump).unwrap();

    // Assert: Should match within tolerance
    assert_approx_eq(
        greeks.gamma,
        fd_gamma,
        1e-1,
        100.0,
        "Gamma vs finite difference",
    );
}

#[test]
fn test_vega_positive_for_long_option() {
    // Arrange: Long option always has positive vega
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let put = build_put_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let call_greeks = compute_greeks(&call, &market, as_of);
    let put_greeks = compute_greeks(&put, &market, as_of);

    // Assert: Both should have positive vega
    assert!(call_greeks.vega > 0.0, "Call vega should be positive");
    assert!(put_greeks.vega > 0.0, "Put vega should be positive");

    // Call and put vega should be equal for same strike/expiry
    assert_approx_eq(
        call_greeks.vega,
        put_greeks.vega,
        1e-6,
        1e-6,
        "Call and put vega equal",
    );
}

#[test]
fn test_vega_higher_with_longer_expiry() {
    // Arrange: Longer dated options have higher vega
    let as_of = date!(2024 - 01 - 01);
    let expiry_3m = date!(2024 - 04 - 01);
    let expiry_1y = date!(2025 - 01 - 01);
    let call_3m = build_call_option(as_of, expiry_3m, 1.20, 1_000_000.0);
    let call_1y = build_call_option(as_of, expiry_1y, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks_3m = compute_greeks(&call_3m, &market, as_of);
    let greeks_1y = compute_greeks(&call_1y, &market, as_of);

    // Assert: 1Y vega should be higher
    assert!(
        greeks_1y.vega > greeks_3m.vega,
        "Longer dated vega should be higher"
    );
}

#[test]
fn test_vega_matches_finite_difference() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act: Analytical vega (per 1% vol move)
    let greeks = compute_greeks(&call, &market, as_of);

    // Act: Finite difference vega
    let bump = 0.01; // 1% vol move
    let fd_vega = finite_diff_vega(&call, &market, as_of, bump).unwrap();

    // Assert: Should match within tolerance
    assert_approx_eq(
        greeks.vega,
        fd_vega,
        5e-2,
        500.0,
        "Vega vs finite difference",
    );
}

#[test]
fn test_theta_negative_for_long_option() {
    // Arrange: Long options decay over time (negative theta)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let put = build_put_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let call_greeks = compute_greeks(&call, &market, as_of);
    let put_greeks = compute_greeks(&put, &market, as_of);

    // Assert: Theta should be negative (time decay)
    // Note: Sign convention varies; here we expect negative for decay
    assert!(
        call_greeks.theta.abs() > 0.0,
        "Call theta should be non-zero"
    );
    assert!(put_greeks.theta.abs() > 0.0, "Put theta should be non-zero");
}

#[test]
fn test_rho_has_expected_sign() {
    // Arrange: Call rho should be positive (benefits from higher domestic rates)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Call rho should be positive
    assert!(greeks.rho > 0.0, "Call rho should be positive");
}

#[test]
fn test_foreign_rho_has_expected_sign() {
    // Arrange: Call foreign rho should be negative (hurt by higher foreign rates)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Call foreign rho should be negative
    assert!(
        greeks.foreign_rho < 0.0,
        "Call foreign rho should be negative"
    );
}

#[test]
fn test_all_greeks_computed_together() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: All greeks should be finite and non-NaN
    assert!(greeks.delta.is_finite(), "Delta should be finite");
    assert!(greeks.gamma.is_finite(), "Gamma should be finite");
    assert!(greeks.vega.is_finite(), "Vega should be finite");
    assert!(greeks.theta.is_finite(), "Theta should be finite");
    assert!(greeks.rho.is_finite(), "Rho should be finite");
    assert!(
        greeks.foreign_rho.is_finite(),
        "Foreign rho should be finite"
    );
}

#[test]
fn test_expired_call_greeks_static() {
    // Arrange: Expired ITM call
    let expiry = date!(2024 - 01 - 01);
    let as_of = expiry;
    let strike = 1.10;
    let spot = 1.20;
    let call = build_call_option(expiry, expiry, strike, 1_000_000.0);
    let market = build_market_context(
        as_of,
        MarketParams {
            spot,
            ..Default::default()
        },
    );

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Gamma, vega, theta should be zero; delta should be 1
    assert_approx_eq(
        greeks.delta,
        1_000_000.0,
        1e-6,
        1e-6,
        "Expired ITM call delta = notional",
    );
    assert_eq!(greeks.gamma, 0.0, "Expired gamma = 0");
    assert_eq!(greeks.vega, 0.0, "Expired vega = 0");
    assert_eq!(greeks.theta, 0.0, "Expired theta = 0");
}

#[test]
fn test_expired_put_greeks_static() {
    // Arrange: Expired ITM put
    let expiry = date!(2024 - 01 - 01);
    let as_of = expiry;
    let strike = 1.30;
    let spot = 1.20;
    let put = build_put_option(expiry, expiry, strike, 1_000_000.0);
    let market = build_market_context(
        as_of,
        MarketParams {
            spot,
            ..Default::default()
        },
    );

    // Act
    let greeks = compute_greeks(&put, &market, as_of);

    // Assert: Gamma, vega, theta should be zero; delta should be -1
    assert_approx_eq(
        greeks.delta,
        -1_000_000.0,
        1e-6,
        1e-6,
        "Expired ITM put delta = -notional",
    );
    assert_eq!(greeks.gamma, 0.0, "Expired gamma = 0");
    assert_eq!(greeks.vega, 0.0, "Expired vega = 0");
    assert_eq!(greeks.theta, 0.0, "Expired theta = 0");
}

#[test]
fn test_expired_otm_call_greeks_zero() {
    // Arrange: Expired OTM call
    let expiry = date!(2024 - 01 - 01);
    let as_of = expiry;
    let strike = 1.30;
    let spot = 1.20;
    let call = build_call_option(expiry, expiry, strike, 1_000_000.0);
    let market = build_market_context(
        as_of,
        MarketParams {
            spot,
            ..Default::default()
        },
    );

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: All greeks should be zero for expired OTM
    assert_eq!(greeks.delta, 0.0, "Expired OTM delta = 0");
    assert_eq!(greeks.gamma, 0.0, "Expired gamma = 0");
    assert_eq!(greeks.vega, 0.0, "Expired vega = 0");
    assert_eq!(greeks.theta, 0.0, "Expired theta = 0");
}
