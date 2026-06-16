//! Near-expiry edge case tests for equity options.
//!
//! Tests numerical stability when T → 0, including:
//! - Gamma behavior (should increase for ATM options)
//! - Theta behavior (should be large negative)
//! - Delta behavior (should approach step function)
//! - Expired option handling

use super::helpers::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;
use time::Duration;

#[test]
fn test_near_expiry_gamma_stability() {
    let as_of = date!(2024 - 01 - 01);

    for days_to_expiry in [1, 3, 7, 14, 30] {
        let expiry = as_of + Duration::days(days_to_expiry);
        let call = create_call(as_of, expiry, 100.0);
        let market = build_standard_market(as_of, 100.0, 0.25, 0.05, 0.0);

        let result = call
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Gamma],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let gamma = *result.measures.get("gamma").unwrap();

        // Gamma must be finite and non-negative
        assert!(
            gamma.is_finite(),
            "Gamma must be finite at T={}d, got {}",
            days_to_expiry,
            gamma
        );
        assert!(
            gamma >= 0.0,
            "Gamma must be non-negative at T={}d, got {}",
            days_to_expiry,
            gamma
        );
    }
}

#[test]
fn test_gamma_increases_near_expiry_atm() {
    // For ATM options, gamma should increase as T → 0
    let as_of = date!(2024 - 01 - 01);
    let market = build_standard_market(as_of, 100.0, 0.25, 0.05, 0.0);

    let expiry_30d = as_of + Duration::days(30);
    let call_30d = create_call(as_of, expiry_30d, 100.0);
    let result_30d = call_30d
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let gamma_30d = *result_30d.measures.get("gamma").unwrap();

    let expiry_7d = as_of + Duration::days(7);
    let call_7d = create_call(as_of, expiry_7d, 100.0);
    let result_7d = call_7d
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let gamma_7d = *result_7d.measures.get("gamma").unwrap();

    assert!(
        gamma_7d > gamma_30d,
        "ATM gamma should increase near expiry: gamma(7d)={:.4} should be > gamma(30d)={:.4}",
        gamma_7d,
        gamma_30d
    );
}

#[test]
fn test_near_expiry_theta_stability() {
    let as_of = date!(2024 - 01 - 01);

    for days_to_expiry in [1, 3, 7, 14, 30] {
        let expiry = as_of + Duration::days(days_to_expiry);
        let call = create_call(as_of, expiry, 100.0);
        let market = build_standard_market(as_of, 100.0, 0.25, 0.05, 0.0);

        let result = call
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Theta],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let theta = *result.measures.get("theta").unwrap();

        // Theta must be finite (and typically negative for long options)
        assert!(
            theta.is_finite(),
            "Theta must be finite at T={}d, got {}",
            days_to_expiry,
            theta
        );
    }
}

#[test]
fn test_theta_magnitude_increases_near_expiry_atm() {
    // For ATM options, theta magnitude (absolute value) should increase as T → 0
    let as_of = date!(2024 - 01 - 01);
    let market = build_standard_market(as_of, 100.0, 0.25, 0.05, 0.0);

    let expiry_30d = as_of + Duration::days(30);
    let call_30d = create_call(as_of, expiry_30d, 100.0);
    let result_30d = call_30d
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta_30d = *result_30d.measures.get("theta").unwrap();

    let expiry_7d = as_of + Duration::days(7);
    let call_7d = create_call(as_of, expiry_7d, 100.0);
    let result_7d = call_7d
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta_7d = *result_7d.measures.get("theta").unwrap();

    // ATM theta should be negative (time decay)
    assert!(
        theta_30d < 0.0 && theta_7d < 0.0,
        "ATM theta should be negative: theta(7d)={:.4}, theta(30d)={:.4}",
        theta_7d,
        theta_30d
    );

    // |theta| should be larger near expiry
    assert!(
        theta_7d.abs() > theta_30d.abs(),
        "ATM |theta| should increase near expiry: |theta(7d)|={:.4} > |theta(30d)|={:.4}",
        theta_7d.abs(),
        theta_30d.abs()
    );
}

