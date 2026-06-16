//! Metric calculator integration tests for convertible bonds.
//!
//! Tests the metric framework integration:
//! - Parity metric calculator
//! - Conversion premium metric calculator
//! - Greeks metric calculators
//! - Metric registry integration
//! - price_with_metrics interface

use super::fixtures::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_parity_metric() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::custom("parity")],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_ok(),
        "Parity metric should calculate successfully"
    );

    let valuation = result.unwrap();
    assert!(
        valuation.measures.contains_key("parity"),
        "Should contain parity metric"
    );

    let parity = valuation.measures.get("parity").unwrap();

    // Parity should be around 1.5 for ITM scenario (150 * 10 / 1000)
    let expected_parity = theoretical_parity(
        market_params::SPOT_PRICE,
        bond_params::CONVERSION_RATIO,
        bond_params::NOTIONAL,
    );

    assert!(
        (*parity - expected_parity).abs() < TOLERANCE,
        "Parity should be correct: got {}, expected {}",
        parity,
        expected_parity
    );
}

#[test]
fn test_conversion_premium_metric() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::custom("conversion_premium")],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_ok(),
        "Conversion premium metric should calculate successfully"
    );

    let valuation = result.unwrap();
    assert!(
        valuation.measures.contains_key("conversion_premium"),
        "Should contain conversion_premium metric"
    );

    let premium = valuation.measures.get("conversion_premium").unwrap();

    // Conversion premium = (bond_price / conversion_value) - 1
    // Should be non-negative (bond price >= conversion value)
    assert!(
        *premium >= -0.10, // Allow small negative due to numerical issues
        "Conversion premium should be reasonable: {}",
        premium
    );
}

#[test]
fn test_delta_metric() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Delta],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(result.is_ok(), "Delta metric should calculate successfully");

    let valuation = result.unwrap();
    assert!(
        valuation.measures.contains_key("delta"),
        "Should contain delta metric"
    );

    let delta = valuation.measures.get("delta").unwrap();

    // Delta should be positive for ITM convertible
    assert!(*delta > 0.0, "Delta should be positive: {}", delta);

    // Delta should not exceed conversion ratio
    assert!(
        *delta <= bond_params::CONVERSION_RATIO * 1.1,
        "Delta should not exceed conversion ratio: {}",
        delta
    );
}

#[test]
fn test_gamma_metric() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Gamma],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(result.is_ok(), "Gamma metric should calculate successfully");

    let valuation = result.unwrap();
    assert!(
        valuation.measures.contains_key("gamma"),
        "Should contain gamma metric"
    );

    let gamma = valuation.measures.get("gamma").unwrap();

    // Gamma should be non-negative
    assert!(*gamma >= 0.0, "Gamma should be non-negative: {}", gamma);
}

#[test]
fn test_vega_metric() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Vega],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(result.is_ok(), "Vega metric should calculate successfully");

    let valuation = result.unwrap();
    assert!(
        valuation.measures.contains_key("vega"),
        "Should contain vega metric"
    );

    let vega = valuation.measures.get("vega").unwrap();

    // Vega should be non-negative
    assert!(*vega >= 0.0, "Vega should be non-negative: {}", vega);
}

#[test]
fn test_rho_metric() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Rho],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(result.is_ok(), "Rho metric should calculate successfully");

    let valuation = result.unwrap();
    assert!(
        valuation.measures.contains_key("rho"),
        "Should contain rho metric"
    );

    let rho = valuation.measures.get("rho").unwrap();

    // Rho should be finite
    assert!(rho.is_finite(), "Rho should be finite: {}", rho);
}

#[test]
fn test_theta_metric() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Theta],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(result.is_ok(), "Theta metric should calculate successfully");

    let valuation = result.unwrap();
    assert!(
        valuation.measures.contains_key("theta"),
        "Should contain theta metric"
    );

    let theta = valuation.measures.get("theta").unwrap();

    // Theta should be finite
    assert!(theta.is_finite(), "Theta should be finite: {}", theta);
}

