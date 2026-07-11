//! Edge case and boundary condition tests.
//!
//! Tests extreme scenarios, boundary values, and error conditions
//! to ensure robustness.

use super::helpers::*;
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::prelude::Instrument;
use time::macros::date;

#[test]
fn test_zero_volatility_call_becomes_forward() {
    // Arrange: Zero vol means option becomes forward contract
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 1.20;
    let spot = 1.20;

    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let params = MarketParams {
        spot,
        vol: 1e-10,
        ..Default::default()
    };
    let market = build_market_context(as_of, params);

    // Act
    let pv = call.value(&market, as_of).unwrap();

    // Assert: At zero vol, ATM option ~ forward value
    // Forward = S * exp(-r_f * T) - K * exp(-r_d * T)
    let t = call
        .day_count
        .year_fraction(as_of, call.expiry, DayCountContext::default())
        .unwrap();
    let forward_value = 1_000_000.0
        * (spot * (-params.r_foreign * t).exp() - strike * (-params.r_domestic * t).exp());

    // At zero vol, option converges to max(forward, 0)
    assert_approx_eq(
        pv.amount(),
        forward_value.max(0.0),
        1e-2,
        100.0,
        "Zero vol option",
    );
}

#[test]
fn test_very_high_volatility() {
    // Arrange: Extremely high vol (100%)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(
        as_of,
        MarketParams {
            vol: 1.0,
            ..Default::default()
        },
    );
    // Act
    let pv = call.value(&market, as_of).unwrap();
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Should produce finite results
    assert!(
        pv.amount().is_finite() && pv.amount() > 0.0,
        "High vol PV should be finite and positive"
    );
    assert!(greeks.delta.is_finite(), "High vol delta should be finite");
    assert!(
        greeks.gamma.is_finite() && greeks.gamma >= 0.0,
        "High vol gamma should be finite and non-negative"
    );
    assert!(
        greeks.vega.is_finite() && greeks.vega > 0.0,
        "High vol vega should be finite and positive"
    );
}

#[test]
fn test_deep_itm_call_behaves_like_forward() {
    // Arrange: Very deep ITM call (strike << spot)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 0.50; // Very deep ITM (spot = 1.20)

    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Delta should be very close to 1 (scaled by notional)
    assert!(
        greeks.delta > 950_000.0,
        "Deep ITM delta should approach notional"
    );

    // Gamma and vega should be significantly reduced for deep ITM (approaches forward behavior)
    // Deep ITM gamma is typically < 0.05 per 1M notional (vs ~0.5+ for ATM)
    assert!(
        greeks.gamma < 0.1,
        "Deep ITM gamma should be substantially reduced (forward-like behavior), got {}",
        greeks.gamma
    );
    // Vega for deep ITM is typically < 1.0 per 1M notional (vs ~100+ for ATM)
    assert!(
        greeks.vega < 5.0,
        "Deep ITM vega should be substantially reduced (forward-like behavior), got {}",
        greeks.vega
    );
}

#[test]
fn test_deep_otm_call_has_minimal_value() {
    // Arrange: Very deep OTM call (strike >> spot)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 2.00; // Very deep OTM (spot = 1.20)

    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Value should be very small (strike=2.00 is 67% above spot=1.20)
    // For $1M notional, deep OTM 1Y call should be worth < $100
    assert!(
        pv.amount() < 100.0,
        "Deep OTM value should be near zero: {}",
        pv.amount()
    );
    assert!(pv.amount() >= 0.0, "Value should be non-negative");

    // Delta should be near zero (deep OTM call has near-zero probability of exercise)
    // For $1M notional call 67% OTM, delta << 1% of notional
    assert!(
        greeks.delta < 10_000.0,
        "Deep OTM delta should be near zero: {}",
        greeks.delta
    );
    assert!(greeks.delta >= 0.0, "Call delta should be non-negative");
}

#[test]
fn test_very_short_dated_option() {
    // Arrange: 1 day to expiry
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 01 - 02);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Should produce valid results
    assert!(
        pv.amount() > 0.0 && pv.amount().is_finite(),
        "Short dated PV should be positive and finite"
    );
    assert!(
        greeks.gamma >= 0.0 && greeks.gamma.is_finite(),
        "Short dated gamma should be finite"
    );

    // Theta should be large in absolute value (rapid decay)
    assert!(
        greeks.theta.abs() > 0.0,
        "Short dated theta should be non-zero"
    );
}

#[test]
fn test_at_expiry_boundary() {
    // Arrange: Valuing exactly at expiry
    let expiry = date!(2024 - 01 - 01);
    let as_of = expiry;
    let strike = 1.20;
    let spot = 1.25;

    let call = build_call_option(expiry, expiry, strike, 1_000_000.0);
    let market = build_market_context(
        as_of,
        MarketParams {
            spot,
            ..Default::default()
        },
    );

    // Act
    let pv = call.value(&market, as_of).unwrap();

    // Assert: Should equal intrinsic
    let intrinsic = (spot - strike).max(0.0) * 1_000_000.0;
    assert_approx_eq(
        pv.amount(),
        intrinsic,
        1e-6,
        1e-6,
        "At expiry value = intrinsic",
    );
}

