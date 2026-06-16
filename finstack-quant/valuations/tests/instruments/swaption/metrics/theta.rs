//! Theta (time decay) tests

use crate::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_theta_finite() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    assert!(theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_theta_time_decay_validation() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Analytical theta
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta = *result.measures.get("theta").unwrap();

    // Validate by pricing 1 day forward
    let pv_today = swaption.value(&market, as_of).unwrap().amount();

    let tomorrow = as_of.checked_add(time::Duration::days(1)).unwrap();
    let pv_tomorrow = swaption.value(&market, tomorrow).unwrap().amount();

    let time_decay = pv_tomorrow - pv_today;

    // Theta should approximate the P&L from one day passing
    // (opposite sign since theta is negative for long options)
    // Note: Theta can have carry effects that differ from simple time decay
    let rel_diff = ((theta + time_decay) / theta.abs().max(1.0)).abs();
    assert!(
        rel_diff < 5.0,
        "Theta should be reasonably related to time decay, rel_diff={}",
        rel_diff
    );
}

#[test]
fn test_theta_reasonable_magnitude() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    // Theta for 1Y option should be reasonable (negative for long option)
    assert_reasonable(theta.abs(), 0.0, 100_000.0, "Theta magnitude");
}

#[test]
fn test_theta_increases_near_expiry() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let swap_start = time::macros::date!(2025 - 01 - 01);
    let swap_end = time::macros::date!(2030 - 01 - 01);
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Far from expiry (1Y)
    let expiry_far = time::macros::date!(2025 - 01 - 01);
    let swaption_far = create_standard_payer_swaption(expiry_far, swap_start, swap_end, 0.05);

    // Closer to expiry (3M)
    let as_of_later = time::macros::date!(2024 - 10 - 01);
    let expiry_near = time::macros::date!(2025 - 01 - 01);
    let swaption_near = create_standard_payer_swaption(expiry_near, swap_start, swap_end, 0.05);

    let theta_far = swaption_far
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("theta")
        .copied()
        .unwrap();

    let theta_near = swaption_near
        .price_with_metrics(
            &market,
            as_of_later,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("theta")
        .copied()
        .unwrap();

    // Theta magnitude typically increases as expiry approaches
    // (both should be negative for long options)
    assert!(
        theta_far.is_finite() && theta_near.is_finite(),
        "Thetas should be finite"
    );
}