#[test]
fn test_multiple_metrics() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let metrics = vec![
        MetricId::custom("parity"),
        MetricId::custom("conversion_premium"),
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
    ];

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_ok(),
        "Multiple metrics should calculate successfully"
    );

    let valuation = result.unwrap();

    // All requested metrics should be present
    assert!(valuation.measures.contains_key("parity"));
    assert!(valuation.measures.contains_key("conversion_premium"));
    assert!(valuation.measures.contains_key("delta"));
    assert!(valuation.measures.contains_key("gamma"));
    assert!(valuation.measures.contains_key("vega"));
}

#[test]
fn test_all_greeks_together() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Rho,
        MetricId::Theta,
    ];

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(result.is_ok(), "All Greeks should calculate successfully");

    let valuation = result.unwrap();

    // All Greeks should be present and finite
    let delta = valuation.measures.get("delta").unwrap();
    let gamma = valuation.measures.get("gamma").unwrap();
    let vega = valuation.measures.get("vega").unwrap();
    let rho = valuation.measures.get("rho").unwrap();
    let theta = valuation.measures.get("theta").unwrap();

    assert!(delta.is_finite(), "Delta should be finite");
    assert!(gamma.is_finite(), "Gamma should be finite");
    assert!(vega.is_finite(), "Vega should be finite");
    assert!(rho.is_finite(), "Rho should be finite");
    assert!(theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_metrics_with_callable_bond() {
    let call_date = dates::mid_date();
    let bond = create_callable_convertible(call_date, 102.0);
    let market = create_market_context();
    let as_of = dates::base_date();

    let metrics = vec![MetricId::custom("parity"), MetricId::Delta, MetricId::Gamma];

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_ok(),
        "Metrics should work for callable convertibles"
    );
}

#[test]
fn test_metrics_with_puttable_bond() {
    let put_date = dates::mid_date();
    let bond = create_puttable_convertible(put_date, 98.0);
    let market = create_market_context();
    let as_of = dates::base_date();

    let metrics = vec![MetricId::custom("parity"), MetricId::Delta, MetricId::Vega];

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_ok(),
        "Metrics should work for puttable convertibles"
    );
}

#[test]
fn test_metrics_with_zero_coupon() {
    let bond = create_zero_coupon_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let metrics = vec![MetricId::custom("parity"), MetricId::Delta];

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &metrics,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_ok(),
        "Metrics should work for zero coupon convertibles"
    );
}

#[test]
fn test_valuation_result_structure() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Check valuation result structure
    assert_eq!(
        result.instrument_id,
        bond.id(),
        "Instrument ID should match"
    );
    assert_eq!(result.as_of, as_of, "As-of date should match");
    assert!(result.value.amount() > 0.0, "Base value should be positive");
    assert!(!result.measures.is_empty(), "Should have metrics");
}

#[test]
fn test_empty_metrics_request() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(result.is_ok(), "Should work with empty metrics request");

    let valuation = result.unwrap();
    assert!(valuation.measures.is_empty(), "Should have no metrics");
    assert!(
        valuation.value.amount() > 0.0,
        "Should still have base value"
    );
}

#[test]
fn test_metrics_consistency_with_direct_calculation() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    // Calculate via metrics framework
    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::custom("parity")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let metric_parity = *result.measures.get("parity").unwrap();

    // Calculate directly
    let direct_parity = bond.parity(&market).unwrap();

    // Should match
    assert!(
        (metric_parity - direct_parity).abs() < TOLERANCE,
        "Metric parity {} should match direct calculation {}",
        metric_parity,
        direct_parity
    );
}

#[test]
fn test_bucketed_dv01_metric() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::BucketedDv01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // BucketedDv01 should work for convertibles
    assert!(
        result.is_ok(),
        "BucketedDv01 metric should work for convertibles"
    );
}

#[test]
fn test_custom_metrics_only() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let metrics = vec![
        MetricId::custom("parity"),
        MetricId::custom("conversion_premium"),
    ];

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert_eq!(
        result.measures.len(),
        2,
        "Should have exactly 2 custom metrics"
    );
}

#[test]
fn test_standard_metrics_only() {
    let bond = create_standard_convertible();
    let market = create_market_context();
    let as_of = dates::base_date();

    let metrics = vec![MetricId::Delta, MetricId::Gamma, MetricId::Vega];

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert_eq!(
        result.measures.len(),
        3,
        "Should have exactly 3 standard metrics"
    );
}
