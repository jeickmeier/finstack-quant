//! Shared test fixtures and helpers for finstack_quant_core integration tests.
//!
//! This module provides common utilities used across multiple test modules.
//! Module-specific helpers should be placed in their respective `test_helpers.rs`
//! or `common.rs` files within each test subdirectory.
//!
//! # Date Helpers
//!
//! - [`test_date`]: Standard test date (2025-01-15)
//! - [`sample_base_date`]: Base date for market data tests (2024-01-01)
//! - [`make_date`]: Create arbitrary dates from components
//!
//! # Comparison Helpers
//!
//! - [`approx_eq`]: Approximate equality for f64 comparisons

use finstack_quant_core::dates::Date;
use time::Month;

/// Standard test date: 2025-01-15 (used across serde/market_data tests)
pub fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

/// Base date for market data tests: 2024-01-01
pub fn sample_base_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

/// Create arbitrary date from components
pub fn make_date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

/// Approximate equality for f64 comparisons
#[inline]
pub fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol.max(1e-15)
}
