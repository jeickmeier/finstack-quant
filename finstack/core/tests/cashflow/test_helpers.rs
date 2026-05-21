//! Shared test utilities for cashflow module tests.
//!
//! This module provides standardized tolerance constants, helper functions,
//! and test curve implementations for cashflow-related tests.
//!
//! # Tolerance Conventions
//!
//! The library uses a tiered tolerance system based on the type of calculation:
//!
//! | Constant | Value | Use Case |
//! |----------|-------|----------|
//! | [`RATE_TOLERANCE`] | 1e-10 | IRR, CPR, SMM, discount factors |
//! | [`FACTOR_TOLERANCE`] | 1e-12 | Year fractions, day count calculations |
//! | [`XIRR_TOLERANCE`] | 1e-6 | XIRR results (matches Excel precision) |
//! | [`financial_tolerance(n)`] | max(n * 1e-8, 0.01) | Money amounts |
//!
//! # Rationale
//!
//! - **RATE_TOLERANCE (1e-10)**: For unitless rates and factors where machine
//!   precision matters. Rates should match to at least 10 decimal places.
//!
//! - **FACTOR_TOLERANCE (1e-12)**: For year fractions where day-count precision
//!   is critical. Year fractions should be exact within floating-point limits.
//!
//! - **XIRR_TOLERANCE (1e-6)**: For XIRR calculations, matching Microsoft Excel's
//!   de facto industry standard precision. This is the expected precision for
//!   performance measurement per GIPS standards.
//!
//! - **financial_tolerance**: For money amounts, scales with notional to avoid
//!   overly tight tolerances for large amounts while ensuring at least $0.01
//!   precision for small amounts.
//!
//! # Curve Conventions
//!
//! Test curves in this module use **continuous compounding**:
//! ```text
//! DF(t) = exp(-rate * t)
//! ```
//!
//! This differs from some market conventions:
//! - **Annual compounding**: DF(t) = 1 / (1 + rate)^t
//! - **Semi-annual compounding**: DF(t) = 1 / (1 + rate/2)^(2t)
//!
//! For tests requiring specific compounding conventions, use the market data
//! curve builders from `finstack_core::market_data::term_structures`.
//!
//! # Examples
//!
//! ```ignore
//! use crate::test_helpers::{XIRR_TOLERANCE, financial_tolerance, FlatRateCurve};
//!
//! // Check XIRR result
//! assert!((result - 0.10).abs() < XIRR_TOLERANCE);
//!
//! // Check PV for $1M notional
//! assert!((pv - 950_000.0).abs() < financial_tolerance(1_000_000.0));
//!
//! // Create flat discount curve
//! let curve = FlatRateCurve::new("USD-OIS", base_date, 0.05);
//! ```
//!
//! # References
//!
//! - Microsoft Excel XIRR function specification (1e-6 precision target)
//! - CFA Institute GIPS Standards (performance measurement precision)
//! - ISDA 2006 Definitions (day count conventions)

use finstack_core::dates::Date;
use finstack_core::market_data::traits::{Discounting, TermStructure};
use finstack_core::types::CurveId;

// =============================================================================
// Tolerance Constants
// =============================================================================

/// Tolerance for rate and factor comparisons (e.g., IRR, CPR, SMM, discount factors).
///
/// Use this for unitless quantities where machine precision is expected.
#[allow(dead_code)]
pub const RATE_TOLERANCE: f64 = 1e-10;

/// Tolerance for year fraction comparisons.
///
/// Use this for day count calculations where exact fractions are expected.
#[allow(dead_code)]
pub const FACTOR_TOLERANCE: f64 = 1e-12;

/// Tolerance for XIRR/IRR result comparisons.
///
/// Matches Excel XIRR precision and GIPS performance measurement standards.
#[allow(dead_code)]
pub const XIRR_TOLERANCE: f64 = 1e-6;

/// Convenience assertion for floating-point comparisons used across cashflow tests.
#[allow(dead_code)]
pub fn assert_close(actual: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{}: expected {:.12}, got {:.12}",
        label,
        expected,
        actual
    );
}

