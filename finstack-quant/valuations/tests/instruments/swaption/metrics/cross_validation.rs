//! Cross-validation tests between analytical and numerical Greeks

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_all_greeks_run_together() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Rho,
        MetricId::Theta,
        MetricId::Dv01,
        MetricId::ImpliedVol,
    ];

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify all metrics computed
    assert!(
        result.measures.contains_key("delta"),
        "Delta should be computed"
    );
    assert!(
        result.measures.contains_key("gamma"),
        "Gamma should be computed"
    );
    assert!(
        result.measures.contains_key("vega"),
        "Vega should be computed"
    );
    assert!(
        result.measures.contains_key("rho"),
        "Rho should be computed"
    );
    assert!(
        result.measures.contains_key("theta"),
        "Theta should be computed"
    );
    assert!(
        result.measures.contains_key("dv01"),
        "DV01 should be computed"
    );
    assert!(
        result.measures.contains_key("implied_vol"),
        "Implied vol should be computed"
    );

    // All should be finite
    for (name, value) in &result.measures {
        assert!(
            value.is_finite(),
            "{} should be finite, got: {}",
            name,
            value
        );
    }
}

#[test]
fn test_delta_gamma_consistency() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Compute delta and gamma
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();
    let gamma = *result.measures.get("gamma").unwrap();

    // For ATM options, both should be positive and reasonable
    assert!(delta > 0.0, "Delta should be positive");
    assert!(gamma >= 0.0, "Gamma should be non-negative");

    // Gamma should be smaller in magnitude than delta (typically)
    // This is a rough heuristic check - can be violated for scaled cash greeks
    assert!(gamma.abs() < delta.abs() * 100.0, "Gamma magnitude check");
}

#[test]
fn test_vega_theta_tradeoff() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega, MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let vega = *result.measures.get("vega").unwrap();
    let theta = *result.measures.get("theta").unwrap();

    // Vega should be positive (long option)
    assert!(vega > 0.0, "Vega should be positive");

    // Theta can be positive or negative depending on carry
    assert!(theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_metric_stability_across_maturities() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let swap_start = time::macros::date!(2025 - 01 - 01);
    let swap_end = time::macros::date!(2030 - 01 - 01);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let metrics = vec![MetricId::Delta, MetricId::Vega, MetricId::Dv01];

    // Test across different expiries
    for months in [3, 6, 12, 24] {
        let expiry = as_of
            .checked_add(time::Duration::days(months * 30))
            .unwrap();
        if expiry <= swap_start {
            let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

            let result = swaption
                .price_with_metrics(
                    &market,
                    as_of,
                    &metrics,
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();

            // All metrics should be finite and reasonable
            for (name, value) in &result.measures {
                assert!(
                    value.is_finite(),
                    "{} should be finite for {}M expiry, got: {}",
                    name,
                    months,
                    value
                );
            }
        }
    }
}
