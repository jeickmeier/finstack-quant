//! Spot rate metric tests.

use super::super::common::*;
use finstack_quant_core::{
    currency::Currency, dates::Date, market_data::context::MarketContext, money::Money,
};
use finstack_quant_valuations::{
    instruments::{fx::fx_spot::SpotRateCalculator, FxSpot, Instrument},
    metrics::{MetricCalculator, MetricContext},
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
fn test_spot_rate_basic() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let mut ctx = create_context(fx, test_date());
    let calc = SpotRateCalculator;

    let rate = calc.calculate(&mut ctx).unwrap();
    assert_approx_eq(rate, 1.20, EPSILON, "Spot rate");
}

#[test]
fn test_spot_rate_derived_from_pv() {
    // spot_rate = quote_amount / base_amount = PV / notional
    let fx = eurusd_with_notional(2_000_000.0, 1.22);
    let mut ctx = create_context(fx, test_date());
    let calc = SpotRateCalculator;

    let rate = calc.calculate(&mut ctx).unwrap();

    // PV = 2M * 1.22 = 2.44M, so rate = 2.44M / 2M = 1.22
    assert_approx_eq(rate, 1.22, EPSILON, "Derived spot rate");
}

#[test]
fn test_spot_rate_default_notional() {
    let fx = sample_eurusd().with_rate(1.18).expect("test rate");
    let mut ctx = create_context(fx, test_date());
    let calc = SpotRateCalculator;

    let rate = calc.calculate(&mut ctx).unwrap();
    assert_approx_eq(rate, 1.18, EPSILON, "Default notional spot rate");
}

#[test]
fn test_spot_rate_zero_notional_errors() {
    let fx = sample_eurusd()
        .with_notional(Money::new(0.0, Currency::EUR))
        .unwrap()
        .with_rate(1.20)
        .expect("test rate");
    let mut ctx = create_context(fx, test_date());
    let calc = SpotRateCalculator;

    let err = calc
        .calculate(&mut ctx)
        .expect_err("spot rate is undefined for zero notional");
    assert!(
        err.to_string().contains("undefined"),
        "error should say spot rate is undefined: {}",
        err
    );
}

#[test]
fn test_spot_rate_various_rates() {
    let calc = SpotRateCalculator;
    let rates = vec![0.5, 0.9, 1.0, 1.2, 1.5, 2.0, 100.0];

    for expected_rate in rates {
        let fx = eurusd_with_notional(1_000_000.0, expected_rate);
        let mut ctx = create_context(fx, test_date());

        let calculated_rate = calc.calculate(&mut ctx).unwrap();
        assert_approx_eq(
            calculated_rate,
            expected_rate,
            EPSILON,
            &format!("Spot rate {}", expected_rate),
        );
    }
}

#[test]
fn test_spot_rate_various_currencies() {
    let calc = SpotRateCalculator;

    // GBPUSD = 1.40
    let gbp_fx = sample_gbpusd()
        .with_notional(Money::new(500_000.0, Currency::GBP))
        .unwrap()
        .with_rate(1.40)
        .expect("test rate");
    let mut gbp_ctx = create_context(gbp_fx, test_date());
    assert_approx_eq(
        calc.calculate(&mut gbp_ctx).unwrap(),
        1.40,
        EPSILON,
        "GBP/USD rate",
    );

    // USDJPY = 110.0
    let jpy_fx = sample_usdjpy()
        .with_notional(Money::new(100_000.0, Currency::USD))
        .unwrap()
        .with_rate(110.0)
        .expect("test rate");
    let mut jpy_ctx = create_context(jpy_fx, test_date());
    assert_approx_eq(
        calc.calculate(&mut jpy_ctx).unwrap(),
        110.0,
        EPSILON,
        "USD/JPY rate",
    );
}

#[test]
fn test_spot_rate_independent_of_notional_size() {
    let calc = SpotRateCalculator;
    let rate = 1.22;

    let notionals = vec![1.0, 1000.0, 1_000_000.0, 1_000_000_000.0];

    for notional in notionals {
        let fx = eurusd_with_notional(notional, rate);
        let mut ctx = create_context(fx, test_date());

        let calculated_rate = calc.calculate(&mut ctx).unwrap();
        assert_approx_eq(
            calculated_rate,
            rate,
            LARGE_EPSILON,
            &format!("Rate for notional {}", notional),
        );
    }
}

#[test]
fn test_spot_rate_independent_of_date() {
    let calc = SpotRateCalculator;
    let fx = eurusd_with_notional(1_000_000.0, 1.20);

    let dates = vec![d(2025, 1, 15), d(2025, 6, 15), d(2026, 1, 15)];

    for date in dates {
        let mut ctx = create_context(fx.clone(), date);
        let rate = calc.calculate(&mut ctx).unwrap();

        assert_approx_eq(rate, 1.20, EPSILON, &format!("Rate on {:?}", date));
    }
}

#[test]
fn test_spot_rate_fractional_values() {
    let fx = eurusd_with_notional(1_234_567.89, 1.23456789);
    let mut ctx = create_context(fx, test_date());
    let calc = SpotRateCalculator;

    let rate = calc.calculate(&mut ctx).unwrap();
    assert_approx_eq(rate, 1.23456789, LARGE_EPSILON, "Fractional rate"); // Relaxed for f64 precision
}

#[test]
fn test_spot_rate_relationship_with_amounts() {
    // Verify: spot_rate = quote_amount / base_amount
    use finstack_quant_valuations::instruments::fx::fx_spot::BaseAmountCalculator;

    let fx = eurusd_with_notional(1_500_000.0, 1.25);
    let mut ctx = create_context(fx.clone(), test_date());
    let market = MarketContext::new();
    let result = fx
        .price_with_metrics(
            &market,
            test_date(),
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let base_calc = BaseAmountCalculator;
    let rate_calc = SpotRateCalculator;

    let base_amt = base_calc.calculate(&mut ctx).unwrap();
    let quote_amt = result.value.amount(); // Quote amount is in result.value
    let spot_rate = rate_calc.calculate(&mut ctx).unwrap();

    let derived_rate = quote_amt / base_amt;
    assert_approx_eq(spot_rate, derived_rate, EPSILON, "Rate relationship");
}
