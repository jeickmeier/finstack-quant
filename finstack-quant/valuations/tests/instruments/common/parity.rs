//! Parity Testing Framework
//!
//! Provides common infrastructure for comparing finstack-quant valuations with reference
//! implementations (QuantLib, Bloomberg, analytical formulas) to ensure feature parity.
//!
//! # Reference Sources
//!
//! - QuantLib 1.34 (2024): https://github.com/lballabio/QuantLib/tree/master/test-suite
//! - Bloomberg Terminal functions
//! - Analytical closed-form solutions
//!
//! # Tolerance Configuration
//!
//! Default tolerance is 0.01% relative (1 basis point), which is appropriate for
//! most financial calculations. This accounts for:
//! - Rounding differences between f64 (finstack) and reference double
//! - Minor numerical method differences
//! - Day count convention edge cases
//!
//! Tolerance can be tightened to 6 decimal places for high-precision validation.

use std::fmt;

// =============================================================================
// Tolerance Configuration
// =============================================================================

/// Configuration for parity test tolerance.
///
/// # Examples
///
/// ```rust,ignore
/// use crate::integration::golden::ParityConfig;
///
/// // Default: 0.01% relative tolerance (1 basis point)
/// let config = ParityConfig::default();
///
/// // Tight: 0.001% relative tolerance (0.1 basis points)
/// let config = ParityConfig::tight();
///
/// // Loose: 0.1% relative tolerance (10 basis points)
/// let config = ParityConfig::loose();
///
/// // Custom relative tolerance
/// let config = ParityConfig::with_relative_tolerance(0.0005); // 0.05%
///
/// // Decimal place matching
/// let config = ParityConfig::with_decimal_places(6);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ParityConfig {
    /// Relative tolerance (e.g., 0.0001 for 0.01%)
    pub relative_tolerance: f64,
    /// Absolute tolerance for near-zero values
    pub absolute_tolerance: f64,
    /// Whether to use decimal place matching instead of relative tolerance
    pub use_decimal_places: Option<usize>,
}

impl Default for ParityConfig {
    /// Default configuration: 0.01% relative tolerance (1 basis point)
    fn default() -> Self {
        Self {
            relative_tolerance: 0.0001, // 0.01%
            absolute_tolerance: 1e-8,
            use_decimal_places: None,
        }
    }
}

impl ParityConfig {
    // =========================================================================
    // Named Presets (aligned with test_helpers::tolerances)
    // =========================================================================

    /// Analytical tolerance for closed-form solutions (0.0001% = 0.001 basis points).
    ///
    /// Use for put-call parity, zero-coupon YTM, and other exact calculations.
    pub const ANALYTICAL: Self = Self {
        relative_tolerance: 1e-6,
        absolute_tolerance: 1e-10,
        use_decimal_places: None,
    };

    /// Numerical tolerance for iterative methods (0.01% = 1 basis point).
    ///
    /// Use for tree pricing, Newton-Raphson solvers, and other numerical methods.
    pub const NUMERICAL: Self = Self {
        relative_tolerance: 1e-4,
        absolute_tolerance: 1e-8,
        use_decimal_places: None,
    };

    /// Curve pricing tolerance (0.5% = 50 basis points).
    ///
    /// Use for curve-based valuations where convention differences are expected.
    pub const CURVE_PRICING: Self = Self {
        relative_tolerance: 5e-3,
        absolute_tolerance: 1e-6,
        use_decimal_places: None,
    };

    /// Relative tolerance for proportional comparisons (1% = 100 basis points).
    pub const RELATIVE: Self = Self {
        relative_tolerance: 1e-2,
        absolute_tolerance: 1e-4,
        use_decimal_places: None,
    };

    /// Statistical tolerance for Monte Carlo tests (2% = 200 basis points).
    pub const STATISTICAL: Self = Self {
        relative_tolerance: 2e-2,
        absolute_tolerance: 1e-4,
        use_decimal_places: None,
    };

    // =========================================================================
    // Constructor Methods
    // =========================================================================

    /// Create configuration with specific relative tolerance.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Relative tolerance as decimal (e.g., 0.0001 for 0.01%)
    pub fn with_relative_tolerance(tolerance: f64) -> Self {
        Self {
            relative_tolerance: tolerance,
            absolute_tolerance: 1e-8,
            use_decimal_places: None,
        }
    }

