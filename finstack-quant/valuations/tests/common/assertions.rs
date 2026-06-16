//! Custom assertion helpers for risk tests.
//!
//! These helpers provide better error messages and consistent tolerance handling.

use super::tolerances;

/// Assert that two f64 values are approximately equal within an absolute tolerance.
///
/// Uses `#[track_caller]` to report the actual test location on failure.
///
/// # Panics
///
/// Panics if `|actual - expected| > tolerance`.
#[track_caller]
pub fn assert_approx_eq(actual: f64, expected: f64, tolerance: f64) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tolerance,
        "assertion failed: values not approximately equal\n  \
         expected: {expected}\n  \
         actual:   {actual}\n  \
         diff:     {diff}\n  \
         tol:      {tolerance}"
    );
}

/// Assert that two f64 values are approximately equal within the standard tolerance.
///
/// Uses `tolerances::STANDARD` (1e-6) as the default tolerance.
#[track_caller]
pub fn assert_approx_eq_std(actual: f64, expected: f64) {
    assert_approx_eq(actual, expected, tolerances::STANDARD);
}

/// Assert that two f64 values are approximately equal within a relative tolerance.
///
/// The comparison is: `|actual - expected| / |expected| <= rel_tolerance`
/// Falls back to absolute comparison if `expected` is near zero.
///
/// # Panics
///
/// Panics if the relative difference exceeds `rel_tolerance`.
#[track_caller]
pub fn assert_relative_eq(actual: f64, expected: f64, rel_tolerance: f64) {
    if expected.abs() < tolerances::NEAR_ZERO {
        // Use a near-zero scaled absolute tolerance so caller intent is preserved.
        let abs_tolerance = rel_tolerance * tolerances::NEAR_ZERO;
        assert_approx_eq(actual, expected, abs_tolerance);
    } else {
        let rel_diff = ((actual - expected) / expected).abs();
        assert!(
            rel_diff <= rel_tolerance,
            "assertion failed: relative difference too large\n  \
             expected:  {expected}\n  \
             actual:    {actual}\n  \
             rel_diff:  {:.4}%\n  \
             tolerance: {:.4}%",
            rel_diff * 100.0,
            rel_tolerance * 100.0
        );
    }
}

/// Assert that a value is positive.
#[track_caller]
pub fn assert_positive(value: f64, name: &str) {
    assert!(
        value > 0.0,
        "assertion failed: {name} should be positive, got {value}"
    );
}

/// Assert that a value is negative.
#[track_caller]
pub fn assert_negative(value: f64, name: &str) {
    assert!(
        value < 0.0,
        "assertion failed: {name} should be negative, got {value}"
    );
}

/// Assert that a value is non-negative.
#[track_caller]
pub fn assert_non_negative(value: f64, name: &str) {
    assert!(
        value >= 0.0,
        "assertion failed: {name} should be non-negative, got {value}"
    );
}

/// Assert that a value is finite (not NaN or infinity).
#[track_caller]
pub fn assert_finite(value: f64, name: &str) {
    assert!(
        value.is_finite(),
        "assertion failed: {name} should be finite, got {value}"
    );
}

