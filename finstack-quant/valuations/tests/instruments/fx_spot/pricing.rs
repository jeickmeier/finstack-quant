//! FX Spot pricing tests.

use super::common::*;
use finstack_quant_core::{
    currency::Currency, market_data::context::MarketContext, money::Money, types::InstrumentId,
};
use finstack_quant_valuations::instruments::{FxSpot, Instrument};

#[test]
fn test_npv_with_explicit_rate() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.currency(), Currency::USD);
    assert_approx_eq(pv.amount(), 1_200_000.0, EPSILON, "NPV with explicit rate");
}

#[test]
fn test_npv_with_default_notional() {
    let fx = sample_eurusd().with_rate(1.18).expect("test rate");
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.currency(), Currency::USD);
    assert_approx_eq(pv.amount(), 1.18, EPSILON, "NPV with default notional");
}

#[test]
fn test_npv_from_fx_matrix() {
    let fx = sample_eurusd()
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap();
    let market = market_with_fx_matrix();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.currency(), Currency::USD);
    assert_approx_eq(
        pv.amount(),
        1_200_000.0,
        LARGE_EPSILON,
        "NPV from FX matrix",
    );
}

#[test]
fn test_npv_explicit_rate_overrides_matrix() {
    let fx = eurusd_with_notional(1_000_000.0, 1.25);
    let market = market_with_fx_matrix(); // Has EUR/USD = 1.20
    let pv = fx.value(&market, test_date()).unwrap();

    // Explicit rate should override matrix
    assert_eq!(pv.currency(), Currency::USD);
    assert_approx_eq(
        pv.amount(),
        1_250_000.0,
        EPSILON,
        "Explicit rate overrides matrix",
    );
}

#[test]
fn test_npv_without_rate_or_matrix_fails() {
    let fx = sample_eurusd()
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap();
    let market = MarketContext::new(); // No FX matrix

    let result = fx.value(&market, test_date());
    assert!(result.is_err());
}

#[test]
fn test_value_method() {
    let fx = eurusd_with_notional(2_000_000.0, 1.22);
    let market = MarketContext::new();
    let value = fx.value(&market, test_date()).unwrap();

    assert_eq!(value.currency(), Currency::USD);
    assert_approx_eq(value.amount(), 2_440_000.0, EPSILON, "Value method");
}

#[test]
fn test_gbpusd_pricing() {
    let fx = sample_gbpusd()
        .with_notional(Money::new(500_000.0, Currency::GBP))
        .unwrap();
    let market = market_with_fx_matrix(); // Has GBP/USD = 1.40
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.currency(), Currency::USD);
    assert_approx_eq(pv.amount(), 700_000.0, LARGE_EPSILON, "GBP/USD pricing");
}

#[test]
fn test_usdjpy_pricing() {
    let fx = sample_usdjpy()
        .with_notional(Money::new(100_000.0, Currency::USD))
        .unwrap();
    let market = market_with_fx_matrix(); // Has USD/JPY = 110.0
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.currency(), Currency::JPY);
    assert_approx_eq(pv.amount(), 11_000_000.0, LARGE_EPSILON, "USD/JPY pricing");
}

#[test]
fn test_zero_notional() {
    let fx = sample_eurusd()
        .with_notional(Money::new(0.0, Currency::EUR))
        .unwrap()
        .with_rate(1.20)
        .expect("test rate");
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_approx_eq(pv.amount(), 0.0, EPSILON, "Zero notional");
}

#[test]
fn test_large_notional_pricing() {
    let fx = eurusd_with_notional(1_000_000_000.0, 1.18);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_approx_eq(pv.amount(), 1_180_000_000.0, 1.0, "Large notional");
}

#[test]
fn test_very_small_rate() {
    let fx = eurusd_with_notional(1_000_000.0, 0.0001);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_approx_eq(pv.amount(), 100.0, EPSILON, "Very small rate");
}

#[test]
fn test_very_large_rate() {
    let fx = eurusd_with_notional(1_000.0, 1000.0);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_approx_eq(pv.amount(), 1_000_000.0, EPSILON, "Very large rate");
}

#[test]
fn test_pricing_consistency_across_dates() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();

    let pv1 = fx.value(&market, d(2025, 1, 15)).unwrap();
    let pv2 = fx.value(&market, d(2025, 6, 15)).unwrap();
    let pv3 = fx.value(&market, d(2026, 1, 15)).unwrap();

    // With explicit rate, PV should be independent of date
    assert_approx_eq(
        pv1.amount(),
        pv2.amount(),
        EPSILON,
        "PV consistency date1 vs date2",
    );
    assert_approx_eq(
        pv1.amount(),
        pv3.amount(),
        EPSILON,
        "PV consistency date1 vs date3",
    );
}

#[test]
fn test_price_with_metrics_base_value() {
    // Test price_with_metrics returns correct structure
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();

    let result = fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert_eq!(result.instrument_id, "EURUSD");
    assert_approx_eq(result.value.amount(), 1_200_000.0, EPSILON, "Base value");
}

#[test]
fn test_triangulated_rate() {
    // EUR/GBP should be EUR/USD / GBP/USD = 1.20 / 1.40
    let fx = FxSpot::new(InstrumentId::new("EURGBP"), Currency::EUR, Currency::GBP)
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap();

    let market = market_with_fx_matrix();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.currency(), Currency::GBP);
    let expected = 1_000_000.0 * (1.20 / 1.40);
    assert_approx_eq(pv.amount(), expected, LARGE_EPSILON, "Triangulated rate");
}

#[test]
fn test_inverse_pair() {
    // Test USD/EUR (inverse of EUR/USD)
    let fx = FxSpot::new(InstrumentId::new("USDEUR"), Currency::USD, Currency::EUR)
        .with_notional(Money::new(1_000_000.0, Currency::USD))
        .unwrap();

    let market = market_with_fx_matrix(); // Has EUR/USD = 1.20
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.currency(), Currency::EUR);
    let expected = 1_000_000.0 / 1.20; // Inverse of EUR/USD rate
    assert_approx_eq(pv.amount(), expected, LARGE_EPSILON, "Inverse pair");
}

#[test]
fn test_multiple_currencies_independence() {
    let eurusd = eurusd_with_notional(1_000_000.0, 1.20);
    let gbpusd = sample_gbpusd()
        .with_notional(Money::new(500_000.0, Currency::GBP))
        .unwrap()
        .with_rate(1.40)
        .expect("test rate");

    let market = MarketContext::new();

    let pv_eur = eurusd.value(&market, test_date()).unwrap();
    let pv_gbp = gbpusd.value(&market, test_date()).unwrap();

    assert_approx_eq(pv_eur.amount(), 1_200_000.0, EPSILON, "EUR/USD PV");
    assert_approx_eq(pv_gbp.amount(), 700_000.0, EPSILON, "GBP/USD PV");
}
