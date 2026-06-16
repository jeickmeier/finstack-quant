//! Common test utilities and constants.
//!
//! This module provides shared testing infrastructure including tolerance constants,
//! helper functions, and fixtures used across multiple test files.

#![allow(dead_code)]

// ============================================================================
// Tolerance Constants
// ============================================================================
// These constants define acceptable error margins when comparing floating-point
// results against expected values from external references or theoretical calculations.

/// Excel double precision limit (1e-8).
///
/// Microsoft Excel stores numbers as IEEE 754 double-precision floating-point,
/// which has approximately 15-17 significant decimal digits of precision.
/// For financial functions like NPV, IRR, PMT, we use this tolerance when
/// comparing against Excel-generated results.
///
/// **Source:** Excel 365 (Version 16.80, November 2024)
/// **Use cases:** NPV, IRR, PMT, FV, PV calculations
pub const EXCEL_TOLERANCE: f64 = 1e-8;

/// pandas default float64 precision (1e-10).
///
/// pandas uses NumPy float64 for numerical operations, which typically maintains
/// precision to about 1e-15, but we use a more conservative 1e-10 to account for
/// algorithmic differences and accumulated rounding.
///
/// **Source:** pandas 2.1.3, NumPy 1.26
/// **Use cases:** DataFrame operations, time-series functions, aggregations
pub const PANDAS_TOLERANCE: f64 = 1e-10;

/// Statistical calculation tolerance (1e-3).
///
/// Sample variance and standard deviation calculations can vary slightly depending
/// on the algorithm used (one-pass vs two-pass, Welford's method, etc.). This
/// tolerance accounts for these numerical differences while still catching
/// meaningful errors.
///
/// **Use cases:** variance, std, rolling statistics with ddof=1
pub const SAMPLE_VAR_TOLERANCE: f64 = 1e-3;

/// Bond pricing tolerance - basis points (0.0001 = 1 bp).
///
/// Market convention for bond pricing typically quotes to 1/32nd for Treasuries
/// and 1/100th for corporates. We use 1 basis point as the tolerance for
/// yield-to-maturity and yield-to-worst calculations.
///
/// **Use cases:** YTM, YTW, bond prices, spreads
pub const BOND_PRICING_BP_TOLERANCE: f64 = 0.0001; // 1 basis point

/// General numerical precision for deterministic calculations (1e-12).
///
/// For calculations that should be exactly reproducible (no statistical variance,
/// no external dependencies), we use tight tolerance to catch implementation bugs.
///
/// **Use cases:** Compound growth, TTM sums, deterministic forecasts
pub const DETERMINISTIC_TOLERANCE: f64 = 1e-12;

/// Floating-point epsilon for near-zero comparisons (1e-15).
///
/// Used when checking if a value is effectively zero to avoid false positives
/// from floating-point rounding.
pub const EPSILON: f64 = 1e-15;

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Assert that two f64 values are approximately equal within a tolerance.
///
/// # Arguments
/// * `actual` - The computed value
/// * `expected` - The expected value
/// * `tolerance` - Maximum allowed absolute difference
/// * `message` - Context message for assertion failure
///
/// # Panics
/// Panics if the absolute difference exceeds the tolerance.
pub fn assert_close(actual: f64, expected: f64, tolerance: f64, message: &str) {
    let diff = (actual - expected).abs();
    assert!(
        diff < tolerance,
        "{}: expected {}, got {} (diff: {}, tolerance: {})",
        message,
        expected,
        actual,
        diff,
        tolerance
    );
}

/// Assert that a value is effectively zero (within EPSILON).
pub fn assert_near_zero(value: f64, message: &str) {
    assert_close(value, 0.0, EPSILON, message);
}

/// Assert that two vectors are element-wise close within a tolerance.
pub fn assert_vec_close(actual: &[f64], expected: &[f64], tolerance: f64, message: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{}: vector lengths differ ({} vs {})",
        message,
        actual.len(),
        expected.len()
    );

    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        let diff = (a - e).abs();
        assert!(
            diff < tolerance,
            "{} [element {}]: expected {}, got {} (diff: {}, tolerance: {})",
            message,
            i,
            e,
            a,
            diff,
            tolerance
        );
    }
}
