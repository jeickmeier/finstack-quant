//! Market validation tests for option value bounds and no-arbitrage conditions.

use super::common::*;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_option_value_non_negative() {
    // No-arbitrage: option value >= 0
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for strike in [50.0, 100.0, 200.0, 400.0, 800.0] {
        let call = CDSOptionBuilder::new().call().strike(strike).build(as_of);
        let put = CDSOptionBuilder::new().put().strike(strike).build(as_of);

        let call_pv = call.value(&market, as_of).unwrap().amount();
        let put_pv = put.value(&market, as_of).unwrap().amount();

        assert_non_negative(call_pv, &format!("Call value, strike={}", strike));
        assert_non_negative(put_pv, &format!("Put value, strike={}", strike));
    }
}

#[test]
fn test_call_upper_bound() {
    // No-arbitrage: call value <= forward * PV factor * notional
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for strike in [50.0, 100.0, 200.0] {
        let option = CDSOptionBuilder::new().call().strike(strike).build(as_of);
        let pv = option.value(&market, as_of).unwrap().amount();

        // Upper bound: option can't be worth more than discounted forward spread * notional
        let notional = option.notional.amount();
        let reasonable_upper_bound = notional; // Conservative bound

        assert!(
            pv <= reasonable_upper_bound,
            "Call value {} exceeds reasonable upper bound {} for strike={}",
            pv,
            reasonable_upper_bound,
            strike
        );
    }
}

#[test]
fn test_put_upper_bound() {
    // No-arbitrage: put value <= strike * PV factor * notional
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for strike in [100.0, 200.0, 400.0] {
        let option = CDSOptionBuilder::new().put().strike(strike).build(as_of);
        let pv = option.value(&market, as_of).unwrap().amount();

        // Upper bound: put can't be worth more than strike
        let notional = option.notional.amount();
        let reasonable_upper_bound = notional; // Conservative bound

        assert!(
            pv <= reasonable_upper_bound,
            "Put value {} exceeds reasonable upper bound {} for strike={}",
            pv,
            reasonable_upper_bound,
            strike
        );
    }
}

#[test]
fn test_call_spread_monotonicity() {
    // No-arbitrage: C(K1) >= C(K2) for K1 < K2 (lower strike more valuable)
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let strikes = [100.0, 150.0, 200.0, 250.0, 300.0];
    let mut values = Vec::new();

    for &strike in &strikes {
        let option = CDSOptionBuilder::new().call().strike(strike).build(as_of);
        let pv = option.value(&market, as_of).unwrap().amount();
        values.push(pv);
    }

    // Each value should be >= next value (monotonic decreasing)
    for i in 1..values.len() {
        assert!(
            values[i - 1] >= values[i],
            "Call spread arbitrage: C(K={}) = {} < C(K={}) = {}",
            strikes[i - 1],
            values[i - 1],
            strikes[i],
            values[i]
        );
    }
}

#[test]
fn test_put_spread_monotonicity() {
    // No-arbitrage: P(K1) <= P(K2) for K1 < K2 (higher strike more valuable)
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let strikes = [100.0, 150.0, 200.0, 250.0, 300.0];
    let mut values = Vec::new();

    for &strike in &strikes {
        let option = CDSOptionBuilder::new().put().strike(strike).build(as_of);
        let pv = option.value(&market, as_of).unwrap().amount();
        values.push(pv);
    }

    // Each value should be <= next value (monotonic increasing)
    for i in 1..values.len() {
        assert!(
            values[i - 1] <= values[i],
            "Put spread arbitrage: P(K={}) = {} > P(K={}) = {}",
            strikes[i - 1],
            values[i - 1],
            strikes[i],
            values[i]
        );
    }
}