#[test]
fn test_past_expiry_is_zero() {
    // Arrange: Valuing after expiry
    let expiry = date!(2024 - 01 - 01);
    let as_of = date!(2024 - 01 - 15); // 2 weeks past expiry
    let strike = 1.20;
    let spot = 1.30;

    let call = build_call_option(expiry, expiry, strike, 1_000_000.0);
    let market = build_market_context(
        as_of,
        MarketParams {
            spot,
            ..Default::default()
        },
    );

    // Act
    let pv = call.value(&market, as_of).unwrap();

    assert_eq!(pv.amount(), 0.0);
}

#[test]
fn test_very_small_notional() {
    // Arrange: Tiny notional (1 unit)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Should scale correctly
    assert!(
        pv.amount() > 0.0 && pv.amount() < 1.0,
        "Small notional PV should be scaled"
    );
    assert!(
        greeks.delta.is_finite() && greeks.delta.abs() < 10.0,
        "Small notional delta scaled"
    );
}

#[test]
fn test_very_large_notional() {
    // Arrange: Huge notional (1 billion)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();
    let greeks = compute_greeks(&call, &market, as_of);

    // Assert: Should scale linearly without overflow
    assert!(
        pv.amount() > 1_000_000.0 && pv.amount().is_finite(),
        "Large notional PV should be scaled and finite"
    );
    assert!(
        greeks.delta.is_finite(),
        "Large notional delta should be finite"
    );
}

#[test]
fn test_strike_equals_spot_exactly() {
    // Arrange: Exactly ATM
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 1.20;
    let spot = 1.20;

    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let put = build_put_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(
        as_of,
        MarketParams {
            spot,
            ..Default::default()
        },
    );

    // Act
    let call_pv = call.value(&market, as_of).unwrap();
    let put_pv = put.value(&market, as_of).unwrap();
    let call_greeks = compute_greeks(&call, &market, as_of);
    let put_greeks = compute_greeks(&put, &market, as_of);

    // Assert: Both should have positive value
    assert!(call_pv.amount() > 0.0, "Exactly ATM call has time value");
    assert!(put_pv.amount() > 0.0, "Exactly ATM put has time value");

    // Vega should be equal
    assert_approx_eq(
        call_greeks.vega,
        put_greeks.vega,
        1e-6,
        1e-6,
        "ATM vega equal for call/put",
    );
}

#[test]
fn test_negative_time_to_expiry_is_zero() {
    // Arrange: as_of after expiry
    let expiry = date!(2024 - 01 - 01);
    let as_of = date!(2024 - 06 - 01);
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
    let pv = call.value(&market, as_of).unwrap();
    let greeks = compute_greeks(&call, &market, as_of);

    assert_eq!(pv.amount(), 0.0);
    assert_eq!(greeks.gamma, 0.0, "Expired gamma = 0");
}

#[test]
fn test_spot_zero_edge_case() {
    // Arrange: Spot = 0 (theoretical edge case)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(
        as_of,
        MarketParams {
            spot: 1e-10,
            ..Default::default()
        },
    );

    // Act
    let pv = call.value(&market, as_of).unwrap();

    // Assert: Call should be worthless if spot ~ 0
    assert!(
        pv.amount() < 1.0,
        "Call at spot=0 should be nearly worthless"
    );
}

#[test]
fn test_strike_zero_edge_case() {
    // Arrange: Strike = 0 (call is equivalent to forward)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 0.0, 1_000_000.0);
    let params = MarketParams::atm();
    let market = build_market_context(as_of, params);

    // Act
    let pv = call.value(&market, as_of).unwrap();

    // Assert: Call with K=0 is worth discounted forward spot
    let t = call
        .day_count
        .year_fraction(as_of, call.expiry, DayCountContext::default())
        .unwrap();
    let expected = 1_000_000.0 * params.spot * (-params.r_foreign * t).exp();
    assert_approx_eq(
        pv.amount(),
        expected,
        1e-2,
        100.0,
        "Call with K=0 is forward",
    );
}

#[test]
fn test_put_type_with_various_moneyness() {
    // Arrange: Test puts across moneyness spectrum
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 1.20;

    // Deep ITM put
    let put_itm = build_put_option(as_of, expiry, 1.40, 1_000_000.0);
    // ATM put
    let put_atm = build_put_option(as_of, expiry, 1.20, 1_000_000.0);
    // OTM put
    let put_otm = build_put_option(as_of, expiry, 1.00, 1_000_000.0);

    let market = build_market_context(
        as_of,
        MarketParams {
            spot,
            ..Default::default()
        },
    );

    // Act
    let pv_itm = put_itm.value(&market, as_of).unwrap();
    let pv_atm = put_atm.value(&market, as_of).unwrap();
    let pv_otm = put_otm.value(&market, as_of).unwrap();

    // Assert: ITM > ATM > OTM
    assert!(pv_itm.amount() > pv_atm.amount(), "ITM put > ATM put");
    assert!(pv_atm.amount() > pv_otm.amount(), "ATM put > OTM put");
    assert!(pv_otm.amount() > 0.0, "OTM put has time value");
}

#[test]
fn test_currency_mismatch_detected() {
    // Tested in test_calculator.rs already, but worth having comprehensive coverage
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let mut call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    call.notional = Money::new(1_000_000.0, QUOTE); // Wrong currency

    let market = build_market_context(as_of, MarketParams::atm());

    // Act & Assert
    let result = call.value(&market, as_of);
    assert!(result.is_err(), "Currency mismatch should error");
}
