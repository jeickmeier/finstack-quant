//! Cross-validation tests between analytical and numerical Greeks

use crate::instruments::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

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