    /// Create configuration with decimal place matching.
    ///
    /// # Arguments
    ///
    /// * `places` - Number of decimal places to match (e.g., 6 for 1e-6 tolerance)
    pub fn with_decimal_places(places: usize) -> Self {
        let tolerance = 10_f64.powi(-(places as i32));
        Self {
            relative_tolerance: 0.0,
            absolute_tolerance: tolerance,
            use_decimal_places: Some(places),
        }
    }

    /// Create configuration with both absolute and relative tolerances.
    ///
    /// # Arguments
    ///
    /// * `abs_tol` - Absolute tolerance
    /// * `rel_tol` - Relative tolerance as decimal
    pub fn with_tolerances(abs_tol: f64, rel_tol: f64) -> Self {
        Self {
            relative_tolerance: rel_tol,
            absolute_tolerance: abs_tol,
            use_decimal_places: None,
        }
    }

    // =========================================================================
    // Convenience Named Constructors
    // =========================================================================

    /// Tight tolerance for high-precision tests (0.001% = 0.1 basis points).
    ///
    /// **Note:** Prefer `ParityConfig::ANALYTICAL` for new code.
    pub fn tight() -> Self {
        Self {
            relative_tolerance: 0.00001, // 0.001%
            absolute_tolerance: 1e-10,
            use_decimal_places: None,
        }
    }

    /// Loose tolerance for tests with known numerical instabilities (0.1% = 10 basis points).
    ///
    /// **Note:** Prefer `ParityConfig::CURVE_PRICING` for new code.
    pub fn loose() -> Self {
        Self {
            relative_tolerance: 0.001, // 0.1%
            absolute_tolerance: 1e-6,
            use_decimal_places: None,
        }
    }

    /// Very loose tolerance for Monte Carlo or highly unstable tests (1% = 100 basis points).
    ///
    /// **Note:** Prefer `ParityConfig::STATISTICAL` for new code.
    pub fn very_loose() -> Self {
        Self {
            relative_tolerance: 0.01, // 1%
            absolute_tolerance: 1e-4,
            use_decimal_places: None,
        }
    }
}

// =============================================================================
// Comparison Results
// =============================================================================

/// Result of a parity comparison.
#[derive(Debug)]
pub struct ParityResult {
    /// Whether the comparison passed within tolerance
    pub passed: bool,
    /// The value from finstack-quant.
    pub finstack_value: f64,
    /// The reference value (QuantLib, Bloomberg, etc.)
    pub reference_value: f64,
    /// Absolute difference
    pub difference: f64,
    /// Relative difference (as decimal)
    pub relative_diff: f64,
    /// Tolerance configuration used
    pub tolerance_used: ParityConfig,
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

// =============================================================================
// Comparison Functions
// =============================================================================

/// Compare two numeric values with configured tolerance.
///
/// # Arguments
///
/// * `finstack_value` - Value from finstack-quant calculation
/// * `reference_value` - Reference value from external source
/// * `config` - Tolerance configuration
///
/// # Returns
///
/// A [`ParityResult`] containing comparison details.
///
/// # Examples
///
/// ```rust,ignore
/// use crate::integration::golden::{compare_values, ParityConfig};
///
/// let result = compare_values(100.005, 100.0, ParityConfig::default());
/// assert!(result.passed); // Within 0.01% tolerance
/// ```
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

    let passed = if config.use_decimal_places.is_some() {
        // Use absolute tolerance based on decimal places
        difference <= config.absolute_tolerance
    } else {
        // Use relative tolerance, falling back to absolute for near-zero values
        if reference_value.abs() < config.absolute_tolerance {
            difference <= config.absolute_tolerance
        } else {
            relative_diff <= config.relative_tolerance
        }
    };

    ParityResult {
        passed,
        finstack_value,
        reference_value,
        difference,
        relative_diff,
        tolerance_used: config,
    }
}

/// Compare values and return whether they match within tolerance.
///
/// Simpler version of [`compare_values`] that just returns a boolean.
pub fn values_match(finstack_value: f64, reference_value: f64, config: ParityConfig) -> bool {
    compare_values(finstack_value, reference_value, config).passed
}

// =============================================================================
// Assertion Functions
// =============================================================================

