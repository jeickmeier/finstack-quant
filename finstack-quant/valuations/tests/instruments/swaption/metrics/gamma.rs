//! Gamma tests

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_gamma_positive_for_long_option() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let gamma = *result.measures.get("gamma").unwrap();

    // Long options have positive gamma
    assert!(gamma >= 0.0, "Long option gamma should be non-negative");
    assert!(gamma.is_finite(), "Gamma should be finite");
}

#[test]
fn test_atm_gamma_highest() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    let atm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let itm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.03);
    let otm = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.07);

    let gamma_atm = atm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .copied()
        .unwrap();

    let gamma_itm = itm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .copied()
        .unwrap();

    let gamma_otm = otm
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .copied()
        .unwrap();

    // ATM options typically have highest gamma; allow small numerical slack
    let max_other = gamma_itm.max(gamma_otm);
    let tol = max_other.abs() * 0.15 + 1e-12; // 15% relative tolerance
    assert!(
        gamma_atm + tol >= max_other,
        "ATM gamma should be near the max; atm={}, itm={}, otm={}",
        gamma_atm,
        gamma_itm,
        gamma_otm
    );
}

#[test]
fn test_gamma_reasonable_magnitude() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let gamma = *result.measures.get("gamma").unwrap();

    // Gamma should be finite and positive
    assert_reasonable(gamma, 0.0, 1e10, "Gamma magnitude");
}
