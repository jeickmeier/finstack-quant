//! Common test fixtures and helpers for FX swap tests.
//!
//! Provides reusable market data setups, mock providers, and utility functions
//! to maintain DRY principles across the test suite.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_core::money::Money;
use finstack_core::HashMap;
use finstack_valuations::instruments::fx::fx_swap::FxSwap;
use std::sync::Arc;
use time::Month;

/// Mock FX provider for testing.
///
/// Provides deterministic FX rates for test scenarios without requiring
/// external market data sources.
#[derive(Clone)]
pub struct MockFxProvider {
    pub rates: HashMap<(Currency, Currency), f64>,
}

impl MockFxProvider {
    /// Create a new mock provider with EUR/USD = 1.1
    pub fn default_eurusd() -> Self {
        let mut rates = HashMap::default();
        rates.insert((Currency::EUR, Currency::USD), 1.1);
        Self { rates }
    }

    /// Create a new mock provider with custom rates
    pub fn with_rates(rates: HashMap<(Currency, Currency), f64>) -> Self {
        Self { rates }
    }

    /// Add a rate to the provider
    #[allow(dead_code)]
    pub fn add_rate(&mut self, from: Currency, to: Currency, rate: f64) {
        self.rates.insert((from, to), rate);
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
        Err(finstack_core::Error::internal(
            "test FX provider missing requested currency pair",
        ))
    }
}

/// Standard test dates for consistent scenarios
pub struct TestDates {
    pub as_of: Date,
    #[allow(dead_code)]
    pub spot_date: Date,
    pub near_date: Date,
    pub far_date_1m: Date,
    pub far_date_3m: Date,
    pub far_date_1y: Date,
}

impl TestDates {
    /// Create standard test dates starting from 2024-01-01
    pub fn standard() -> Self {
        Self {
            as_of: Date::from_calendar_date(2024, Month::January, 1).unwrap(),
            spot_date: Date::from_calendar_date(2024, Month::January, 3).unwrap(),
            near_date: Date::from_calendar_date(2024, Month::January, 3).unwrap(),
            far_date_1m: Date::from_calendar_date(2024, Month::February, 3).unwrap(),
            far_date_3m: Date::from_calendar_date(2024, Month::April, 3).unwrap(),
            far_date_1y: Date::from_calendar_date(2025, Month::January, 3).unwrap(),
        }
    }
}

/// Setup standard market data with flat curves.
///
/// Creates:
/// - USD-OIS: ~1.0% flat rate (DF from 1.0 to 0.9 over 10 years)
/// - EUR-OIS: ~0.5% flat rate (DF from 1.0 to 0.95 over 10 years)
/// - EUR/USD spot: 1.1
pub fn setup_standard_market(as_of: Date) -> MarketContext {
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.9)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let provider = MockFxProvider::default_eurusd();
    let fx_matrix = FxMatrix::new(Arc::new(provider));

    MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_fx(fx_matrix)
}

/// Setup market data with steep curves for sensitivity testing.
///
/// Creates:
/// - USD-OIS: ~5.0% flat rate
/// - EUR-OIS: ~3.0% flat rate
/// - EUR/USD spot: 1.2
pub fn setup_steep_curve_market(as_of: Date) -> MarketContext {
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.95), (10.0, 0.60)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.97), (10.0, 0.74)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let mut rates = HashMap::default();
    rates.insert((Currency::EUR, Currency::USD), 1.2);
    let provider = MockFxProvider::with_rates(rates);
    let fx_matrix = FxMatrix::new(Arc::new(provider));

    MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_fx(fx_matrix)
}

/// Setup market with inverted yield curves (for stress testing).
pub fn setup_inverted_curve_market(as_of: Date) -> MarketContext {
    // Inverted curves (negative rates) require allow_non_monotonic()
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (0.25, 0.99), (10.0, 1.05)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        ) // DFs increase for negative rates
        .build()
        .unwrap();

    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (0.25, 0.995), (10.0, 1.02)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        ) // DFs increase for negative rates
        .build()
        .unwrap();

    let provider = MockFxProvider::default_eurusd();
    let fx_matrix = FxMatrix::new(Arc::new(provider));

    MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_fx(fx_matrix)
}

/// Create a standard EUR/USD FX swap for testing.
pub fn create_standard_fx_swap(id: &str, near_date: Date, far_date: Date, notional: f64) -> FxSwap {
    FxSwap::builder()
        .id(id.to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(near_date)
        .far_date(far_date)
        .base_notional(Money::new(notional, Currency::EUR))
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .build()
        .unwrap()
}

/// Create an FX swap with explicit contract rates.
pub fn create_fx_swap_with_rates(
    id: &str,
    near_date: Date,
    far_date: Date,
    notional: f64,
    near_rate: f64,
    far_rate: f64,
) -> FxSwap {
    FxSwap::builder()
        .id(id.to_string().into())
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .near_date(near_date)
        .far_date(far_date)
        .base_notional(Money::new(notional, Currency::EUR))
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id("EUR-OIS".into())
        .near_rate(near_rate)
        .far_rate(far_rate)
        .build()
        .unwrap()
}

/// Assert that a value is approximately equal within tolerance.
pub fn assert_approx_eq(actual: f64, expected: f64, tolerance: f64, msg: &str) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tolerance,
        "{}: expected {}, got {} (diff: {})",
        msg,
        expected,
        actual,
        diff
    );
}

/// Assert that a value is within a relative percentage range.
pub fn assert_within_pct(actual: f64, expected: f64, pct: f64, msg: &str) {
    let tolerance = expected.abs() * pct / 100.0;
    assert_approx_eq(actual, expected, tolerance, msg);
}
