//! Theta metric tests for FX Spot.

use super::super::common::*;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{currency::Currency, dates::Date, money::Money};
use finstack_quant_valuations::{
    instruments::{FxSpot, Instrument},
    metrics::MetricId,
};

fn theta_for(fx: FxSpot, market: &MarketContext, as_of: Date) -> f64 {
    let result = fx
        .price_with_metrics(
            market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing with theta should succeed");
    *result
        .measures
        .get(MetricId::Theta.as_str())
        .unwrap_or(&0.0)
}

#[test]
fn test_theta_basic() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2025, 1, 17));
    let market = market_full();
    let theta = theta_for(fx, &market, test_date());

    // For FX spot with explicit rate, theta should be very small or zero
    // since value doesn't change with time when rate is fixed
    assert!(theta.abs() < 100.0, "Theta should be small for fixed rate");
}

#[test]
fn test_theta_settled_position() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2025, 1, 10)); // Past
    let market = market_full();
    let theta = theta_for(fx, &market, test_date());

    // Settled position has zero theta
    assert_approx_eq(theta, 0.0, EPSILON, "Theta zero for settled position");
}

#[test]
fn test_theta_zero_notional() {
    let fx = sample_eurusd()
        .with_notional(Money::new(0.0, Currency::EUR))
        .unwrap()
        .with_rate(1.20)
        .expect("test rate")
        .with_settlement(d(2025, 1, 17));
    let market = market_full();
    let theta = theta_for(fx, &market, test_date());
    assert_approx_eq(theta, 0.0, EPSILON, "Theta zero for zero notional");
}

#[test]
fn test_theta_calculation_completes() {
    // Regression test: ensure theta calculation completes without error
    let test_cases = vec![
        eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2025, 1, 17)),
        eurusd_with_notional(5_000_000.0, 1.25).with_settlement(d(2025, 2, 15)),
        sample_gbpusd()
            .with_notional(Money::new(2_000_000.0, Currency::GBP))
            .unwrap()
            .with_rate(1.40)
            .expect("test rate")
            .with_settlement(d(2025, 3, 15)),
    ];

    for fx in test_cases {
        let market = market_full();
        let _ = theta_for(fx, &market, test_date());
    }
}
