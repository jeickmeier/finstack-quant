//! Tests for put-call parity relationships.
//!
//! Put-call parity for European options:
//! C - P = S*e^(-qT) - K*e^(-rT)
//!
//! Where:
//! - C = call value
//! - P = put value
//! - S = spot price
//! - K = strike
//! - r = risk-free rate
//! - q = dividend yield
//! - T = time to expiry
//!
//! # Tolerance Rationale
//!
//! Put-call parity is an analytical relationship that should hold exactly in theory.
//! In practice, numerical precision in Black-Scholes pricing introduces small errors.
//!
//! We use absolute tolerances scaled to position size (notional = spot × contract_size):
//! - Standard (1Y): ~0.1% of notional = $10 for 100 shares × $100 strike
//! - Short-dated (3M): Tighter tolerance since less discounting
//! - Long-dated (2Y+): Looser tolerance due to accumulated numerical drift

use super::helpers::*;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

/// Tolerance for put-call parity tests.
///
/// Put-call parity is an exact analytical relationship, so tolerance should be tight.
/// However, numerical precision in Black-Scholes implementation can cause small errors.
///
/// Tolerance scales with:
/// - Notional size (spot × contract_size)
/// - Time to expiry (longer = more numerical drift)
mod parity_tolerances {
    /// Standard 1Y option: 0.1% of notional (~$10 for 100 × $100)
    pub const STANDARD_1Y: f64 = 10.0;

    /// Short-dated (< 6M): Tighter tolerance
    pub const SHORT_DATED: f64 = 5.0;

    /// Long-dated (2Y+): Allow more drift
    pub const LONG_DATED: f64 = 20.0;

    /// High dividend scenarios: Slightly looser due to compounding effects
    pub const HIGH_DIVIDEND: f64 = 15.0;
}

#[test]
fn test_put_call_parity_atm() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let rate = 0.05;
    let div_yield = 0.02;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, rate, div_yield);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    // C - P = (S*e^(-qT) - K*e^(-rT)) * contract_size
    let t = 1.0_f64; // 1 year
    let forward_spot = spot * (-div_yield * t).exp();
    let pv_strike = strike * (-rate * t).exp();
    let expected_diff = (forward_spot - pv_strike) * call.notional.amount();

    let actual_diff = call_pv - put_pv;

    // Expected value derivation: C - P = (S×e^(-qT) - K×e^(-rT)) × contract_size
    // At ATM (S=K=100), t=1Y, r=5%, q=2%: expected ≈ (100×0.98 - 100×0.951) × 100 = $286
    assert_approx_eq_tol(
        actual_diff,
        expected_diff,
        parity_tolerances::STANDARD_1Y,
        "Put-call parity ATM",
    );
}

#[test]
fn test_put_call_parity_itm_call() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 90.0;
    let spot = 100.0;
    let rate = 0.05;
    let div_yield = 0.0;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, rate, div_yield);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    let t = 1.0_f64;
    let forward_spot = spot * (-div_yield * t).exp();
    let pv_strike = strike * (-rate * t).exp();
    let expected_diff = (forward_spot - pv_strike) * call.notional.amount();

    let actual_diff = call_pv - put_pv;

    // Expected value derivation: ITM call (K=90, S=100), q=0, r=5%, t=1Y
    // expected = (100 - 90×0.951) × 100 ≈ $1,441
    assert_approx_eq_tol(
        actual_diff,
        expected_diff,
        parity_tolerances::STANDARD_1Y,
        "Put-call parity ITM call",
    );
}

#[test]
fn test_put_call_parity_otm_call() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 110.0;
    let spot = 100.0;
    let rate = 0.05;
    let div_yield = 0.0;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, rate, div_yield);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    let t = 1.0_f64;
    let forward_spot = spot * (-div_yield * t).exp();
    let pv_strike = strike * (-rate * t).exp();
    let expected_diff = (forward_spot - pv_strike) * call.notional.amount();

    let actual_diff = call_pv - put_pv;

    // Expected value derivation: OTM call (K=110, S=100), q=0, r=5%, t=1Y
    // expected = (100 - 110×0.951) × 100 ≈ -$461 (negative since K > S adjusted)
    assert_approx_eq_tol(
        actual_diff,
        expected_diff,
        parity_tolerances::STANDARD_1Y,
        "Put-call parity OTM call",
    );
}

