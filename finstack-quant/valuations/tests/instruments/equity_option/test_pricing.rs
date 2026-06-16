//! Tests for Black-Scholes pricing implementation.

use super::helpers::*;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

#[test]
fn test_atm_call_has_positive_value() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.02);

    let pv = call.value(&market, as_of).unwrap();

    assert_positive(pv.amount(), "ATM call PV");
    // ATM call with 1Y expiry, 25% vol, 5% rate, 100 contract size
    // Should be roughly 10-15 per share * 100 = 1000-1500 total
    assert_in_range(pv.amount(), 500.0, 2000.0, "ATM call PV range");
}

#[test]
fn test_atm_put_has_positive_value() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let put = create_put(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.02);

    let pv = put.value(&market, as_of).unwrap();

    assert_positive(pv.amount(), "ATM put PV");
    assert_in_range(pv.amount(), 500.0, 2000.0, "ATM put PV range");
}

#[test]
fn test_itm_call_worth_more_than_atm() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 110.0;

    let itm_call = create_call(as_of, expiry, 100.0);
    let atm_call = create_call(as_of, expiry, 110.0);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.02);

    let itm_pv = itm_call.value(&market, as_of).unwrap();
    let atm_pv = atm_call.value(&market, as_of).unwrap();

    assert!(
        itm_pv.amount() > atm_pv.amount(),
        "ITM call ({}) should be worth more than ATM call ({})",
        itm_pv.amount(),
        atm_pv.amount()
    );
}

#[test]
fn test_otm_call_worth_less_than_atm() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    let otm_call = create_call(as_of, expiry, 120.0);
    let atm_call = create_call(as_of, expiry, 100.0);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.02);

    let otm_pv = otm_call.value(&market, as_of).unwrap();
    let atm_pv = atm_call.value(&market, as_of).unwrap();

    assert!(
        otm_pv.amount() < atm_pv.amount(),
        "OTM call ({}) should be worth less than ATM call ({})",
        otm_pv.amount(),
        atm_pv.amount()
    );
}

#[test]
fn test_longer_maturity_increases_value() {
    let as_of = date!(2024 - 01 - 01);
    let short_expiry = date!(2024 - 07 - 01); // 6M
    let long_expiry = date!(2025 - 01 - 01); // 1Y
    let strike = 100.0;
    let spot = 100.0;

    let short_call = create_call(as_of, short_expiry, strike);
    let long_call = create_call(as_of, long_expiry, strike);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.02);

    let short_pv = short_call.value(&market, as_of).unwrap();
    let long_pv = long_call.value(&market, as_of).unwrap();

    assert!(
        long_pv.amount() > short_pv.amount(),
        "Longer maturity ({}) should have higher value than shorter ({})",
        long_pv.amount(),
        short_pv.amount()
    );
}

#[test]
fn test_higher_volatility_increases_value() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);

    let low_vol_market = build_standard_market(as_of, spot, 0.15, 0.05, 0.02);
    let high_vol_market = build_standard_market(as_of, spot, 0.35, 0.05, 0.02);

    let low_vol_pv = call.value(&low_vol_market, as_of).unwrap();
    let high_vol_pv = call.value(&high_vol_market, as_of).unwrap();

    assert!(
        high_vol_pv.amount() > low_vol_pv.amount(),
        "Higher vol ({}) should increase value vs lower vol ({})",
        high_vol_pv.amount(),
        low_vol_pv.amount()
    );
}

#[test]
fn test_contract_size_scales_value_linearly() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let mut call_100 = create_call(as_of, expiry, strike);
    call_100.notional = finstack_quant_core::money::Money::new(100.0, call_100.notional.currency());

    let mut call_200 = create_call(as_of, expiry, strike);
    call_200.notional = finstack_quant_core::money::Money::new(200.0, call_200.notional.currency());

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.02);

    let pv_100 = call_100.value(&market, as_of).unwrap();
    let pv_200 = call_200.value(&market, as_of).unwrap();

    assert_approx_eq_tol(
        pv_200.amount(),
        pv_100.amount() * 2.0,
        0.1, // Allow 10 cent tolerance for rounding
        "Double contract size should double value",
    );
}

#[test]
fn test_pricing_with_dividend_yield() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);

    let no_div_market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);
    let with_div_market = build_standard_market(as_of, spot, 0.25, 0.05, 0.03);

    let no_div_pv = call.value(&no_div_market, as_of).unwrap();
    let with_div_pv = call.value(&with_div_market, as_of).unwrap();

    // Dividend yield reduces call value
    assert!(
        with_div_pv.amount() < no_div_pv.amount(),
        "Dividend yield should reduce call value: {} < {}",
        with_div_pv.amount(),
        no_div_pv.amount()
    );
}

#[test]
fn test_pricing_with_interest_rates() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);

    let low_rate_market = build_standard_market(as_of, spot, 0.25, 0.01, 0.0);
    let high_rate_market = build_standard_market(as_of, spot, 0.25, 0.10, 0.0);

    let low_rate_pv = call.value(&low_rate_market, as_of).unwrap();
    let high_rate_pv = call.value(&high_rate_market, as_of).unwrap();

    // Higher rates increase call value (forward effect)
    assert!(
        high_rate_pv.amount() > low_rate_pv.amount(),
        "Higher rates should increase call value: {} > {}",
        high_rate_pv.amount(),
        low_rate_pv.amount()
    );
}

#[test]
fn test_deep_itm_call_approaches_intrinsic() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 02 - 01); // Short dated
    let strike = 50.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();
    let intrinsic = (spot - strike) * call.notional.amount();

    // Deep ITM short-dated should be close to intrinsic
    assert!(
        pv.amount() > intrinsic,
        "Deep ITM call PV ({}) should exceed intrinsic ({})",
        pv.amount(),
        intrinsic
    );
    // But not by too much for short dated
    assert!(
        pv.amount() < intrinsic * 1.2,
        "Deep ITM short dated call PV ({}) should be within 20% of intrinsic ({})",
        pv.amount(),
        intrinsic
    );
}

#[test]
fn test_deep_otm_call_has_small_value() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 200.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap();

    // Deep OTM should have very small value
    assert!(
        pv.amount() < 100.0,
        "Deep OTM call should have small value, got {}",
        pv.amount()
    );
}