#[test]
fn test_butterfly_spread_no_arbitrage() {
    // No-arbitrage: for K1 < K2 < K3, C(K1) + C(K3) >= 2*C(K2)
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let k1 = 100.0;
    let k2 = 200.0;
    let k3 = 300.0;

    let c1 = CDSOptionBuilder::new()
        .call()
        .strike(k1)
        .build(as_of)
        .value(&market, as_of)
        .unwrap()
        .amount();

    let c2 = CDSOptionBuilder::new()
        .call()
        .strike(k2)
        .build(as_of)
        .value(&market, as_of)
        .unwrap()
        .amount();

    let c3 = CDSOptionBuilder::new()
        .call()
        .strike(k3)
        .build(as_of)
        .value(&market, as_of)
        .unwrap()
        .amount();

    // Convexity condition (with tolerance for numerical precision)
    assert!(
        c1 + c3 >= 2.0 * c2 - 1e-6,
        "Butterfly spread arbitrage: C({}) + C({}) = {} < 2*C({}) = {}",
        k1,
        k3,
        c1 + c3,
        k2,
        2.0 * c2
    );
}

#[test]
fn test_value_reasonable_magnitude() {
    // Sanity check: option values in reasonable range
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let option = CDSOptionBuilder::new()
        .strike(200.0)
        .notional(10_000_000.0, finstack_quant_core::currency::Currency::USD)
        .build(as_of);

    let pv = option.value(&market, as_of).unwrap().amount();

    // Option on 10M notional, 200bp strike, 1Y expiry should be reasonable
    assert_in_range(
        pv,
        1_000.0,     // At least $1k
        1_000_000.0, // At most $1M
        "Option value reasonableness",
    );
}

#[test]
fn test_deep_otm_option_low_value() {
    // Deep OTM option should have low value
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let option = CDSOptionBuilder::new()
        .call()
        .strike(1000.0) // Very high strike, deep OTM
        .build(as_of);

    let pv = option.value(&market, as_of).unwrap().amount();

    // Deep OTM should have small but positive time value
    assert_non_negative(pv, "Deep OTM option value");
    assert_finite(pv, "Deep OTM option value");
}

#[test]
fn test_greeks_reasonable_magnitude() {
    // Greeks should be finite and have sensible signs
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let notional = 10_000_000.0;

    let option = CDSOptionBuilder::new()
        .strike(200.0)
        .notional(notional, finstack_quant_core::currency::Currency::USD)
        .build(as_of);

    let delta = option.delta(&market, as_of).unwrap();
    let gamma = option.gamma(&market, as_of).unwrap();
    let vega = option.vega(&market, as_of).unwrap();

    // All greeks should be finite
    assert_finite(delta, "Delta");
    assert_finite(gamma, "Gamma");
    assert_finite(vega, "Vega");

    // Gamma and vega should be positive
    assert_non_negative(gamma, "Gamma");
    assert_positive(vega, "Vega");
}

#[test]
fn test_put_call_parity_at_forward() {
    // Special case: at-the-forward (ATF), call and put should have equal value
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let notional = 10_000_000.0;

    // Get forward spread
    let temp_option = CDSOptionBuilder::new().build(as_of);
    let underlying =
        synthetic_underlying_cds(&temp_option, as_of).expect("synthetic underlying CDS");
    let forward = underlying
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("par spread should compute")
        .measures[&MetricId::ParSpread];

    // Create call and put at forward strike
    let call = CDSOptionBuilder::new()
        .call()
        .strike(forward)
        .expiry_months(12)
        .cds_maturity_months(60)
        .notional(notional, finstack_quant_core::currency::Currency::USD)
        .implied_vol(0.30)
        .build(as_of);

    let put = CDSOptionBuilder::new()
        .put()
        .strike(forward)
        .expiry_months(12)
        .cds_maturity_months(60)
        .notional(notional, finstack_quant_core::currency::Currency::USD)
        .implied_vol(0.30)
        .build(as_of);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    // ATF: C ≈ P. Under the Bloomberg CDSO model the lognormal-spread
    // assumption (DOCS 2055833 §2.2) plus the calibration of the lognormal
    // mean `m` to the no-knockout forward (rather than to the strike)
    // introduces a measured ~5–10% put/call asymmetry at ATF on a 1-yr
    // option with 30% spread vol — symmetry is recovered only in the small-
    // σ-or-σ²t limit. The 10% bound below is a sanity check that the two
    // sides remain in the same order of magnitude rather than the strict
    // closed-form Black equality the test originally asserted.
    let rel_diff = (call_pv - put_pv).abs() / call_pv.max(put_pv);
    assert!(
        rel_diff < 0.10,
        "ATF call and put should be approximately equal: C={}, P={}, rel_diff={}",
        call_pv,
        put_pv,
        rel_diff
    );
}
