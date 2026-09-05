//! Base amount metric tests.

use super::super::common::*;
use finstack_quant_core::{
    currency::Currency, dates::Date, market_data::context::MarketContext, money::Money,
};
use finstack_quant_valuations::{
    instruments::{FxSpot, Instrument},
    metrics::{MetricContext, MetricId},
};
use std::sync::Arc;

fn create_context(fx: FxSpot, as_of: Date) -> MetricContext {
    let market = MarketContext::new();
    let base_value = fx.value(&market, as_of).unwrap();
    let instrument: Arc<dyn Instrument> = Arc::new(fx);
    MetricContext::new(
        instrument,
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    )
}

#[test]
fn test_base_amount_default_notional() {
    let fx = sample_eurusd().with_rate(1.20).expect("test rate");
    let mut ctx = create_context(fx, test_date());

    let amount = calculate_metric(&mut ctx, MetricId::BaseAmount).unwrap();
    approx_eq(amount, 1.0, EPSILON, "Default notional");
}

#[test]
fn test_base_amount_explicit_notional() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let mut ctx = create_context(fx, test_date());

    let amount = calculate_metric(&mut ctx, MetricId::BaseAmount).unwrap();
    approx_eq(amount, 1_000_000.0, EPSILON, "Explicit notional");
}

#[test]
fn test_base_amount_various_currencies() {
    // EUR base
    let eur_fx = eurusd_with_notional(5_000_000.0, 1.20);
    let mut eur_ctx = create_context(eur_fx, test_date());
    approx_eq(
        calculate_metric(&mut eur_ctx, MetricId::BaseAmount).unwrap(),
        5_000_000.0,
        EPSILON,
        "EUR base",
    );

    // GBP base
    let gbp_fx = sample_gbpusd()
        .with_notional(Money::new(2_500_000.0, Currency::GBP).expect("valid money fixture"))
        .unwrap()
        .with_rate(1.40)
        .expect("test rate");
    let mut gbp_ctx = create_context(gbp_fx, test_date());
    approx_eq(
        calculate_metric(&mut gbp_ctx, MetricId::BaseAmount).unwrap(),
        2_500_000.0,
        EPSILON,
        "GBP base",
    );
}

#[test]
fn test_base_amount_zero_notional() {
    let fx = sample_eurusd()
        .with_notional(Money::new(0.0, Currency::EUR).expect("valid money fixture"))
        .unwrap()
        .with_rate(1.20)
        .expect("test rate");
    let mut ctx = create_context(fx, test_date());

    let amount = calculate_metric(&mut ctx, MetricId::BaseAmount).unwrap();
    approx_eq(amount, 0.0, EPSILON, "Zero notional");
}

#[test]
fn test_base_amount_large_notional() {
    let fx = eurusd_with_notional(1_000_000_000.0, 1.20);
    let mut ctx = create_context(fx, test_date());

    let amount = calculate_metric(&mut ctx, MetricId::BaseAmount).unwrap();
    approx_eq(amount, 1_000_000_000.0, 1.0, "Large notional");
}

#[test]
fn test_base_amount_fractional_notional() {
    let fx = eurusd_with_notional(1_234_567.89, 1.20);
    let mut ctx = create_context(fx, test_date());

    let amount = calculate_metric(&mut ctx, MetricId::BaseAmount).unwrap();
    approx_eq(amount, 1_234_567.89, EPSILON, "Fractional notional");
}

#[test]
fn test_base_amount_independent_of_rate() {
    let fx1 = eurusd_with_notional(1_000_000.0, 1.10);
    let fx2 = eurusd_with_notional(1_000_000.0, 1.50);

    let mut ctx1 = create_context(fx1, test_date());
    let mut ctx2 = create_context(fx2, test_date());

    let amount1 = calculate_metric(&mut ctx1, MetricId::BaseAmount).unwrap();
    let amount2 = calculate_metric(&mut ctx2, MetricId::BaseAmount).unwrap();

    approx_eq(amount1, amount2, EPSILON, "Independent of rate");
    approx_eq(amount1, 1_000_000.0, EPSILON, "Base amount");
}

#[test]
fn test_base_amount_independent_of_date() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);

    let mut ctx1 = create_context(fx.clone(), d(2025, 1, 15));
    let mut ctx2 = create_context(fx.clone(), d(2025, 6, 15));
    let mut ctx3 = create_context(fx, d(2026, 1, 15));

    let amount1 = calculate_metric(&mut ctx1, MetricId::BaseAmount).unwrap();
    let amount2 = calculate_metric(&mut ctx2, MetricId::BaseAmount).unwrap();
    let amount3 = calculate_metric(&mut ctx3, MetricId::BaseAmount).unwrap();

    approx_eq(amount1, amount2, EPSILON, "Date independence 1");
    approx_eq(amount1, amount3, EPSILON, "Date independence 2");
}

#[test]
fn test_base_amount_returns_base_currency_amount() {
    // Verify that base_amount always returns the amount in base currency,
    // not quote currency
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let mut ctx = create_context(fx, test_date());

    let base_amount = calculate_metric(&mut ctx, MetricId::BaseAmount).unwrap();

    // Base amount should be 1M EUR (base currency)
    approx_eq(base_amount, 1_000_000.0, EPSILON, "Base currency amount");

    // Not 1.2M USD (quote currency value)
    assert!((base_amount - 1_200_000.0).abs() > 1.0);
}
