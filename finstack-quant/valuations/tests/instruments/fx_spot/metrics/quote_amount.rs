//! Quote amount tests (now in ValuationResult.value).
//!
//! The quote amount for FX Spot is simply the PV in the quote currency,
//! which is always available in `ValuationResult.value`.

use super::super::common::*;
use finstack_quant_core::{currency::Currency, market_data::context::MarketContext, money::Money};
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_quote_amount_basic() {
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

    // Quote amount is the PV in result.value
    let amount = result.value.amount();
    assert_approx_eq(amount, 1_200_000.0, EPSILON, "Quote amount");
}

#[test]
fn test_quote_amount_equals_npv() {
    let fx = eurusd_with_notional(2_000_000.0, 1.22);
    let market = MarketContext::new();
    let result = fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let direct_npv = fx.value(&market, test_date()).unwrap();

    let quote_amt = result.value.amount();
    assert_approx_eq(quote_amt, direct_npv.amount(), EPSILON, "Equals NPV");
}

#[test]
fn test_quote_amount_default_notional() {
    let fx = sample_eurusd().with_rate(1.18).expect("test rate");
    let market = MarketContext::new();
    let result = fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let amount = result.value.amount();
    assert_approx_eq(amount, 1.18, EPSILON, "Default notional quote amount");
}

#[test]
fn test_quote_amount_various_rates() {
    let market = MarketContext::new();
    let rates = vec![0.5, 1.0, 1.2, 1.5, 2.0, 100.0];
    let notional = 1_000_000.0;

    for rate in rates {
        let fx = eurusd_with_notional(notional, rate);
        let result = fx
            .price_with_metrics(
                &market,
                test_date(),
                &[],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let quote_amt = result.value.amount();
        let expected = notional * rate;

        assert_approx_eq(
            quote_amt,
            expected,
            EPSILON,
            &format!("Quote amount for rate {}", rate),
        );
    }
}

#[test]
fn test_quote_amount_various_currencies() {
    let market = MarketContext::new();

    // GBPUSD
    let gbp_fx = sample_gbpusd()
        .with_notional(Money::new(500_000.0, Currency::GBP))
        .unwrap()
        .with_rate(1.40)
        .expect("test rate");
    let gbp_result = gbp_fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    assert_approx_eq(
        gbp_result.value.amount(),
        700_000.0,
        EPSILON,
        "GBP/USD quote amount",
    );

    // USDJPY
    let jpy_fx = sample_usdjpy()
        .with_notional(Money::new(100_000.0, Currency::USD))
        .unwrap()
        .with_rate(110.0)
        .expect("test rate");
    let jpy_result = jpy_fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    assert_approx_eq(
        jpy_result.value.amount(),
        11_000_000.0,
        EPSILON,
        "USD/JPY quote amount",
    );
}

#[test]
fn test_quote_amount_zero_notional() {
    let fx = sample_eurusd()
        .with_notional(Money::new(0.0, Currency::EUR))
        .unwrap()
        .with_rate(1.20)
        .expect("test rate");
    let market = MarketContext::new();
    let result = fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let amount = result.value.amount();
    assert_approx_eq(amount, 0.0, EPSILON, "Zero notional");
}

#[test]
fn test_quote_amount_large_notional() {
    let fx = eurusd_with_notional(1_000_000_000.0, 1.20);
    let market = MarketContext::new();
    let result = fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let amount = result.value.amount();
    assert_approx_eq(amount, 1_200_000_000.0, 1.0, "Large notional");
}

#[test]
fn test_quote_amount_independence_from_date() {
    let market = MarketContext::new();
    let fx = eurusd_with_notional(1_000_000.0, 1.20);

    let result1 = fx
        .price_with_metrics(
            &market,
            d(2025, 1, 15),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result2 = fx
        .price_with_metrics(
            &market,
            d(2025, 6, 15),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result3 = fx
        .price_with_metrics(
            &market,
            d(2026, 1, 15),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let amount1 = result1.value.amount();
    let amount2 = result2.value.amount();
    let amount3 = result3.value.amount();

    // With explicit rate, quote amount should be date-independent
    assert_approx_eq(amount1, amount2, EPSILON, "Date independence 1");
    assert_approx_eq(amount1, amount3, EPSILON, "Date independence 2");
    assert_approx_eq(amount1, 1_200_000.0, EPSILON, "Expected value");
}

#[test]
fn test_quote_amount_conversion_relationship() {
    // Verify: quote_amount = base_amount * spot_rate
    let fx = eurusd_with_notional(1_234_567.0, 1.23456);
    let market = MarketContext::new();
    let result = fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let quote_amt = result.value.amount();
    let spot_rate = fx.spot_rate.unwrap();

    // Base amount is the notional
    let base_amt = fx.notional.amount();

    let expected_quote = base_amt * spot_rate;
    assert_approx_eq(
        quote_amt,
        expected_quote,
        1e-2, // Relaxed tolerance for Decimal arithmetic
        "Conversion relationship",
    );
}
