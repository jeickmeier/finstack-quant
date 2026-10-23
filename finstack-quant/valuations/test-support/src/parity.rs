//! Numeric parity helpers for instrument integration tests.
//!
//! The default configuration allows a 0.01% relative difference and falls
//! back to an absolute tolerance near zero. Use [`ParityConfig::tight`] for
//! higher-precision comparisons or [`ParityConfig::with_relative_tolerance`]
//! when a reference source requires a documented custom band.

use std::fmt;

/// Relative and near-zero absolute tolerances for a parity comparison.
#[derive(Debug, Clone, Copy)]
pub struct ParityConfig {
    /// Maximum relative difference as a decimal fraction.
    pub relative_tolerance: f64,
    /// Maximum absolute difference when the reference value is near zero.
    pub absolute_tolerance: f64,
}

impl Default for ParityConfig {
    fn default() -> Self {
        Self {
            relative_tolerance: 0.0001,
            absolute_tolerance: 1e-8,
        }
    }
}

impl ParityConfig {
    /// Create a configuration with a caller-supplied relative tolerance.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Maximum relative difference as a decimal fraction.
    pub fn with_relative_tolerance(tolerance: f64) -> Self {
        Self {
            relative_tolerance: tolerance,
            absolute_tolerance: 1e-8,
        }
    }

    /// Return the high-precision configuration used by analytical comparisons.
    pub fn tight() -> Self {
        Self {
            relative_tolerance: 0.00001,
            absolute_tolerance: 1e-10,
        }
    }
}

/// Diagnostic result of comparing a calculated value with a reference value.
#[derive(Debug)]
pub struct ParityResult {
    /// Whether the values are within the configured tolerance.
    pub passed: bool,
    /// Value calculated by finstack-quant.
    pub finstack_value: f64,
    /// Reference value from the external implementation or formula.
    pub reference_value: f64,
    /// Absolute difference between the two values.
    pub difference: f64,
    /// Difference divided by the absolute reference value.
    pub relative_diff: f64,
}

impl fmt::Display for ParityResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ParityResult {{ passed: {}, finstack: {:.10}, reference: {:.10}, \
             diff: {:.2e}, rel_diff: {:.4}% }}",
            self.passed,
            self.finstack_value,
            self.reference_value,
            self.difference,
            self.relative_diff * 100.0
        )
    }
}

/// Compare a calculated value with a reference value.
///
/// # Arguments
///
/// * `finstack_value` - Value calculated by finstack-quant.
/// * `reference_value` - Value supplied by the reference implementation.
/// * `config` - Finite, nonnegative relative and near-zero absolute tolerances.
///   References whose absolute value is at most the absolute tolerance use
///   the absolute comparison; larger references use the relative comparison.
///
/// # Returns
///
/// Comparison diagnostics and whether the configured tolerance was met.
/// Nonfinite values or invalid tolerances always fail. Relative diagnostics
/// remain independent of the selected comparison; at a zero reference, an
/// exact match reports zero and any nonzero difference reports infinity.
pub fn compare_values(
    finstack_value: f64,
    reference_value: f64,
    config: ParityConfig,
) -> ParityResult {
    let difference = (finstack_value - reference_value).abs();
    let relative_diff = if reference_value == 0.0 && difference == 0.0 {
        0.0
    } else {
        difference / reference_value.abs()
    };

    let valid = finstack_value.is_finite()
        && reference_value.is_finite()
        && config.absolute_tolerance.is_finite()
        && config.absolute_tolerance >= 0.0
        && config.relative_tolerance.is_finite()
        && config.relative_tolerance >= 0.0;
    let passed = valid
        && if reference_value.abs() <= config.absolute_tolerance {
            difference <= config.absolute_tolerance
        } else {
            relative_diff <= config.relative_tolerance
        };

    ParityResult {
        passed,
        finstack_value,
        reference_value,
        difference,
        relative_diff,
    }
}