/// Assert that a value is within a given range [min, max].
#[track_caller]
pub fn assert_in_range(value: f64, min: f64, max: f64, name: &str) {
    assert!(
        (min..=max).contains(&value),
        "assertion failed: {name} should be in range [{min}, {max}], got {value}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // assert_approx_eq tests
    // =========================================================================

    #[test]
    fn test_assert_approx_eq_passes() {
        assert_approx_eq(1.0, 1.0 + 1e-11, tolerances::TIGHT);
    }

    #[test]
    fn test_assert_approx_eq_exact_match() {
        assert_approx_eq(42.0, 42.0, tolerances::TIGHT);
    }

    #[test]
    fn test_assert_approx_eq_at_tolerance_boundary() {
        // Exactly at tolerance should pass
        assert_approx_eq(1.0, 1.0 + tolerances::STANDARD, tolerances::STANDARD);
    }

    #[test]
    #[should_panic(expected = "values not approximately equal")]
    fn test_assert_approx_eq_fails() {
        assert_approx_eq(1.0, 2.0, tolerances::TIGHT);
    }

    // =========================================================================
    // assert_approx_eq_std tests
    // =========================================================================

    #[test]
    fn test_assert_approx_eq_std_passes() {
        assert_approx_eq_std(1.0, 1.0 + 1e-7);
    }

    #[test]
    #[should_panic(expected = "values not approximately equal")]
    fn test_assert_approx_eq_std_fails_beyond_standard_tolerance() {
        // STANDARD tolerance is 1e-6, so 1e-5 difference should fail
        assert_approx_eq_std(1.0, 1.0 + 1e-5);
    }

    // =========================================================================
    // assert_relative_eq tests
    // =========================================================================

    #[test]
    fn test_assert_relative_eq_passes() {
        assert_relative_eq(100.5, 100.0, tolerances::PERCENT_1);
    }

    #[test]
    fn test_assert_relative_eq_near_zero_falls_back_to_absolute() {
        // When expected is near zero, should use absolute tolerance
        assert_relative_eq(5e-11, 0.0, tolerances::PERCENT_1);
    }

    #[test]
    #[should_panic(expected = "relative difference too large")]
    fn test_assert_relative_eq_fails() {
        assert_relative_eq(110.0, 100.0, tolerances::PERCENT_1);
    }

    #[test]
    #[should_panic(expected = "values not approximately equal")]
    fn test_assert_relative_eq_near_zero_fails_when_outside_scaled_abs_tol() {
        // NEAR_ZERO=1e-8, rel_tol=1% => abs_tol=1e-10
        assert_relative_eq(2e-10, 0.0, tolerances::PERCENT_1);
    }

    // =========================================================================
    // assert_positive tests
    // =========================================================================

    #[test]
    fn test_assert_positive_passes() {
        assert_positive(0.001, "small positive");
        assert_positive(1_000_000.0, "large positive");
        assert_positive(f64::MIN_POSITIVE, "min positive");
    }

    #[test]
    #[should_panic(expected = "should be positive")]
    fn test_assert_positive_fails_for_zero() {
        assert_positive(0.0, "zero");
    }

    #[test]
    #[should_panic(expected = "should be positive")]
    fn test_assert_positive_fails_for_negative() {
        assert_positive(-1.0, "negative");
    }

    // =========================================================================
    // assert_negative tests
    // =========================================================================

    #[test]
    fn test_assert_negative_passes() {
        assert_negative(-0.001, "small negative");
        assert_negative(-1_000_000.0, "large negative");
        assert_negative(f64::MIN, "min f64");
    }

    #[test]
    #[should_panic(expected = "should be negative")]
    fn test_assert_negative_fails_for_zero() {
        assert_negative(0.0, "zero");
    }

    #[test]
    #[should_panic(expected = "should be negative")]
    fn test_assert_negative_fails_for_positive() {
        assert_negative(1.0, "positive");
    }

    // =========================================================================
    // assert_non_negative tests
    // =========================================================================

    #[test]
    fn test_assert_non_negative_passes_for_positive() {
        assert_non_negative(1.0, "positive");
    }

    #[test]
    fn test_assert_non_negative_passes_for_zero() {
        assert_non_negative(0.0, "zero");
    }

    #[test]
    #[should_panic(expected = "should be non-negative")]
    fn test_assert_non_negative_fails_for_negative() {
        assert_non_negative(-0.001, "small negative");
    }

    // =========================================================================
    // assert_finite tests
    // =========================================================================

    #[test]
    fn test_assert_finite_passes_for_normal_values() {
        assert_finite(0.0, "zero");
        assert_finite(1.0, "one");
        assert_finite(-1.0, "negative one");
        assert_finite(f64::MAX, "max f64");
        assert_finite(f64::MIN, "min f64");
        assert_finite(f64::MIN_POSITIVE, "min positive");
    }

    #[test]
    #[should_panic(expected = "should be finite")]
    fn test_assert_finite_fails_for_nan() {
        assert_finite(f64::NAN, "NaN");
    }

    #[test]
    #[should_panic(expected = "should be finite")]
    fn test_assert_finite_fails_for_positive_infinity() {
        assert_finite(f64::INFINITY, "positive infinity");
    }

    #[test]
    #[should_panic(expected = "should be finite")]
    fn test_assert_finite_fails_for_negative_infinity() {
        assert_finite(f64::NEG_INFINITY, "negative infinity");
    }

    // =========================================================================
    // assert_in_range tests
    // =========================================================================

    #[test]
    fn test_assert_in_range_passes_for_value_in_range() {
        assert_in_range(5.0, 0.0, 10.0, "mid range");
    }

    #[test]
    fn test_assert_in_range_passes_at_lower_bound() {
        assert_in_range(0.0, 0.0, 10.0, "at lower bound");
    }

    #[test]
    fn test_assert_in_range_passes_at_upper_bound() {
        assert_in_range(10.0, 0.0, 10.0, "at upper bound");
    }

    #[test]
    #[should_panic(expected = "should be in range")]
    fn test_assert_in_range_fails_below_range() {
        assert_in_range(-0.001, 0.0, 10.0, "below range");
    }

    #[test]
    #[should_panic(expected = "should be in range")]
    fn test_assert_in_range_fails_above_range() {
        assert_in_range(10.001, 0.0, 10.0, "above range");
    }

    #[test]
    fn test_assert_in_range_negative_bounds() {
        assert_in_range(-5.0, -10.0, 0.0, "negative range");
    }
}