#[test]
fn test_near_expiry_delta_stability() {
    let as_of = date!(2024 - 01 - 01);

    for days_to_expiry in [1, 3, 7, 14, 30] {
        let expiry = as_of + Duration::days(days_to_expiry);
        let market = build_standard_market(as_of, 100.0, 0.25, 0.05, 0.0);

        // ATM call
        let call = create_call(as_of, expiry, 100.0);
        let result = call
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
        let delta = *result.measures.get("delta").unwrap();

        // Normalize delta by contract size (100) to get per-share delta
        // Cash delta = normalized_delta × contract_size
        let delta_normalized = delta / call.notional.amount();

        // Normalized delta must be in [0, 1] for calls
        assert!(
            (0.0..=1.0).contains(&delta_normalized),
            "Call delta must be in [0,1] at T={}d, got {} (normalized from cash delta {})",
            days_to_expiry,
            delta_normalized,
            delta
        );

        // ATM delta should be near 0.5 (slightly above due to drift)
        assert!(
            delta_normalized > 0.3 && delta_normalized < 0.7,
            "ATM call delta should be near 0.5 at T={}d, got {} (normalized from cash delta {})",
            days_to_expiry,
            delta_normalized,
            delta
        );
    }
}

#[test]
fn test_delta_approaches_step_function() {
    // As T → 0, delta should approach a step function (0 for OTM, 1 for ITM)
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of + Duration::days(1); // 1 day to expiry
    let market = build_standard_market(as_of, 100.0, 0.25, 0.05, 0.0);

    // Deep ITM call (S >> K)
    let itm_call = create_call(as_of, expiry, 80.0);
    let result_itm = itm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let delta_itm = *result_itm.measures.get("delta").unwrap();

    // Deep OTM call (S << K)
    let otm_call = create_call(as_of, expiry, 120.0);
    let result_otm = otm_call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let delta_otm = *result_otm.measures.get("delta").unwrap();

    // Near expiry, ITM delta should approach 1
    assert!(
        delta_itm > 0.90,
        "Deep ITM call delta should approach 1 near expiry, got {}",
        delta_itm
    );

    // Near expiry, OTM delta should approach 0
    assert!(
        delta_otm < 0.10,
        "Deep OTM call delta should approach 0 near expiry, got {}",
        delta_otm
    );
}

#[test]
fn test_expired_option_price_is_intrinsic() {
    // At expiry, option value should equal intrinsic value
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of; // Expired
    let spot = 100.0;
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    // ITM call: intrinsic = max(S - K, 0)
    let itm_call = create_call(as_of, expiry, 90.0);
    let pv_itm = itm_call.value(&market, as_of).unwrap().amount();
    let intrinsic_itm = (spot - 90.0).max(0.0) * itm_call.notional.amount();

    assert!(
        (pv_itm - intrinsic_itm).abs() < 0.01,
        "Expired ITM call should have intrinsic value: got {:.2}, expected {:.2}",
        pv_itm,
        intrinsic_itm
    );

    // OTM call: intrinsic = 0
    let otm_call = create_call(as_of, expiry, 110.0);
    let pv_otm = otm_call.value(&market, as_of).unwrap().amount();

    assert!(
        pv_otm.abs() < 0.01,
        "Expired OTM call should have zero value, got {:.2}",
        pv_otm
    );
}

#[test]
fn test_expired_option_greeks() {
    // Expired options should have well-defined Greeks
    let as_of = date!(2024 - 01 - 01);
    let expiry = as_of; // Expired
    let market = build_standard_market(as_of, 100.0, 0.25, 0.05, 0.0);

    let call = create_call(as_of, expiry, 100.0);
    let result = call
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Gamma, MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();
    let gamma = *result.measures.get("gamma").unwrap();
    let vega = *result.measures.get("vega").unwrap();

    // All Greeks should be finite
    assert!(
        delta.is_finite(),
        "Expired option delta should be finite, got {}",
        delta
    );
    assert!(
        gamma.is_finite(),
        "Expired option gamma should be finite, got {}",
        gamma
    );
    assert!(
        vega.is_finite(),
        "Expired option vega should be finite, got {}",
        vega
    );

    // Vega should be near zero at expiry (no time value)
    assert!(
        vega.abs() < 0.1,
        "Expired option vega should be near zero, got {}",
        vega
    );
}

#[test]
fn test_vega_decreases_near_expiry() {
    // Vega should decrease as T → 0 (less sensitivity to vol)
    let as_of = date!(2024 - 01 - 01);
    let market = build_standard_market(as_of, 100.0, 0.25, 0.05, 0.0);

    let expiry_30d = as_of + Duration::days(30);
    let call_30d = create_call(as_of, expiry_30d, 100.0);
    let result_30d = call_30d
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let vega_30d = *result_30d.measures.get("vega").unwrap();

    let expiry_7d = as_of + Duration::days(7);
    let call_7d = create_call(as_of, expiry_7d, 100.0);
    let result_7d = call_7d
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let vega_7d = *result_7d.measures.get("vega").unwrap();

    assert!(
        vega_30d > vega_7d,
        "Vega should decrease near expiry: vega(30d)={:.4} > vega(7d)={:.4}",
        vega_30d,
        vega_7d
    );
}
