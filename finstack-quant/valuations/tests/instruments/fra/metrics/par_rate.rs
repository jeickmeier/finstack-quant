//! FRA par rate metric tests.
//!
//! Par rate is the fixed rate that makes the FRA's PV zero under
//! current market curves. For standard FRAs, this equals the forward
//! rate over the period.

use crate::fra::common::*;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::prelude::ToPrimitive;
use time::macros::date;

#[test]
fn test_par_rate_matches_forward_curve() {
    let market = standard_market(); // 5% flat
    let fra = create_standard_fra();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    assert_approx_equal(
        par_rate,
        0.05,
        0.001,
        "Par rate should match flat forward curve",
    );
}

#[test]
fn test_par_rate_at_market_fra_is_fixed_rate() {
    // For at-market FRA, par rate should equal the fixed rate
    let market = standard_market(); // 5% flat
    let fra = TestFraBuilder::new().fixed_rate(0.05).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    assert_approx_equal(
        par_rate,
        fra.fixed_rate.to_f64().unwrap_or_default(),
        0.001,
        "At-market FRA par rate should equal fixed rate",
    );
}

#[test]
fn test_par_rate_independent_of_fixed_rate() {
    // Par rate depends only on curves, not the FRA's fixed rate
    let market = standard_market();

    let fra_4pct = TestFraBuilder::new().fixed_rate(0.04).build();
    let fra_6pct = TestFraBuilder::new().fixed_rate(0.06).build();

    let result_4 = fra_4pct
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_4 = *result_4.measures.get("par_rate").unwrap();

    let result_6 = fra_6pct
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_6 = *result_6.measures.get("par_rate").unwrap();

    assert_approx_equal(
        par_4,
        par_6,
        0.0001,
        "Par rate should be independent of fixed rate",
    );
}

#[test]
fn test_par_rate_upward_sloping_curve() {
    let disc = build_flat_discount_curve(0.05, BASE_DATE, "USD_OIS");
    let fwd = build_upward_forward_curve(BASE_DATE, "USD_LIBOR_3M");

    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = create_standard_fra();
    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    assert_finite(par_rate, "Par rate should be finite");
    assert_in_range(
        par_rate,
        0.03,
        0.06,
        "Par rate should be in reasonable range for upward curve",
    );
}

#[test]
fn test_par_rate_inverted_curve() {
    let disc = build_flat_discount_curve(0.05, BASE_DATE, "USD_OIS");
    let fwd = build_inverted_forward_curve(BASE_DATE, "USD_LIBOR_3M");

    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = create_standard_fra();
    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    assert_finite(par_rate, "Par rate should be finite for inverted curve");
}

#[test]
fn test_par_rate_short_period() {
    let market = standard_market();

    let start = date!(2024 - 02 - 01);
    let end = date!(2024 - 03 - 01);
    let fra = TestFraBuilder::new().dates(start, start, end).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    assert_finite(par_rate, "Par rate should be finite for short period");
}

#[test]
fn test_par_rate_long_period() {
    let market = standard_market();

    let start = date!(2024 - 07 - 01);
    let end = date!(2025 - 01 - 01);
    let fra = TestFraBuilder::new().dates(start, start, end).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    assert_finite(par_rate, "Par rate should be finite for long period");
}

#[test]
fn test_par_rate_zero_tau_returns_error() {
    let market = standard_market();
    let same_date = date!(2024 - 04 - 01);

    let fra = TestFraBuilder::new()
        .dates(same_date, same_date, same_date)
        .build();

    // Zero-length period should now return an error (not 0.0)
    // because a zero-length FRA has undefined par rate
    let result = fra.price_with_metrics(
        &market,
        BASE_DATE,
        &[MetricId::ParRate],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_err(),
        "Zero-length FRA period should return error for par rate"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("period length is zero"),
        "Error should mention zero period length: {}",
        err_msg
    );
}

#[test]
fn test_par_rate_different_day_counts() {
    let market = standard_market();

    let fra_360 = TestFraBuilder::new().day_count(DayCount::Act360).build();
    let fra_365 = TestFraBuilder::new().day_count(DayCount::Act365F).build();

    let result_360 = fra_360
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_360 = *result_360.measures.get("par_rate").unwrap();

    let result_365 = fra_365
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_365 = *result_365.measures.get("par_rate").unwrap();

    // Both should be close for flat curve (day count affects tau, not par rate directly)
    assert_approx_equal(
        par_360,
        par_365,
        0.001,
        "Par rates should be similar for different day counts",
    );
}

#[test]
fn test_par_rate_negative_rate_environment() {
    let disc = build_flat_discount_curve(-0.01, BASE_DATE, "USD_OIS");
    let fwd = build_flat_forward_curve(-0.01, BASE_DATE, "USD_LIBOR_3M");
    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = create_standard_fra();
    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_rate = *result.measures.get("par_rate").unwrap();

    assert_approx_equal(
        par_rate,
        -0.01,
        0.0005,
        "Par rate should match negative forward rate",
    );
}
