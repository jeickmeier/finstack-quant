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
/// * `config` - Relative and near-zero absolute tolerance configuration.
///
/// # Returns
///
/// Comparison diagnostics and whether the configured tolerance was met.
pub fn compare_values(
    finstack_value: f64,
    reference_value: f64,
    config: ParityConfig,
) -> ParityResult {
    let difference = (finstack_value - reference_value).abs();
    let relative_diff = if reference_value.abs() > config.absolute_tolerance {
        difference / reference_value.abs()
    } else {
        0.0
    };

    let passed = if reference_value.abs() < config.absolute_tolerance {
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
macro_rules! assert_parity {
    ($finstack:expr, $reference:expr, $config:expr, $msg:expr) => {{
        let result = $crate::instruments::parity::compare_values($finstack, $reference, $config);
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
}