#[test]
fn test_put_call_parity_short_dated() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 04 - 01); // 3 months
    let strike = 100.0;
    let spot = 100.0;
    let rate = 0.05;
    let div_yield = 0.02;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, rate, div_yield);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    let t = 0.25; // 3 months
    let forward_spot = spot * (-div_yield * t).exp();
    let pv_strike = strike * (-rate * t).exp();
    let expected_diff = (forward_spot - pv_strike) * call.notional.amount();

    let actual_diff = call_pv - put_pv;

    // Expected value derivation: Short-dated (3M), less discounting effect
    assert_approx_eq_tol(
        actual_diff,
        expected_diff,
        parity_tolerances::SHORT_DATED,
        "Put-call parity short dated",
    );
}

#[test]
fn test_put_call_parity_long_dated() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2026 - 01 - 01); // 2 years
    let strike = 100.0;
    let spot = 100.0;
    let rate = 0.05;
    let div_yield = 0.02;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, rate, div_yield);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    let t = 2.0; // 2 years
    let forward_spot = spot * (-div_yield * t).exp();
    let pv_strike = strike * (-rate * t).exp();
    let expected_diff = (forward_spot - pv_strike) * call.notional.amount();

    let actual_diff = call_pv - put_pv;

    // Expected value derivation: Long-dated (2Y), more numerical drift in discounting
    assert_approx_eq_tol(
        actual_diff,
        expected_diff,
        parity_tolerances::LONG_DATED,
        "Put-call parity long dated",
    );
}

#[test]
fn test_put_call_parity_with_high_dividends() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let rate = 0.05;
    let div_yield = 0.08; // High dividend

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, rate, div_yield);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    let t = 1.0_f64;
    let forward_spot = spot * (-div_yield * t).exp();
    let pv_strike = strike * (-rate * t).exp();
    let expected_diff = (forward_spot - pv_strike) * call.notional.amount();

    let actual_diff = call_pv - put_pv;

    // Expected value derivation: High dividend (8%) reduces forward significantly
    assert_approx_eq_tol(
        actual_diff,
        expected_diff,
        parity_tolerances::HIGH_DIVIDEND,
        "Put-call parity with high dividends",
    );
}

#[test]
fn test_put_call_parity_zero_rates() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let rate = 0.0;
    let div_yield = 0.0;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, rate, div_yield);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    // With zero rates and dividends: C - P = (S - K) * contract_size
    let expected_diff = (spot - strike) * call.notional.amount();

    let actual_diff = call_pv - put_pv;

    // Expected value derivation: Zero rates → no discounting → exact parity
    assert_approx_eq_tol(
        actual_diff,
        expected_diff,
        parity_tolerances::SHORT_DATED, // Tighter since no discounting
        "Put-call parity zero rates",
    );
}

#[test]
fn test_put_call_parity_across_volatilities() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let rate = 0.05;
    let div_yield = 0.02;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let t = 1.0_f64;
    let forward_spot = spot * (-div_yield * t).exp();
    let pv_strike = strike * (-rate * t).exp();
    let expected_diff = (forward_spot - pv_strike) * call.notional.amount();

    // Test across different volatilities
    for vol in [0.10, 0.20, 0.30, 0.50, 0.80] {
        let market = build_standard_market(as_of, spot, vol, rate, div_yield);

        let call_pv = call.value(&market, as_of).unwrap().amount();
        let put_pv = put.value(&market, as_of).unwrap().amount();

        let actual_diff = call_pv - put_pv;

        // Parity should hold regardless of volatility level (it's an arbitrage relationship)
        assert_approx_eq_tol(
            actual_diff,
            expected_diff,
            parity_tolerances::STANDARD_1Y,
            &format!("Put-call parity at vol={}", vol),
        );
    }
}

#[test]
fn test_put_call_parity_across_spot_levels() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let rate = 0.05;
    let div_yield = 0.02;

    let call = create_call(as_of, expiry, strike);
    let put = create_put(as_of, expiry, strike);

    let t = 1.0;

    // Test across different spot levels
    for spot in [80.0, 90.0, 100.0, 110.0, 120.0] {
        let market = build_standard_market(as_of, spot, 0.25, rate, div_yield);

        let call_pv = call.value(&market, as_of).unwrap().amount();
        let put_pv = put.value(&market, as_of).unwrap().amount();

        let forward_spot = spot * (-div_yield * t).exp();
        let pv_strike = strike * (-rate * t).exp();
        let expected_diff = (forward_spot - pv_strike) * call.notional.amount();

        let actual_diff = call_pv - put_pv;

        // Parity should hold regardless of spot level (it's an arbitrage relationship)
        assert_approx_eq_tol(
            actual_diff,
            expected_diff,
            parity_tolerances::STANDARD_1Y,
            &format!("Put-call parity at spot={}", spot),
        );
    }
}
