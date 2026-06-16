//! Tests for edge cases and boundary conditions.

use super::helpers::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

// ==================== EXPIRY TESTS ====================

#[test]
fn test_expired_itm_call_equals_intrinsic() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of; // Expired
    let strike = 90.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();
    let expected = (spot - strike) * call.notional.amount();

    assert_approx_eq_tol(
        pv.amount(),
        expected,
        TIGHT_TOL,
        "Expired ITM call intrinsic",
    );
}

#[test]
fn test_expired_otm_call_is_worthless() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of;
    let strike = 110.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();

    assert_approx_eq_tol(pv.amount(), 0.0, TIGHT_TOL, "Expired OTM call");
}

#[test]
fn test_expired_itm_put_equals_intrinsic() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of;
    let strike = 110.0;
    let spot = 100.0;

    let put = create_put(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = put.value(&market, as_of).unwrap();
    let expected = (strike - spot) * put.notional.amount();

    assert_approx_eq_tol(
        pv.amount(),
        expected,
        TIGHT_TOL,
        "Expired ITM put intrinsic",
    );
}

#[test]
fn test_expired_otm_put_is_worthless() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of;
    let strike = 90.0;
    let spot = 100.0;

    let put = create_put(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = put.value(&market, as_of).unwrap();

    assert_approx_eq_tol(pv.amount(), 0.0, TIGHT_TOL, "Expired OTM put");
}

#[test]
fn test_expired_option_greeks_are_static() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of;
    let strike = 90.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
        MetricId::Rho,
    ];

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();
    let gamma = *result.measures.get("gamma").unwrap();
    let vega = *result.measures.get("vega").unwrap();
    let theta = *result.measures.get("theta").unwrap();
    let rho = *result.measures.get("rho").unwrap();

    // Delta should be 1.0 * contract_size for ITM call
    assert_approx_eq_tol(delta, 100.0, TIGHT_TOL, "Expired ITM call delta");
    // All other Greeks should be zero
    assert_approx_eq_tol(gamma, 0.0, TIGHT_TOL, "Expired gamma");
    assert_approx_eq_tol(vega, 0.0, TIGHT_TOL, "Expired vega");
    assert_approx_eq_tol(theta, 0.0, TIGHT_TOL, "Expired theta");
    assert_approx_eq_tol(rho, 0.0, TIGHT_TOL, "Expired rho");
}

// ==================== EXTREME STRIKES ====================

#[test]
fn test_very_deep_itm_call() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 10.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();
    let intrinsic = (spot - strike) * call.notional.amount();

    // Very deep ITM should be close to discounted intrinsic
    assert!(
        pv.amount() > intrinsic * 0.9,
        "Deep ITM call >= 90% of intrinsic"
    );
}

#[test]
fn test_very_deep_otm_call() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 500.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();

    // Strike=500 is 5x spot=100: this is astronomically OTM, PV should be essentially 0
    // For $100 notional, BS price << $0.001 for 400% OTM call
    assert!(
        pv.amount() < 0.01,
        "Deep OTM call should be near worthless, got {}",
        pv.amount()
    );
}

// ==================== EXTREME VOLATILITY ====================

#[test]
fn test_very_high_volatility() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 2.0, 0.05, 0.0); // 200% vol

    let pv = call.value(&market, as_of).unwrap();

    // Very high vol should significantly increase value
    assert_positive(pv.amount(), "High vol call PV");
    // But should not exceed spot * contract_size
    assert!(
        pv.amount() < spot * call.notional.amount(),
        "Call cannot exceed spot * contract_size"
    );
}

// ==================== EXTREME RATES ====================

#[test]
fn test_negative_interest_rate() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, -0.02, 0.0);

    let pv = call.value(&market, as_of).unwrap();

    // Should still produce valid price
    assert_positive(pv.amount(), "Negative rate call PV");
}

#[test]
fn test_very_high_interest_rate() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.20, 0.0); // 20% rate

    let pv = call.value(&market, as_of).unwrap();

    // High rates increase call value
    assert_positive(pv.amount(), "High rate call PV");
}

// ==================== VERY SHORT/LONG MATURITY ====================

#[test]
fn test_very_short_maturity() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 01 - 08); // 1 week
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();

    // Short dated ATM should have small but positive value
    assert_positive(pv.amount(), "Short dated call PV");
    assert!(
        pv.amount() < 500.0,
        "Short dated call should have limited value"
    );
}

#[test]
fn test_very_long_maturity() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2029 - 01 - 01); // 5 years
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();

    // Long dated should have substantial value
    assert_positive(pv.amount(), "Long dated call PV");
    // Vega should be high for long dated
    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let vega = *result.measures.get("vega").unwrap();
    assert_positive(vega, "Long dated vega");
}

// ==================== SPOT EDGE CASES ====================

#[test]
fn test_very_low_spot_price() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 1.0; // Very low spot

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();

    // Deep OTM call should have minimal value
    assert_non_negative(pv.amount(), "Low spot call PV");
}

#[test]
fn test_very_high_spot_price() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 1000.0; // Very high spot

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();
    let intrinsic = (spot - strike) * call.notional.amount();

    // Deep ITM should be close to intrinsic
    assert!(
        pv.amount() > intrinsic * 0.95,
        "Deep ITM call should approach intrinsic"
    );
}

// ==================== HIGH DIVIDEND YIELD ====================

#[test]
fn test_high_dividend_yield() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.10); // 10% div yield

    let pv = call.value(&market, as_of).unwrap();

    // High dividend should reduce call value significantly
    assert_positive(pv.amount(), "High div call PV");

    // Compare to no dividend
    let no_div_market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);
    let no_div_pv = call.value(&no_div_market, as_of).unwrap();

    assert!(
        pv.amount() < no_div_pv.amount() * 0.8,
        "High dividend should significantly reduce call value"
    );
}
