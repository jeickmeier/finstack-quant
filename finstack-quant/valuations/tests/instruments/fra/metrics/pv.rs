//! FRA PV tests.
//!
//! The PV is returned in ValuationResult.value (not in measures).
//! These tests verify it's consistent with direct value() calls.

use crate::fra::common::*;
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_pv_result_matches_value() {
    let market = standard_market();
    let fra = TestFraBuilder::new().fixed_rate(0.06).build();

    let direct_pv = fra.value(&market, BASE_DATE).unwrap();
    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let result_pv = result.value.amount();

    assert_approx_equal(
        result_pv,
        direct_pv.amount(),
        0.01,
        "Result.value should match direct value()",
    );
}

#[test]
fn test_pv_at_market() {
    let market = standard_market();
    let fra = create_standard_fra();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv = result.value.amount();

    assert_near_zero(pv, 1000.0, "At-market FRA PV should be near zero");
}

#[test]
fn test_pv_off_market_positive() {
    let market = standard_market();
    let fra = TestFraBuilder::new()
        .fixed_rate(0.06) // above market
        .receive_fixed(true) // true = receive fixed
        .build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv = result.value.amount();

    assert_positive(pv, "Above-market receive-fixed should have positive PV");
}

#[test]
fn test_pv_off_market_negative() {
    let market = standard_market();
    let fra = TestFraBuilder::new()
        .fixed_rate(0.04) // below market
        .receive_fixed(true) // true = receive fixed
        .build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv = result.value.amount();

    assert_negative(pv, "Below-market receive-fixed should have negative PV");
}

#[test]
fn test_pv_with_other_metrics() {
    let market = standard_market();
    let fra = TestFraBuilder::new().fixed_rate(0.06).build();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // PV is in result.value, not in measures
    assert!(result.value.amount().is_finite());
    assert!(result.measures.contains_key("dv01"));
}
