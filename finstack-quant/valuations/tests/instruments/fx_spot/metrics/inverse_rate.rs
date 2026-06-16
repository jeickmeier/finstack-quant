//! Inverse rate metric tests.

use super::super::common::*;
use finstack_quant_core::{
    currency::Currency, dates::Date, market_data::context::MarketContext, money::Money,
};
use finstack_quant_valuations::{
    instruments::{fx::fx_spot::InverseRateCalculator, FxSpot, Instrument},
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
fn test_inverse_rate_basic() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let mut ctx = create_context(fx, test_date());
    let calc = InverseRateCalculator;

    let inv_rate = calc.calculate(&mut ctx).unwrap();
    let expected = 1.0 / 1.20;
    assert_approx_eq(inv_rate, expected, EPSILON, "Inverse rate");
}

#[test]
fn test_inverse_rate_reciprocal_relationship() {
    // EUR/USD = 1.20, so USD/EUR = 1/1.20 ≈ 0.8333
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let mut ctx = create_context(fx, test_date());
    let calc = InverseRateCalculator;

    let inv_rate = calc.calculate(&mut ctx).unwrap();
    assert_approx_eq(inv_rate, 0.8333333333, LARGE_EPSILON, "Reciprocal");
}

#[test]
fn test_inverse_rate_various_rates() {
    let calc = InverseRateCalculator;
    let rates = vec![0.5, 1.0, 1.2, 1.5, 2.0, 110.0];

    for rate in rates {
        let fx = eurusd_with_notional(1_000_000.0, rate);
        let mut ctx = create_context(fx, test_date());

        let inv_rate = calc.calculate(&mut ctx).unwrap();
        let expected = 1.0 / rate;

        assert_approx_eq(
            inv_rate,
            expected,
            LARGE_EPSILON,
            &format!("Inverse of {}", rate),
        );
    }
}

#[test]
fn test_inverse_rate_zero_notional_errors() {
    // Zero base notional makes the inverse rate ill-defined; the calculator
    // surfaces a Validation error rather than silently returning zero.
    let fx = sample_eurusd()
        .with_notional(Money::new(0.0, Currency::EUR))
        .unwrap()
        .with_rate(1.20)
        .expect("test rate");
    let mut ctx = create_context(fx, test_date());
    let calc = InverseRateCalculator;

    let err = calc
        .calculate(&mut ctx)
        .expect_err("zero notional should not silently return 0");
    let msg = err.to_string();
    assert!(
        msg.contains("base notional"),
        "expected base-notional error, got: {msg}"
    );
}

#[test]
fn test_inverse_rate_unity() {
    // Rate of 1.0 has inverse of 1.0
    let fx = eurusd_with_notional(1_000_000.0, 1.0);
    let mut ctx = create_context(fx, test_date());
    let calc = InverseRateCalculator;

    let inv_rate = calc.calculate(&mut ctx).unwrap();
    assert_approx_eq(inv_rate, 1.0, EPSILON, "Inverse of unity");
}

#[test]
fn test_inverse_rate_symmetry() {
    // If EUR/USD = r, then USD/EUR = 1/r
    // And inverse(inverse(r)) = r
    use finstack_quant_valuations::instruments::fx::fx_spot::SpotRateCalculator;

    let fx = eurusd_with_notional(1_000_000.0, 1.25);
    let mut ctx = create_context(fx, test_date());

    let spot_calc = SpotRateCalculator;
    let inv_calc = InverseRateCalculator;

    let spot_rate = spot_calc.calculate(&mut ctx).unwrap();
    let inv_rate = inv_calc.calculate(&mut ctx).unwrap();

    // inverse(spot) should equal inv_rate
    assert_approx_eq(inv_rate, 1.0 / spot_rate, EPSILON, "Inverse symmetry");

    // inverse(inverse(spot)) should equal spot
    assert_approx_eq(1.0 / inv_rate, spot_rate, EPSILON, "Double inverse");
}

#[test]
fn test_inverse_rate_large_rate() {
    // USD/JPY = 110.0, so JPY/USD = 1/110 ≈ 0.009091
    let fx = sample_usdjpy()
        .with_notional(Money::new(100_000.0, Currency::USD))
        .unwrap()
        .with_rate(110.0)
        .expect("test rate");
    let mut ctx = create_context(fx, test_date());
    let calc = InverseRateCalculator;

    let inv_rate = calc.calculate(&mut ctx).unwrap();
    assert_approx_eq(inv_rate, 1.0 / 110.0, LARGE_EPSILON, "Large rate inverse");
}

#[test]
fn test_inverse_rate_small_rate() {
    // Small rate (less than 1)
    let fx = eurusd_with_notional(1_000_000.0, 0.5);
    let mut ctx = create_context(fx, test_date());
    let calc = InverseRateCalculator;

    let inv_rate = calc.calculate(&mut ctx).unwrap();
    assert_approx_eq(inv_rate, 2.0, EPSILON, "Small rate inverse");
}

#[test]
fn test_inverse_rate_independent_of_notional_size() {
    let calc = InverseRateCalculator;
    let rate = 1.25;
    let expected_inv = 1.0 / rate;

    let notionals = vec![1.0, 1000.0, 1_000_000.0, 1_000_000_000.0];

    for notional in notionals {
        let fx = eurusd_with_notional(notional, rate);
        let mut ctx = create_context(fx, test_date());

        let inv_rate = calc.calculate(&mut ctx).unwrap();
        assert_approx_eq(
            inv_rate,
            expected_inv,
            LARGE_EPSILON,
            &format!("Inverse for notional {}", notional),
        );
    }
}

#[test]
fn test_inverse_rate_independent_of_date() {
    let calc = InverseRateCalculator;
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let expected_inv = 1.0 / 1.20;

    let dates = vec![d(2025, 1, 15), d(2025, 6, 15), d(2026, 1, 15)];

    for date in dates {
        let mut ctx = create_context(fx.clone(), date);
        let inv_rate = calc.calculate(&mut ctx).unwrap();

        assert_approx_eq(
            inv_rate,
            expected_inv,
            EPSILON,
            &format!("Inverse on {:?}", date),
        );
    }
}

#[test]
fn test_inverse_rate_product_equals_unity() {
    // spot_rate * inverse_rate should equal 1.0
    use finstack_quant_valuations::instruments::fx::fx_spot::SpotRateCalculator;

    let fx = eurusd_with_notional(1_500_000.0, 1.23456);
    let mut ctx = create_context(fx, test_date());

    let spot_calc = SpotRateCalculator;
    let inv_calc = InverseRateCalculator;

    let spot_rate = spot_calc.calculate(&mut ctx).unwrap();
    let inv_rate = inv_calc.calculate(&mut ctx).unwrap();

    let product = spot_rate * inv_rate;
    assert_approx_eq(product, 1.0, EPSILON, "Product equals unity");
}

#[test]
fn test_inverse_rate_fractional_precision() {
    let fx = eurusd_with_notional(1_000_000.0, 1.23456789);
    let mut ctx = create_context(fx, test_date());
    let calc = InverseRateCalculator;

    let inv_rate = calc.calculate(&mut ctx).unwrap();
    let expected = 1.0 / 1.23456789;
    assert_approx_eq(inv_rate, expected, EPSILON, "Fractional precision");
}