/// Assert that finstack value matches reference value within tolerance.
///
/// # Arguments
///
/// * `finstack_value` - Value from finstack-quant calculation
/// * `reference_value` - Reference value from external source
/// * `config` - Tolerance configuration
///
/// # Panics
///
/// Panics if values don't match within tolerance.
#[allow(dead_code)]
pub fn assert_parity_fn(
    finstack_value: f64,
    reference_value: f64,
    config: ParityConfig,
) -> ParityResult {
    let result = compare_values(finstack_value, reference_value, config);
    assert!(
        result.passed,
        "Parity check failed:\n  Finstack:  {:.10}\n  Reference: {:.10}\n  \
         Difference: {:.2e}\n  Rel Diff:  {:.4}%\n  Tolerance: {:.4}%",
        result.finstack_value,
        result.reference_value,
        result.difference,
        result.relative_diff * 100.0,
        result.tolerance_used.relative_tolerance * 100.0
    );
    result
}

/// Assert parity with a descriptive message.
#[allow(dead_code)]
pub fn assert_parity_with_msg(
    finstack_value: f64,
    reference_value: f64,
    config: ParityConfig,
    msg: &str,
) -> ParityResult {
    let result = compare_values(finstack_value, reference_value, config);
    assert!(
        result.passed,
        "Parity check failed for '{}': {}",
        msg, result
    );
    result
}

/// Assert parity with verbose output (prints result even on success).
#[allow(dead_code)]
pub fn assert_parity_verbose_fn(
    finstack_value: f64,
    reference_value: f64,
    config: ParityConfig,
) -> ParityResult {
    let result = compare_values(finstack_value, reference_value, config);
    println!(
        "  Parity: finstack={:.6}, reference={:.6}, diff={:.2e}, rel={:.4}%, passed={}",
        result.finstack_value,
        result.reference_value,
        result.difference,
        result.relative_diff * 100.0,
        result.passed
    );
    assert!(
        result.passed,
        "Parity check failed:\n  Finstack:  {:.10}\n  Reference: {:.10}\n  \
         Difference: {:.2e}\n  Rel Diff:  {:.4}%\n  Tolerance: {:.4}%",
        result.finstack_value,
        result.reference_value,
        result.difference,
        result.relative_diff * 100.0,
        result.tolerance_used.relative_tolerance * 100.0
    );
    result
}

// =============================================================================
// Assertion Macros
// =============================================================================
// These macros rely on ParityConfig and compare_values being in scope via `use`.

/// Assert that finstack value matches reference value within tolerance.
///
/// # Usage
///
/// Requires `use crate::parity::*;` or equivalent import in scope.
///
/// ```rust,ignore
/// use crate::parity::*;
///
/// // Default tolerance (0.01%)
/// assert_parity!(calculated_price, reference_price);
///
/// // Custom config
/// assert_parity!(calculated_price, reference_price, ParityConfig::tight());
///
/// // With descriptive message
/// assert_parity!(calculated_price, reference_price, ParityConfig::default(), "Bond PV");
/// ```
#[allow(unused_macros)]
macro_rules! assert_parity {
    ($finstack:expr, $reference:expr) => {
        assert_parity!($finstack, $reference, ParityConfig::default())
    };
    ($finstack:expr, $reference:expr, $config:expr) => {{
        let result = compare_values($finstack, $reference, $config);
        assert!(
            result.passed,
            "Parity check failed:\n  Finstack:  {:.10}\n  Reference: {:.10}\n  \
             Difference: {:.2e}\n  Rel Diff:  {:.4}%\n  Tolerance: {:.4}%",
            result.finstack_value,
            result.reference_value,
            result.difference,
            result.relative_diff * 100.0,
            result.tolerance_used.relative_tolerance * 100.0
        );
        result
    }};
    ($finstack:expr, $reference:expr, $config:expr, $msg:expr) => {{
        let result = compare_values($finstack, $reference, $config);
        assert!(
            result.passed,
            "Parity check failed for '{}': {}",
            $msg, result
        );
        result
    }};
}