/// Assert parity using an explicit tolerance configuration and description.
///
/// Expands to a [`ParityResult`] so callers can inspect the diagnostics.
#[macro_export]
macro_rules! assert_parity {
    ($finstack:expr, $reference:expr, $config:expr, $msg:expr) => {{
        let result = $crate::parity::compare_values($finstack, $reference, $config);
        assert!(
            result.passed,
            "Parity check failed for '{}': {}",
            $msg, result
        );
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tolerance_accepts_and_rejects_expected_differences() {
        let exact = compare_values(100.0, 100.0, ParityConfig::default());
        assert!(exact.passed);
        assert_eq!(exact.difference, 0.0);

        assert!(compare_values(100.005, 100.0, ParityConfig::default()).passed);
        assert!(!compare_values(100.02, 100.0, ParityConfig::default()).passed);
    }

    #[test]
    fn near_zero_references_use_absolute_tolerance() {
        assert!(compare_values(1e-9, 0.0, ParityConfig::default()).passed);
        assert!(!compare_values(2e-8, 0.0, ParityConfig::default()).passed);
    }

    #[test]
    fn tight_tolerance_is_stricter_than_default() {
        let config = ParityConfig::tight();
        assert!(compare_values(100.0005, 100.0, config).passed);
        assert!(!compare_values(100.002, 100.0, config).passed);
    }

    #[test]
    fn custom_relative_tolerance_is_applied() {
        let config = ParityConfig::with_relative_tolerance(0.0005);
        assert!(compare_values(100.04, 100.0, config).passed);
        assert!(!compare_values(100.06, 100.0, config).passed);
    }

    #[test]
    fn result_display_includes_comparison_diagnostics() {
        let display = compare_values(100.005, 100.0, ParityConfig::default()).to_string();
        assert!(display.contains("passed: true"));
        assert!(display.contains("finstack:"));
        assert!(display.contains("reference:"));
    }

    #[test]
    fn absolute_boundary_and_adjacent_references_choose_the_correct_tolerance() {
        let config = ParityConfig {
            absolute_tolerance: 1.0,
            relative_tolerance: 0.0,
        };
        let below = f64::from_bits(1.0_f64.to_bits() - 1);
        let above = f64::from_bits(1.0_f64.to_bits() + 1);
        for sign in [-1.0, 1.0] {
            for reference in [below, 1.0] {
                assert!(compare_values(sign * 1.5, sign * reference, config).passed);
            }
            assert!(!compare_values(sign * 1.5, sign * above, config).passed);
            assert!(!compare_values(sign * 1.0, sign * 1e-8, ParityConfig::default()).passed);
        }
    }

    #[test]
    fn relative_diagnostics_are_independent_of_the_absolute_comparison() {
        let config = ParityConfig::default();
        let exact_zero = compare_values(-0.0, 0.0, config);
        assert!(exact_zero.passed);
        assert_eq!(exact_zero.relative_diff, 0.0);
        let nonzero = compare_values(1e-9, 0.0, config);
        assert!(nonzero.passed);
        assert_eq!(nonzero.relative_diff, f64::INFINITY);
        let near_zero = compare_values(2e-9, 1e-9, config);
        assert!(near_zero.passed);
        assert_eq!(near_zero.relative_diff, 1.0);
    }

    #[test]
    fn zero_tolerances_require_an_exact_finite_match() {
        let config = ParityConfig {
            absolute_tolerance: 0.0,
            relative_tolerance: 0.0,
        };
        for value in [-1.0, -0.0, 0.0, 1.0] {
            assert!(compare_values(value, value, config).passed);
        }
        assert!(!compare_values(f64::MIN_POSITIVE, 0.0, config).passed);
        assert!(!compare_values(1.0 + f64::EPSILON, 1.0, config).passed);
    }

    #[test]
    fn nonfinite_values_and_invalid_tolerances_always_fail() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for (calculated, reference) in [(invalid, 1.0), (1.0, invalid), (invalid, invalid)] {
                assert!(!compare_values(calculated, reference, ParityConfig::default()).passed);
            }
        }
        assert!(compare_values(f64::NAN, 0.0, ParityConfig::default())
            .relative_diff
            .is_nan());
        assert!(
            compare_values(f64::INFINITY, f64::INFINITY, ParityConfig::default())
                .difference
                .is_nan()
        );
        for invalid in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for config in [
                ParityConfig {
                    absolute_tolerance: invalid,
                    relative_tolerance: 0.0,
                },
                ParityConfig {
                    absolute_tolerance: 0.0,
                    relative_tolerance: invalid,
                },
            ] {
                assert!(!compare_values(0.0, 0.0, config).passed);
            }
        }
    }
}
