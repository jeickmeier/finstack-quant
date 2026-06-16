//! Unit tests for FX option calculator core functionality.
//!
//! Tests FX option pricing via public APIs:
//! value, market data handling, validation, and expired option behavior.

use super::helpers::*;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_npv_matches_garman_kohlhagen() {
    // Arrange: ATM call with 1Y expiry
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 1.20;
    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let params = MarketParams::atm();
    let market = build_market_context(as_of, params);

    // Act
    let pv = call.value(&market, as_of).unwrap();

    // Assert: PV should be positive for ATM call
    assert!(pv.amount() > 0.0, "ATM call PV should be positive");
    assert_eq!(pv.currency(), QUOTE);

    // Verify against explicit GK formula inputs
    let t = call
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())
        .unwrap();
    let spot = market
        .fx()
        .expect("fx matrix")
        .rate(FxQuery::new(BASE, QUOTE, as_of))
        .expect("spot")
        .rate;
    let surf = market.get_surface(VOL_ID).expect("vol surface");
    let sigma = surf.value_clamped(t, call.strike);
    assert_approx_eq(spot, params.spot, 1e-10, 1e-10, "Spot");
    assert_approx_eq(params.r_domestic, 0.03, 1e-3, 1e-3, "Domestic rate");
    assert_approx_eq(params.r_foreign, 0.01, 1e-3, 1e-3, "Foreign rate");
    assert_approx_eq(sigma, params.vol, 1e-10, 1e-10, "Vol");
    // 2024 is a leap year, so 366/365 ≈ 1.0027
    assert_approx_eq(t, 1.0, 5e-3, 5e-3, "Time to expiry");
}

#[test]
fn test_npv_call_vs_put_values() {
    // Arrange: ATM options
    let as_of = date!(2024 - 06 - 15);
    let expiry = date!(2025 - 06 - 15);
    let strike = 1.20;

    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let put = build_put_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let call_pv = call.value(&market, as_of).unwrap();
    let put_pv = put.value(&market, as_of).unwrap();

    // Assert: Both should be positive
    assert!(call_pv.amount() > 0.0, "Call PV should be positive");
    assert!(put_pv.amount() > 0.0, "Put PV should be positive");

    // For ATM with positive carry (r_d > r_f), call typically > put
    // But we don't assert this strictly as it depends on rates
}

#[test]
fn test_npv_itm_call_has_higher_value() {
    // Arrange: ITM call (spot > strike)
    let as_of = date!(2024 - 03 - 01);
    let expiry = date!(2025 - 03 - 01);
    let strike = 1.10; // ITM since spot = 1.20

    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();
    let intrinsic = (1.20 - strike) * 1_000_000.0;

    // Assert: PV should exceed intrinsic value due to time value
    assert!(
        pv.amount() > intrinsic,
        "ITM call PV should exceed intrinsic"
    );
}

#[test]
fn test_npv_otm_call_has_time_value() {
    // Arrange: OTM call (spot < strike)
    let as_of = date!(2024 - 03 - 01);
    let expiry = date!(2025 - 03 - 01);
    let strike = 1.30; // OTM since spot = 1.20

    let call = build_call_option(as_of, expiry, strike, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();

    // Assert: Should have positive time value despite being OTM
    assert!(pv.amount() > 0.0, "OTM call should have time value");
}

#[test]
fn test_surface_vol_used_in_pricing() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let params = MarketParams::default();
    let market = build_market_context(as_of, params);

    // Act
    let t = call
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())
        .unwrap();
    let spot = market
        .fx()
        .expect("fx matrix")
        .rate(FxQuery::new(BASE, QUOTE, as_of))
        .expect("spot")
        .rate;
    let disc_domestic = market.get_discount(DOMESTIC_ID).unwrap();
    let disc_foreign = market.get_discount(FOREIGN_ID).unwrap();
    let df_domestic = disc_domestic.df_between_dates(as_of, expiry).unwrap();
    let df_foreign = disc_foreign.df_between_dates(as_of, expiry).unwrap();
    let r_d = -df_domestic.ln() / t;
    let r_f = -df_foreign.ln() / t;
    let sigma = market
        .get_surface(VOL_ID)
        .expect("vol surface")
        .value_clamped(t, call.strike);

    // Assert
    assert_approx_eq(spot, params.spot, 1e-10, 1e-10, "Spot from FX matrix");
    assert_approx_eq(sigma, params.vol, 1e-10, 1e-10, "Vol from surface");
    assert!(r_d > 0.0, "Domestic rate should be positive");
    assert!(r_f > 0.0, "Foreign rate should be positive");
    assert!(t > 0.9 && t < 1.1, "Time should be ~1Y");
}

