//! Shared test utilities for portfolio tests.
//!
//! This module provides common fixtures and helpers used across unit tests
//! in the portfolio crate. It is only compiled when running tests.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use time::macros::date;

/// Standard base date used across portfolio tests.
pub fn base_date() -> Date {
    date!(2024 - 01 - 01)
}

/// Build a test market context with a flat USD discount curve.
///
/// The curve uses the standard `base_date()` and has flat discount factors
/// (DF=1.0) to simplify test assertions. Uses `allow_non_monotonic()` since
/// flat curves don't satisfy strict monotonicity.
pub fn build_test_market() -> MarketContext {
    let curve = DiscountCurve::builder("USD")
        .base_date(base_date())
        .knots(vec![(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("test curve should build");

    MarketContext::new().insert(curve)
}

/// Build a test market context with a USD-OIS curve at a specified as-of date.
///
/// This variant allows tests to specify a custom valuation date while still
/// using a simple downward-sloping curve for realistic discounting behavior.
pub fn build_test_market_at(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("test curve should build");

    MarketContext::new().insert(curve)
}
