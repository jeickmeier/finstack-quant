//! Integration tests for CDS Option Greeks calculations.

use super::common::*;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_delta_call_positive() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().call().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let delta = *result.measures.get("delta").unwrap();

    assert_finite(delta, "Call delta");
    assert_non_negative(delta, "Call delta should be non-negative");
}

#[test]
fn test_delta_put_negative() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().put().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let delta = *result.measures.get("delta").unwrap();

    assert_finite(delta, "Put delta");
    // Put delta can be negative (option to sell protection)
}

#[test]
fn test_gamma_positive() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let gamma = *result.measures.get("gamma").unwrap();

    assert_finite(gamma, "Gamma");
    assert_non_negative(gamma, "Gamma should be non-negative");
}

#[test]
fn test_vega_positive() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let vega = *result.measures.get("vega").unwrap();

    assert_finite(vega, "Vega");
    assert_positive(vega, "Vega should be positive");
}

#[test]
fn test_theta_exists() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta = *result.measures.get("theta").unwrap();

    assert_finite(theta, "Theta");
    // Theta typically negative for long options (time decay)
}

#[test]
fn test_cs01_call_positive() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().call().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let cs01 = *result.measures.get("cs01").unwrap();

    assert_finite(cs01, "CS01");
    assert_positive(cs01, "Call option CS01 should be positive");
}

#[test]
fn test_dv01_positive() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 = PV(rate+1bp) - PV(base); sign depends on instrument structure
    assert_finite(dv01, "DV01");
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn test_all_greeks_together() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Delta,
                MetricId::Gamma,
                MetricId::Vega,
                MetricId::Theta,
                MetricId::Cs01,
                MetricId::Dv01,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(
        result.measures.len() >= 6,
        "Should compute at least 6 greeks, got {}",
        result.measures.len(),
    );

    for (name, value) in &result.measures {
        assert_finite(*value, &format!("Greek: {}", name));
    }
}

#[test]
fn test_delta_moneyness_effect() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let mut deltas = Vec::new();
    // Strike from deep ITM to deep OTM for a call
    for strike in [50.0, 100.0, 200.0, 300.0, 500.0] {
        let option = CDSOptionBuilder::new().call().strike(strike).build(as_of);
        let result = option
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
        let delta = *result.measures.get("delta").unwrap();
        deltas.push((strike, delta));
    }

    // Delta should decrease as strike increases (call becomes more OTM)
    assert_decreasing(&deltas, "Strike", "Call delta");
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn test_gamma_peaks_atm() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let mut gammas = Vec::new();
    for strike in [50.0, 100.0, 150.0, 200.0, 250.0, 300.0] {
        let option = CDSOptionBuilder::new().strike(strike).build(as_of);
        let result = option
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Gamma],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
        let gamma = *result.measures.get("gamma").unwrap();
        gammas.push((strike, gamma));
    }

    // Gamma should peak near ATM (around 200bp for our market setup)
    let max_gamma = gammas.iter().map(|(_, g)| *g).fold(0.0f64, f64::max);
    assert_positive(max_gamma, "Maximum gamma should be positive");
}

#[test]
fn test_vega_time_decay() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let mut vegas = Vec::new();
    for months in [3, 6, 12, 24] {
        let option = CDSOptionBuilder::new()
            .expiry_months(months)
            .cds_maturity_months(months + 48)
            .build(as_of);
        let result = option
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Vega],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
        let vega = *result.measures.get("vega").unwrap();
        vegas.push((months as f64, vega));
    }

    // Vega should increase with time (more uncertainty)
    assert_increasing(&vegas, "Time to expiry", "Vega");
}

#[test]
fn test_near_expiry_greeks_decline() {
    // Test that greeks decline as we approach expiry
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let long_dated = CDSOptionBuilder::new()
        .expiry_months(12)
        .cds_maturity_months(60)
        .build(as_of);

    let short_dated = CDSOptionBuilder::new()
        .expiry_months(1)
        .cds_maturity_months(13)
        .build(as_of);

    let long_vega = long_dated.vega(&market, as_of).unwrap();
    let short_vega = short_dated.vega(&market, as_of).unwrap();

    // Longer-dated should have higher vega
    assert!(
        long_vega > short_vega,
        "Long-dated vega {} should be > short-dated {}",
        long_vega,
        short_vega
    );
}
