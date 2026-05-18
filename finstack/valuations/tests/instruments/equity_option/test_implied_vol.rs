//! Tests for implied volatility calculation.

use super::helpers::*;
use finstack_core::dates::{DayCount, DayCountContext};
use finstack_core::math::norm_cdf;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::OptionType;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

fn bs_price(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return match option_type {
            OptionType::Call => (spot - strike).max(0.0),
            OptionType::Put => (strike - spot).max(0.0),
        };
    }
    let sqrt_t = t.sqrt();
    let d1 = ((spot / strike).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * sqrt_t);
    let d2 = d1 - sigma * sqrt_t;
    let disc_q = (-q * t).exp();
    let disc_r = (-r * t).exp();

    match option_type {
        OptionType::Call => spot * disc_q * norm_cdf(d1) - strike * disc_r * norm_cdf(d2),
        OptionType::Put => strike * disc_r * norm_cdf(-d2) - spot * disc_q * norm_cdf(-d1),
    }
}

#[allow(clippy::too_many_arguments)]
fn analytical_call_price(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    as_of: finstack_core::dates::Date,
    expiry: finstack_core::dates::Date,
    notional: f64,
) -> f64 {
    let t = DayCount::Act365F
        .year_fraction(as_of, expiry, DayCountContext::default())
        .unwrap_or(0.0);
    bs_price(spot, strike, rate, div_yield, vol, t, OptionType::Call) * notional
}
#[test]
fn test_implied_vol_recovers_surface_vol() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let vol = 0.30;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, 0.02);

    // Use analytical Black-Scholes price as external reference
    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.02,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );

    // Set market price in attributes
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    // Calculate implied vol
    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged (implied_vol > 0 indicates success)
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    assert_approx_eq_tol(
        implied_vol,
        vol,
        1e-4,
        "Implied vol should recover surface vol",
    );
}

#[test]
fn test_implied_vol_atm_option() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let vol = 0.25;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, 0.0);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.0,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    assert_approx_eq_tol(implied_vol, vol, 1e-5, "ATM implied vol");
}

#[test]
fn test_implied_vol_itm_option() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 90.0;
    let spot = 100.0;
    let vol = 0.28;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, 0.0);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.0,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    assert_approx_eq_tol(implied_vol, vol, 1e-5, "ITM implied vol");
}

#[test]
fn test_implied_vol_otm_option() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 110.0;
    let spot = 100.0;
    let vol = 0.32;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, 0.0);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.0,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    assert_approx_eq_tol(implied_vol, vol, 1e-5, "OTM implied vol");
}

#[test]
fn test_implied_vol_short_dated() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 02 - 01); // 1 month
    let strike = 100.0;
    let spot = 100.0;
    let vol = 0.40;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, 0.0);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.0,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    assert_approx_eq_tol(implied_vol, vol, 1e-5, "Short dated implied vol");
}

#[test]
fn test_implied_vol_long_dated() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2026 - 01 - 01); // 2 years
    let strike = 100.0;
    let spot = 100.0;
    let vol = 0.22;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, 0.0);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.0,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    assert_approx_eq_tol(implied_vol, vol, 1e-5, "Long dated implied vol");
}

#[test]
fn test_implied_vol_high_volatility() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let vol = 0.80; // High vol

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, 0.0);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.0,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    // Solver uses 1e-8 tolerance internally; 1e-4 is achievable for high vol
    assert_approx_eq_tol(implied_vol, vol, 1e-4, "High vol implied vol");
}

#[test]
fn test_implied_vol_low_volatility() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let vol = 0.10; // Low vol

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, 0.0);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.0,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    assert_approx_eq_tol(implied_vol, vol, 1e-5, "Low vol implied vol");
}

#[test]
fn test_implied_vol_returns_zero_for_expired() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of; // Already expired
    let strike = 100.0;
    let spot = 110.0;
    let vol = 0.25;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        0.0,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Expired option should return 0 implied vol
    assert_approx_eq_tol(implied_vol, 0.0, TIGHT_TOL, "Expired implied vol");
}

/// A malformed `market_price` attribute must surface as an error, not be
/// silently parsed to 0.0 (which would solve implied volatility against a zero
/// target price and return a meaningless degenerate IV).
#[test]
fn test_implied_vol_malformed_market_price_errors() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    // Not a number — previously `.parse().unwrap_or(0.0)` swallowed this.
    call.attributes
        .meta
        .insert("market_price".to_string(), "not-a-price".to_string());

    let result = call.price_with_metrics(
        &market,
        as_of,
        &[MetricId::ImpliedVol],
        finstack_valuations::instruments::PricingOptions::default(),
    );

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("market_price") && msg.contains("not a valid number"),
                "error must name the malformed market_price attribute; got: {msg}"
            );
        }
        Ok(r) => panic!(
            "a malformed market_price must error, not silently solve IV against 0.0 \
             (got implied_vol = {:?})",
            r.measures.get("implied_vol")
        ),
    }
}

#[test]
fn test_implied_vol_with_dividends() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 100.0;
    let spot = 100.0;
    let vol = 0.25;
    let div_yield = 0.03;

    let mut call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, vol, 0.05, div_yield);

    let market_price = analytical_call_price(
        spot,
        strike,
        0.05,
        div_yield,
        vol,
        as_of,
        expiry,
        call.notional.amount(),
    );
    call.attributes
        .meta
        .insert("market_price".to_string(), market_price.to_string());

    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let implied_vol = *result.measures.get("implied_vol").unwrap();

    // Assert solver converged
    assert!(
        implied_vol > 0.0,
        "Implied vol solver failed to converge (returned {})",
        implied_vol
    );
    assert_approx_eq_tol(implied_vol, vol, 1e-5, "Implied vol with dividends");
}