/// Assert parity for Decimal types (same as assert_parity but with explicit naming).
#[allow(unused_macros)]
macro_rules! assert_parity_decimal {
    ($finstack:expr, $reference:expr) => {
        assert_parity_decimal!($finstack, $reference, ParityConfig::default())
    };
    ($finstack:expr, $reference:expr, $config:expr) => {{
        let result = compare_values($finstack, $reference, $config);
        assert!(
            result.passed,
            "Parity check failed (Decimal):\n  Finstack: {:.10}\n  Reference: {:.10}\n  \
             Difference: {:.2e}\n  Rel Diff: {:.4}%",
            result.finstack_value,
            result.reference_value,
            result.difference,
            result.relative_diff * 100.0
        );
        result
    }};
}

/// Assert parity with verbose output (prints result even on success).
#[allow(unused_macros)]
macro_rules! assert_parity_verbose {
    ($finstack:expr, $reference:expr) => {
        assert_parity_verbose!($finstack, $reference, ParityConfig::default())
    };
    ($finstack:expr, $reference:expr, $config:expr) => {{
        let result = compare_values($finstack, $reference, $config);
        println!(
            "  Parity: finstack={:.6}, reference={:.6}, diff={:.2e}, rel={:.4}%, passed={}",
            result.finstack_value,
            result.reference_value,
            result.difference,
            result.relative_diff * 100.0,
            result.passed
        );
        assert!(
            result.passed,
            "Parity check failed:\n  Finstack:  {:.10}\n  Reference: {:.10}\n  \
             Difference: {:.2e}\n  Rel Diff:  {:.4}%\n  Tolerance: {:.4}%",
            result.finstack_value,
            result.reference_value,
            result.difference,
            result.relative_diff * 100.0,
            result.tolerance_used.relative_tolerance * 100.0
        );
        result
    }};
}

// Note: Macros are available when using `use crate::parity::*;` from test files

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parity_exact_match() {
        let result = compare_values(100.0, 100.0, ParityConfig::default());
        assert!(result.passed);
        assert_eq!(result.difference, 0.0);
    }

    #[test]
    fn test_parity_within_tolerance() {
        // 0.005% difference, within 0.01% tolerance
        let result = compare_values(100.005, 100.0, ParityConfig::default());
        assert!(result.passed);
    }

    #[test]
    fn test_parity_exceeds_tolerance() {
        // 0.02% difference, exceeds 0.01% tolerance
        let result = compare_values(100.02, 100.0, ParityConfig::default());
        assert!(!result.passed);
    }

    #[test]
    fn test_parity_decimal_places() {
        // Exact to 6 decimal places
        let config = ParityConfig::with_decimal_places(6);
        let result = compare_values(100.0000005, 100.0, config);
        assert!(result.passed);

        // Different at 6 decimal places
        let result = compare_values(100.000005, 100.0, config);
        assert!(!result.passed);
    }

    #[test]
    fn test_parity_near_zero() {
        // Near-zero values use absolute tolerance
        let result = compare_values(1e-9, 0.0, ParityConfig::default());
        assert!(result.passed);
    }

    #[test]
    fn test_tight_tolerance() {
        let config = ParityConfig::tight();
        let result = compare_values(100.0005, 100.0, config);
        assert!(result.passed);

        let result = compare_values(100.002, 100.0, config);
        assert!(!result.passed);
    }

    #[test]
    fn test_loose_tolerance() {
        let config = ParityConfig::loose();
        let result = compare_values(100.05, 100.0, config);
        assert!(result.passed);

        let result = compare_values(100.2, 100.0, config);
        assert!(!result.passed);
    }

    #[test]
    fn test_very_loose_tolerance() {
        let config = ParityConfig::very_loose();
        let result = compare_values(100.5, 100.0, config);
        assert!(result.passed);

        let result = compare_values(102.0, 100.0, config);
        assert!(!result.passed);
    }

    #[test]
    fn test_custom_tolerances() {
        let config = ParityConfig::with_tolerances(0.001, 0.0005);
        let result = compare_values(100.04, 100.0, config);
        assert!(result.passed); // 0.04% < 0.05%
    }

    #[test]
    fn test_values_match_helper() {
        assert!(values_match(100.005, 100.0, ParityConfig::default()));
        assert!(!values_match(100.02, 100.0, ParityConfig::default()));
    }

    #[test]
    fn test_result_display() {
        let result = compare_values(100.005, 100.0, ParityConfig::default());
        let display = format!("{}", result);
        assert!(display.contains("passed: true"));
        assert!(display.contains("finstack:"));
        assert!(display.contains("reference:"));
    }
}