/// Calculate appropriate tolerance for financial amounts based on notional.
///
/// Returns `max(notional * 1e-8, 0.01)` to ensure reasonable precision
/// while avoiding overly tight tolerances for small amounts.
///
/// # Examples
///
/// ```ignore
/// // $1M notional → $0.01 tolerance
/// assert_eq!(financial_tolerance(1_000_000.0), 0.01);
///
/// // $100 notional → $0.01 tolerance (minimum)
/// assert_eq!(financial_tolerance(100.0), 0.01);
///
/// // $10B notional → $100 tolerance
/// assert_eq!(financial_tolerance(10_000_000_000.0), 100.0);
/// ```
#[allow(dead_code)]
pub fn financial_tolerance(notional: f64) -> f64 {
    (notional.abs() * 1e-8).max(0.01)
}

// =============================================================================
// Test Curve Implementations
// =============================================================================

/// Flat-rate discount curve with continuous compounding.
///
/// Uses the formula: `DF(t) = exp(-rate * t)`
///
/// # Key Properties
///
/// - `DF(0) = 1.0` (always)
/// - `DF(t)` decreases monotonically for positive rates
/// - `DF(t) > 0` for all t
/// - `DF(t) <= 1.0` for positive rates
/// - For t ≤ 0 (past dates), returns 1.0
///
/// # Example
///
/// ```ignore
/// let curve = FlatRateCurve::new("USD-OIS", base_date, 0.05);
/// let df_1y = curve.df(1.0); // ≈ 0.9512 (exp(-0.05))
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FlatRateCurve {
    /// Curve identifier
    pub id: CurveId,
    /// Base date for the curve
    pub base: Date,
    /// Continuous compounding rate (e.g., 0.05 for 5%)
    pub rate: f64,
}

#[allow(dead_code)]
impl FlatRateCurve {
    /// Create a new flat rate curve.
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier string
    /// * `base` - Base date for discounting
    /// * `rate` - Continuous compounding rate (e.g., 0.05 for 5%)
    pub fn new(id: impl Into<String>, base: Date, rate: f64) -> Self {
        Self {
            id: CurveId::new(id),
            base,
            rate,
        }
    }
}

impl TermStructure for FlatRateCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }
}

impl Discounting for FlatRateCurve {
    fn base_date(&self) -> Date {
        self.base
    }

    fn df(&self, t: f64) -> f64 {
        if t <= 0.0 {
            1.0
        } else {
            (-self.rate * t).exp()
        }
    }
}

// =============================================================================
// Unit Tests for Test Helpers
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date")
    }

    #[test]
    fn flat_rate_curve_df_at_zero_is_one() {
        let curve = FlatRateCurve::new("TEST", test_date(), 0.05);
        assert!((curve.df(0.0) - 1.0).abs() < FACTOR_TOLERANCE);
    }

    #[test]
    fn flat_rate_curve_df_at_negative_time_is_one() {
        let curve = FlatRateCurve::new("TEST", test_date(), 0.05);
        assert!((curve.df(-1.0) - 1.0).abs() < FACTOR_TOLERANCE);
    }

    #[test]
    fn flat_rate_curve_df_decreases_monotonically() {
        let curve = FlatRateCurve::new("TEST", test_date(), 0.05);
        let mut prev_df = f64::MAX;
        for t in [0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0] {
            let df = curve.df(t);
            assert!(df <= prev_df, "DF must decrease: DF({}) = {}", t, df);
            assert!(df > 0.0, "DF must be positive");
            assert!(df <= 1.0, "DF must be <= 1.0 for positive rates");
            prev_df = df;
        }
    }

    #[test]
    fn flat_rate_curve_df_golden_value() {
        let curve = FlatRateCurve::new("TEST", test_date(), 0.05);
        // At t=1, DF = exp(-0.05) ≈ 0.9512
        let expected = (-0.05_f64).exp();
        assert!(
            (curve.df(1.0) - expected).abs() < FACTOR_TOLERANCE,
            "DF(1) should be {}, got {}",
            expected,
            curve.df(1.0)
        );
    }

    #[test]
    fn financial_tolerance_scales_with_notional() {
        // Min is 0.01
        assert!((financial_tolerance(100.0) - 0.01).abs() < 1e-10);
        assert!((financial_tolerance(1_000_000.0) - 0.01).abs() < 1e-10);
        // Scales above threshold
        assert!((financial_tolerance(10_000_000_000.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn tolerance_constants_are_reasonable() {
        // Use const assertions at compile time rather than runtime assertions
        const _: () = {
            assert!(RATE_TOLERANCE < 1e-8);
            assert!(FACTOR_TOLERANCE < RATE_TOLERANCE);
            assert!(XIRR_TOLERANCE > RATE_TOLERANCE);
        };
    }
}
