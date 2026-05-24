//! Shared fixtures and utilities for FX Spot tests.

use finstack_core::HashMap;
use finstack_core::{
    currency::Currency,
    dates::Date,
    market_data::{context::MarketContext, term_structures::DiscountCurve},
    math::interp::InterpStyle,
    money::fx::{FxConversionPolicy, FxMatrix, FxProvider},
    money::Money,
    types::InstrumentId,
};
use finstack_valuations::instruments::fx::fx_spot::FxSpot;
use std::sync::Arc;
use time::Month;

/// Mock FX provider for testing.
#[derive(Clone)]
pub struct MockFxProvider {
    pub rates: HashMap<(Currency, Currency), f64>,
}

impl MockFxProvider {
    /// Create with standard rates
    pub fn standard() -> Self {
        let mut rates = HashMap::default();
        rates.insert((Currency::EUR, Currency::USD), 1.20);
        rates.insert((Currency::GBP, Currency::USD), 1.40);
        rates.insert((Currency::USD, Currency::JPY), 110.0);
        rates.insert((Currency::EUR, Currency::GBP), 1.20 / 1.40);
        Self { rates }
    }
}

impl FxProvider for MockFxProvider {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        if let Some(&rate) = self.rates.get(&(from, to)) {
            return Ok(rate);
        }
        // Try inverse
        if let Some(&rate) = self.rates.get(&(to, from)) {
            return Ok(1.0 / rate);
        }
        Err(finstack_core::Error::internal(
            "test FX provider missing requested currency pair",
        ))
    }
}

/// Helper to create Date from year, month, day
pub fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

/// Standard test date
pub fn test_date() -> Date {
    d(2025, 1, 15)
}

/// Create a simple EURUSD FX spot
pub fn sample_eurusd() -> FxSpot {
    FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
}

/// Create EURUSD with explicit notional and rate
pub fn eurusd_with_notional(notional: f64, rate: f64) -> FxSpot {
    sample_eurusd()
        .with_notional(Money::new(notional, Currency::EUR))
        .unwrap()
        .with_rate(rate)
        .unwrap()
}

/// Create GBPUSD FX spot
pub fn sample_gbpusd() -> FxSpot {
    FxSpot::new(InstrumentId::new("GBPUSD"), Currency::GBP, Currency::USD)
}

/// Create USDJPY FX spot
pub fn sample_usdjpy() -> FxSpot {
    FxSpot::new(InstrumentId::new("USDJPY"), Currency::USD, Currency::JPY)
}

/// Create a market context with FX matrix
pub fn market_with_fx_matrix() -> MarketContext {
    let provider = MockFxProvider::standard();
    let fx_matrix = FxMatrix::new(Arc::new(provider));

    MarketContext::new().insert_fx(fx_matrix)
}

/// Create market context with discount curves
#[allow(dead_code)]
pub fn market_with_curves() -> MarketContext {
    let as_of = test_date();

    let eur_curve = DiscountCurve::builder("EUR.OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.98)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let usd_curve = DiscountCurve::builder("USD.OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.975)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new().insert(eur_curve).insert(usd_curve)
}

/// Create comprehensive market context with both curves and FX
pub fn market_full() -> MarketContext {
    let as_of = test_date();
    let provider = MockFxProvider::standard();
    let fx_matrix = FxMatrix::new(Arc::new(provider));

    let eur_curve = DiscountCurve::builder("EUR.OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.98)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let usd_curve = DiscountCurve::builder("USD.OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.975)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new()
        .insert_fx(fx_matrix)
        .insert(eur_curve)
        .insert(usd_curve)
}

/// Floating-point comparison tolerance
pub const EPSILON: f64 = 1e-10;
pub const LARGE_EPSILON: f64 = 1e-6;

/// Assert approximately equal with tolerance
pub fn assert_approx_eq(actual: f64, expected: f64, tolerance: f64, msg: &str) {
    let diff = (actual - expected).abs();
    assert!(
        diff < tolerance,
        "{}: expected {}, got {} (diff: {})",
        msg,
        expected,
        actual,
        diff
    );
}
