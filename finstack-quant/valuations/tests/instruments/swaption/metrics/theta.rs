//! Theta (time decay) tests

use crate::instruments::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_theta_matches_one_day_rolled_reprice() {
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
    let theta = result.measures[MetricId::Theta.as_str()];

    let tomorrow = as_of.checked_add(time::Duration::days(1)).unwrap();
    let rolled_market = market
        .roll_forward(1)
        .expect("flat swaption market should roll forward one day");
    let base_pv = swaption.value(&market, as_of).unwrap().amount();
    let rolled_pv = swaption.value(&rolled_market, tomorrow).unwrap().amount();
    let expected_theta = rolled_pv - base_pv;
    let theta_error = (theta - expected_theta).abs();

    assert!(
        theta_error <= 1e-10,
        "Theta should equal the same-sign one-day rolled-market P&L: actual={theta}, expected={expected_theta}, error={theta_error}"
    );
    assert!(
        theta < 0.0,
        "Long ATM payer swaption should lose time value, got theta={theta}"
    );
}

#[test]
fn test_atm_theta_magnitude_increases_near_expiry_on_flat_market() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let expiry = time::macros::date!(2025 - 01 - 01);
    let swap_start = expiry;
    let swap_end = time::macros::date!(2030 - 01 - 01);
    let as_of_later = time::macros::date!(2024 - 10 - 01);
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market_far = create_flat_market(as_of, 0.05, 0.30);
    let market_near = create_flat_market(as_of_later, 0.05, 0.30);

    let theta_far = swaption
        .price_with_metrics(
            &market_far,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures[MetricId::Theta.as_str()];

    let theta_near = swaption
        .price_with_metrics(
            &market_near,
            as_of_later,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures[MetricId::Theta.as_str()];

    assert!(
        theta_far < 0.0 && theta_near < 0.0,
        "Long ATM payer swaption thetas should be negative: far={theta_far}, near={theta_near}"
    );
    assert!(
        theta_near.abs() > theta_far.abs(),
        "ATM theta magnitude should increase near expiry on the flat market: far={theta_far}, near={theta_near}"
    );
}