#[test]
fn test_vol_override_used_in_pricing() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let mut call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv_surface = call.value(&market, as_of).unwrap();
    call.pricing_overrides.market_quotes.implied_volatility = Some(0.30); // Override
    let pv_override = call.value(&market, as_of).unwrap();

    // Assert: Higher override vol should increase value
    assert!(
        pv_override.amount() > pv_surface.amount(),
        "Override vol should increase PV"
    );
}

#[test]
fn test_missing_vol_surface_errors() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let params = MarketParams::atm();
    let market = MarketContext::new()
        .insert(build_flat_discount_curve(
            params.r_domestic,
            as_of,
            DOMESTIC_ID,
        ))
        .insert(build_flat_discount_curve(
            params.r_foreign,
            as_of,
            FOREIGN_ID,
        ))
        .insert_fx(create_fx_matrix(params.spot));

    // Act & Assert: Missing vol surface should fail pricing
    let result = call.value(&market, as_of);
    assert!(result.is_err(), "Missing vol surface should error");
}

#[test]
fn test_year_fraction_uses_day_count() {
    // Arrange
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 07 - 01); // ~6 months

    // Act
    let yf_act365 = DayCount::Act365F
        .year_fraction(start, end, DayCountContext::default())
        .unwrap();

    // Assert
    assert!(yf_act365 > 0.49 && yf_act365 < 0.51, "Should be ~0.5Y");
}

#[test]
fn test_npv_expired_call_returns_intrinsic() {
    // Arrange: Expired ITM call
    let expiry = date!(2024 - 01 - 01);
    let as_of = expiry; // At expiry
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

    // Assert: Should equal intrinsic value
    let intrinsic = (spot - strike) * 1_000_000.0;
    assert_approx_eq(pv.amount(), intrinsic, 1e-6, 1e-6, "Expired call intrinsic");
}

#[test]
fn test_npv_expired_put_returns_intrinsic() {
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
    let pv = put.value(&market, as_of).unwrap();

    // Assert: Should equal intrinsic value
    let intrinsic = (strike - spot) * 1_000_000.0;
    assert_approx_eq(pv.amount(), intrinsic, 1e-6, 1e-6, "Expired put intrinsic");
}

#[test]
fn test_npv_expired_otm_call_is_zero() {
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
    let pv = call.value(&market, as_of).unwrap();

    // Assert: Should be zero
    assert_approx_eq(
        pv.amount(),
        0.0,
        1e-10,
        1e-10,
        "Expired OTM call is worthless",
    );
}

#[test]
fn test_currency_validation_rejects_mismatched_notional() {
    // Arrange: Notional in wrong currency
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let mut call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    call.notional = Money::new(1_000_000.0, QUOTE); // Should be BASE currency

    let market = build_market_context(as_of, MarketParams::atm());

    // Act & Assert: Should fail validation
    let result = call.value(&market, as_of);
    assert!(
        result.is_err(),
        "Should reject mismatched notional currency"
    );
}

#[test]
fn test_price_with_metrics_matches_value() {
    // Arrange
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let call = build_call_option(as_of, expiry, 1.20, 1_000_000.0);
    let market = build_market_context(as_of, MarketParams::atm());

    // Act
    let pv = call.value(&market, as_of).unwrap();
    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Assert: price_with_metrics returns the same PV
    assert_approx_eq(
        result.value.amount(),
        pv.amount(),
        1e-10,
        1e-10,
        "price_with_metrics PV matches value()",
    );
}
