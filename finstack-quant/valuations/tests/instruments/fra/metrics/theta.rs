//! FRA theta metric tests.
//!
//! Theta measures the time decay of the FRA value - how much the
//! PV changes as one day passes with all else held constant.

use crate::fra::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_theta_standard_fra() {
    let market = standard_market();
    let fra = create_standard_fra();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    assert_finite(theta, "Theta should be finite");
}

#[test]
fn test_theta_at_market_fra_near_zero() {
    // At-market FRA should have near-zero theta (no value, no decay)
    let market = standard_market();
    let fra = TestFraBuilder::new().fixed_rate(0.05).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    assert_near_zero(theta.abs(), 10.0, "At-market FRA theta should be near zero");
}

#[test]
fn test_theta_off_market_fra() {
    // Off-market FRA should have non-zero theta
    let market = standard_market();
    let fra = TestFraBuilder::new().fixed_rate(0.06).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    assert_finite(theta, "Off-market FRA should have finite theta");
}

#[test]
fn test_theta_scales_with_notional() {
    let market = standard_market();

    let fra_1m = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::USD)
        .fixed_rate(0.06)
        .build();

    let fra_10m = TestFraBuilder::new()
        .notional(10_000_000.0, Currency::USD)
        .fixed_rate(0.06)
        .build();

    let result_1m = fra_1m
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta_1m = *result_1m.measures.get("theta").unwrap();

    let result_10m = fra_10m
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta_10m = *result_10m.measures.get("theta").unwrap();

    assert_approx_equal(
        theta_10m,
        theta_1m * 10.0,
        5.0,
        "Theta should scale with notional",
    );
}

#[test]
fn test_theta_short_period() {
    let market = standard_market();

    let start = date!(2024 - 02 - 01);
    let end = date!(2024 - 03 - 01);
    let fra = TestFraBuilder::new()
        .dates(start, start, end)
        .fixed_rate(0.06)
        .build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    assert_finite(theta, "Short period theta should be finite");
}

#[test]
fn test_theta_long_period() {
    let market = standard_market();

    let start = date!(2024 - 07 - 01);
    let end = date!(2025 - 01 - 01);
    let fra = TestFraBuilder::new()
        .dates(start, start, end)
        .fixed_rate(0.06)
        .build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    assert_finite(theta, "Long period theta should be finite");
}

#[test]
fn test_theta_with_other_metrics() {
    let market = standard_market();
    let fra = TestFraBuilder::new().fixed_rate(0.06).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Theta, MetricId::Dv01, MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(result.measures.contains_key("theta"));
    assert!(result.measures.contains_key("dv01"));
    assert!(result.measures.contains_key("par_rate"));
}
