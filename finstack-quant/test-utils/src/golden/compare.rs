//! Comparison and assertion utilities for golden tests.
//!
//! This module provides assertion helpers that produce actionable error
//! messages including case identifiers, metric labels, and provenance.

use crate::golden::types::{Expectation, ExpectedValue, SuiteMeta, Tolerance};
use finstack_quant_core::error::Error;
use finstack_quant_core::money::Money;
use std::collections::HashMap;

// =============================================================================
// Assertion results
// =============================================================================

/// Result of a golden test comparison.
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Whether the comparison passed.
    pub passed: bool,
    /// Suite ID for context.
    pub suite_id: String,
    /// Case ID for context.
    pub case_id: String,
    /// Metric being compared.
    pub metric: String,
    /// Actual value.
    pub actual: f64,
    /// Expected value (for exact comparisons).
    pub expected: Option<f64>,
    /// Range bounds (for range comparisons).
    pub range: Option<(Option<f64>, Option<f64>)>,
    /// Tolerance used.
    pub tolerance: Option<Tolerance>,
    /// Computed error.
    pub error: Option<f64>,
    /// Error message if failed.
    pub message: Option<String>,
}

impl ComparisonResult {
    /// Create a passing result.
    pub fn pass(
        suite_id: &str,
        case_id: &str,
        metric: &str,
        actual: f64,
        expected: f64,
        tolerance: Option<Tolerance>,
    ) -> Self {
        Self {
            passed: true,
            suite_id: suite_id.to_string(),
            case_id: case_id.to_string(),
            metric: metric.to_string(),
            actual,
            expected: Some(expected),
            range: None,
            tolerance,
            error: tolerance.map(|t| t.compute_error(actual, expected)),
            message: None,
        }
    }

    /// Create a failing result.
    pub fn fail(
        suite_id: &str,
        case_id: &str,
        metric: &str,
        actual: f64,
        expected: f64,
        tolerance: Option<Tolerance>,
        message: String,
    ) -> Self {
        Self {
            passed: false,
            suite_id: suite_id.to_string(),
            case_id: case_id.to_string(),
            metric: metric.to_string(),
            actual,
            expected: Some(expected),
            range: None,
            tolerance,
            error: tolerance.map(|t| t.compute_error(actual, expected)),
            message: Some(message),
        }
    }

    /// Create a failing result for range comparison.
    pub fn fail_range(
        suite_id: &str,
        case_id: &str,
        metric: &str,
        actual: f64,
        min: Option<f64>,
        max: Option<f64>,
        message: String,
    ) -> Self {
        Self {
            passed: false,
            suite_id: suite_id.to_string(),
            case_id: case_id.to_string(),
            metric: metric.to_string(),
            actual,
            expected: None,
            range: Some((min, max)),
            tolerance: None,
            error: None,
            message: Some(message),
        }
    }

    /// Format as an assertion error message.
    pub fn format_error(&self) -> String {
        let mut msg = format!(
            "[{}/{}] {} failed: actual={}",
            self.suite_id, self.case_id, self.metric, self.actual
        );

        if let Some(expected) = self.expected {
            msg.push_str(&format!(", expected={}", expected));
        }

        if let Some((min, max)) = &self.range {
            msg.push_str(&format!(", range=[{:?}, {:?}]", min, max));
        }

        if let Some(tol) = &self.tolerance {
            msg.push_str(&format!(", tolerance={:?}", tol));
        }

        if let Some(err) = self.error {
            msg.push_str(&format!(", error={}", err));
        }

        if let Some(m) = &self.message {
            msg.push_str(&format!(" - {}", m));
        }

        msg
    }
}

// =============================================================================
// Core assertion functions
// =============================================================================

/// Assert that actual is within tolerance of expected.
///
/// Returns Ok(()) if the assertion passes, Err with details if it fails.
///
/// # Arguments
///
/// * `suite_id` - Suite identifier for error context
/// * `case_id` - Case identifier for error context
/// * `metric` - Metric name for error context
/// * `actual` - Actual computed value
/// * `expected` - Expected value with tolerance
pub fn assert_expected_f64(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: f64,
    expected: &Expectation,
) -> Result<(), Error> {
    if expected.is_satisfied(actual) {
        return Ok(());
    }
    let msg = match expected {
        Expectation::Exact {
            value, tolerance, ..
        } => {
            let tol_str = tolerance.map_or(String::new(), |t| {
                format!(
                    ", tolerance={t:?}, error={:.6e}",
                    t.compute_error(actual, *value)
                )
            });
            format!(
                "[{suite_id}/{case_id}] {metric} failed: actual={actual}, expected={value}{tol_str} - value outside tolerance"
            )
        }
        Expectation::Range { min, max, .. } => {
            format!(
                "[{suite_id}/{case_id}] {metric} failed: actual={actual}, range=[{min:?}, {max:?}] - value outside range"
            )
        }
    };
    Err(Error::Validation(msg))
}

/// Assert using ExpectedValue structure (convenience for existing code).
pub fn assert_expected_value(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: f64,
    expected: &ExpectedValue,
) -> Result<(), Error> {
    let expectation = expected.to_expectation();
    assert_expected_f64(suite_id, case_id, metric, actual, &expectation)
}

/// Assert with simple tolerance (convenience function).
pub fn assert_within_tolerance(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: f64,
    expected: f64,
    tolerance: Tolerance,
) -> Result<(), Error> {
    assert_expected_f64(
        suite_id,
        case_id,
        metric,
        actual,
        &Expectation::Exact {
            value: expected,
            tolerance: Some(tolerance),
            notes: None,
        },
    )
}

/// Assert with absolute tolerance (convenience function).
pub fn assert_abs(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: f64,
    expected: f64,
    tolerance: f64,
) -> Result<(), Error> {
    assert_within_tolerance(
        suite_id,
        case_id,
        metric,
        actual,
        expected,
        Tolerance::Abs(tolerance),
    )
}

/// Assert with basis points tolerance (convenience function).
pub fn assert_bp(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: f64,
    expected: f64,
    tolerance_bp: f64,
) -> Result<(), Error> {
    assert_within_tolerance(
        suite_id,
        case_id,
        metric,
        actual,
        expected,
        Tolerance::Bps(tolerance_bp),
    )
}

/// Assert with percentage tolerance (convenience function).
pub fn assert_pct(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: f64,
    expected: f64,
    tolerance_pct: f64,
) -> Result<(), Error> {
    assert_within_tolerance(
        suite_id,
        case_id,
        metric,
        actual,
        expected,
        Tolerance::Pct(tolerance_pct),
    )
}

/// Assert a value is within a range.
pub fn assert_range(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> Result<(), Error> {
    let expectation = Expectation::Range {
        min,
        max,
        notes: None,
    };
    assert_expected_f64(suite_id, case_id, metric, actual, &expectation)
}

// =============================================================================
// Money assertions
// =============================================================================

/// Assert that a Money value is within tolerance of expected.
pub fn assert_money(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: Money,
    expected: Money,
    tolerance: Tolerance,
) -> Result<(), Error> {
    if actual.currency() != expected.currency() {
        return Err(Error::Validation(format!(
            "[{suite_id}/{case_id}] {metric} failed: actual currency={}, expected currency={}",
            actual.currency(),
            expected.currency()
        )));
    }
    assert_within_tolerance(
        suite_id,
        case_id,
        metric,
        actual.amount(),
        expected.amount(),
        tolerance,
    )
}

/// Assert Money amount with absolute tolerance.
pub fn assert_money_abs(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    actual: Money,
    expected: Money,
    tolerance: f64,
) -> Result<(), Error> {
    if actual.currency() != expected.currency() {
        return Err(Error::Validation(format!(
            "[{suite_id}/{case_id}] {metric} failed: actual currency={}, expected currency={}",
            actual.currency(),
            expected.currency()
        )));
    }
    assert_abs(
        suite_id,
        case_id,
        metric,
        actual.amount(),
        expected.amount(),
        tolerance,
    )
}

// =============================================================================
// Map/collection assertions
// =============================================================================

/// Assert all values in a HashMap match expectations.
pub fn assert_map_f64(
    suite_id: &str,
    case_id: &str,
    actual: &HashMap<String, f64>,
    expected: &HashMap<String, ExpectedValue>,
) -> Result<(), Error> {
    let mut errors = Vec::new();

    for (key, exp) in expected {
        if let Some(&act) = actual.get(key) {
            if let Err(e) = assert_expected_value(suite_id, case_id, key, act, exp) {
                errors.push(e.to_string());
            }
        } else {
            errors.push(format!("[{}/{}] missing key: {}", suite_id, case_id, key));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::Validation(errors.join("\n")))
    }
}

/// Assert all values in a nested map match expectations.
///
/// This is useful for statements-style results: `Map<node_id, Map<period, value>>`.
pub fn assert_nested_map_f64<K1, K2>(
    suite_id: &str,
    case_id: &str,
    actual: &HashMap<K1, HashMap<K2, f64>>,
    expected: &HashMap<K1, HashMap<K2, f64>>,
    tolerance: f64,
) -> Result<(), Error>
where
    K1: std::fmt::Display + std::hash::Hash + Eq,
    K2: std::fmt::Display + std::hash::Hash + Eq,
{
    let mut errors = Vec::new();

    for (outer_key, inner_expected) in expected {
        if let Some(inner_actual) = actual.get(outer_key) {
            for (inner_key, &exp_val) in inner_expected {
                if let Some(&act_val) = inner_actual.get(inner_key) {
                    if (act_val - exp_val).abs() > tolerance {
                        errors.push(format!(
                            "[{}/{}] {}/{}: actual={}, expected={}, diff={}",
                            suite_id,
                            case_id,
                            outer_key,
                            inner_key,
                            act_val,
                            exp_val,
                            (act_val - exp_val).abs()
                        ));
                    }
                } else {
                    errors.push(format!(
                        "[{}/{}] missing inner key: {}/{}",
                        suite_id, case_id, outer_key, inner_key
                    ));
                }
            }
        } else {
            errors.push(format!(
                "[{}/{}] missing outer key: {}",
                suite_id, case_id, outer_key
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::Validation(errors.join("\n")))
    }
}

// =============================================================================
// Assertion macros
// =============================================================================

/// Macro for asserting with panic on failure (for use in tests).
///
/// # Usage
///
/// ```ignore
/// use finstack_quant_test_utils::golden_assert;
///
/// golden_assert!(assert_abs("suite", "case", "metric", actual, expected, 0.01));
/// ```
#[macro_export]
macro_rules! golden_assert {
    ($result:expr) => {
        if let Err(e) = $result {
            panic!("Golden test failed: {}", e);
        }
    };
}

/// Macro for asserting and collecting errors (for batch validation).
///
/// # Usage
///
/// ```ignore
/// use finstack_quant_test_utils::golden_check;
///
/// let mut errors = Vec::new();
/// golden_check!(errors, assert_abs("suite", "case", "metric1", a1, e1, 0.01));
/// golden_check!(errors, assert_abs("suite", "case", "metric2", a2, e2, 0.01));
/// assert!(errors.is_empty(), "Failures:\n{}", errors.join("\n"));
/// ```
#[macro_export]
macro_rules! golden_check {
    ($errors:expr, $result:expr) => {
        if let Err(e) = $result {
            $errors.push(e.to_string());
        }
    };
}

// =============================================================================
// Context-aware assertion builder
// =============================================================================

/// Builder for golden test assertions with suite context.
///
/// This provides a cleaner API when making many assertions with the same context.
///
/// # Example
///
/// ```ignore
/// use finstack_quant_test_utils::golden::GoldenAssert;
///
/// let assert = GoldenAssert::new(&suite.meta, "case_123");
/// assert.abs("price", actual_price, 100.0, 0.01)?;
/// assert.pct("spread", actual_spread, 50.0, 0.1)?;
/// ```
pub struct GoldenAssert<'a> {
    suite_id: &'a str,
    case_id: &'a str,
}

impl<'a> GoldenAssert<'a> {
    /// Create a new assertion context.
    pub fn new(meta: &'a SuiteMeta, case_id: &'a str) -> Self {
        Self {
            suite_id: &meta.suite_id,
            case_id,
        }
    }

    /// Create with explicit suite ID.
    pub fn with_ids(suite_id: &'a str, case_id: &'a str) -> Self {
        Self { suite_id, case_id }
    }

    /// Assert with absolute tolerance.
    pub fn abs(
        &self,
        metric: &str,
        actual: f64,
        expected: f64,
        tolerance: f64,
    ) -> Result<(), Error> {
        assert_abs(
            self.suite_id,
            self.case_id,
            metric,
            actual,
            expected,
            tolerance,
        )
    }

    /// Assert with basis points tolerance.
    pub fn bp(
        &self,
        metric: &str,
        actual: f64,
        expected: f64,
        tolerance_bp: f64,
    ) -> Result<(), Error> {
        assert_bp(
            self.suite_id,
            self.case_id,
            metric,
            actual,
            expected,
            tolerance_bp,
        )
    }

    /// Assert with percentage tolerance.
    pub fn pct(
        &self,
        metric: &str,
        actual: f64,
        expected: f64,
        tolerance_pct: f64,
    ) -> Result<(), Error> {
        assert_pct(
            self.suite_id,
            self.case_id,
            metric,
            actual,
            expected,
            tolerance_pct,
        )
    }

    /// Assert within a range.
    pub fn range(
        &self,
        metric: &str,
        actual: f64,
        min: Option<f64>,
        max: Option<f64>,
    ) -> Result<(), Error> {
        assert_range(self.suite_id, self.case_id, metric, actual, min, max)
    }

    /// Assert with ExpectedValue.
    pub fn expected(
        &self,
        metric: &str,
        actual: f64,
        expected: &ExpectedValue,
    ) -> Result<(), Error> {
        assert_expected_value(self.suite_id, self.case_id, metric, actual, expected)
    }

    /// Assert Money with absolute tolerance.
    pub fn money(
        &self,
        metric: &str,
        actual: Money,
        expected: Money,
        tolerance: f64,
    ) -> Result<(), Error> {
        assert_money_abs(
            self.suite_id,
            self.case_id,
            metric,
            actual,
            expected,
            tolerance,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::types::{Expectation, ExpectedValue};
    use std::collections::HashMap;

    #[test]
    fn test_assert_abs_pass() {
        let result = assert_abs("suite", "case", "metric", 1.005, 1.0, 0.01);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assert_abs_fail() {
        let result = assert_abs("suite", "case", "metric", 1.02, 1.0, 0.01);
        assert!(result.is_err(), "Expected assertion to fail");
        if let Err(e) = result {
            let err = e.to_string();
            assert!(err.contains("suite/case"));
            assert!(err.contains("metric"));
        }
    }

    #[test]
    fn test_assert_range_pass() {
        let result = assert_range("suite", "case", "metric", 50.0, Some(0.0), Some(100.0));
        assert!(result.is_ok());
    }

    #[test]
    fn test_assert_range_fail_below() {
        let result = assert_range("suite", "case", "metric", -1.0, Some(0.0), Some(100.0));
        assert!(result.is_err());
    }

    #[test]
    fn test_assert_range_fail_above() {
        let result = assert_range("suite", "case", "metric", 101.0, Some(0.0), Some(100.0));
        assert!(result.is_err());
    }

    #[test]
    fn test_golden_assert_builder() {
        let meta = SuiteMeta {
            suite_id: "test_suite".to_string(),
            ..Default::default()
        };
        let golden_assert = GoldenAssert::new(&meta, "case_1");

        assert!(golden_assert.abs("value", 1.005, 1.0, 0.01).is_ok());
        assert!(golden_assert.abs("value", 1.02, 1.0, 0.01).is_err());
    }

    #[test]
    fn test_comparison_result_format() {
        let result = ComparisonResult::fail(
            "suite",
            "case",
            "price",
            100.5,
            100.0,
            Some(Tolerance::Abs(0.1)),
            "outside tolerance".to_string(),
        );
        let msg = result.format_error();
        assert!(msg.contains("suite/case"));
        assert!(msg.contains("price"));
        assert!(msg.contains("100.5"));
        assert!(msg.contains("100"));
    }

    #[test]
    fn test_assert_money_rejects_currency_mismatch() {
        let result = assert_money_abs(
            "suite",
            "case",
            "money",
            Money::new(100.0, finstack_quant_core::currency::Currency::USD),
            Money::new(100.0, finstack_quant_core::currency::Currency::JPY),
            0.0,
        );

        assert!(
            result.is_err(),
            "golden money assertions should reject matching amounts with mismatched currencies"
        );
    }

    #[test]
    fn comparison_result_pass_and_fail_range_format() {
        let pass =
            ComparisonResult::pass("suite", "case", "m", 1.0, 1.0, Some(Tolerance::Abs(0.01)));
        assert!(pass.passed);
        assert!(pass.format_error().contains("suite/case"));

        let fail_range = ComparisonResult::fail_range(
            "suite",
            "case",
            "m",
            3.0,
            Some(0.0),
            Some(2.0),
            "outside range".to_string(),
        );
        let msg = fail_range.format_error();
        assert!(msg.contains("range=[Some(0.0), Some(2.0)]"));
        assert!(msg.contains("outside range"));
    }

    #[test]
    fn assert_expected_f64_and_expected_value_branches() {
        let exact = Expectation::Exact {
            value: 1.0,
            tolerance: Some(Tolerance::Abs(0.05)),
            notes: None,
        };
        assert!(assert_expected_f64("s", "c", "m", 1.02, &exact).is_ok());
        assert!(assert_expected_f64("s", "c", "m", 2.0, &exact).is_err());

        let range = Expectation::Range {
            min: Some(0.0),
            max: Some(1.0),
            notes: None,
        };
        assert!(assert_expected_f64("s", "c", "m", 0.5, &range).is_ok());
        assert!(assert_expected_f64("s", "c", "m", 2.0, &range).is_err());

        let ev = ExpectedValue {
            value: 10.0,
            tolerance_abs: Some(0.1),
            tolerance_bp: None,
            tolerance_pct: None,
            tolerance_rel: None,
            notes: None,
        };
        assert!(assert_expected_value("s", "c", "k", 10.05, &ev).is_ok());
        assert!(assert_expected_value("s", "c", "k", 12.0, &ev).is_err());
    }

    #[test]
    fn assert_within_bp_pct_and_map_helpers() {
        assert!(assert_within_tolerance("s", "c", "m", 1.0, 1.0, Tolerance::Abs(0.01),).is_ok());
        assert!(assert_bp("s", "c", "m", 1.0001, 1.0, 1.0).is_ok());
        assert!(assert_pct("s", "c", "m", 1.005, 1.0, 1.0).is_ok());

        let mut actual = HashMap::new();
        actual.insert("a".to_string(), 1.0);
        let mut expected = HashMap::new();
        expected.insert(
            "a".to_string(),
            ExpectedValue {
                value: 1.0,
                tolerance_abs: Some(0.01),
                tolerance_bp: None,
                tolerance_pct: None,
                tolerance_rel: None,
                notes: None,
            },
        );
        assert!(assert_map_f64("s", "c", &actual, &expected).is_ok());
        expected.insert(
            "missing".to_string(),
            ExpectedValue {
                value: 0.0,
                tolerance_abs: None,
                tolerance_bp: None,
                tolerance_pct: None,
                tolerance_rel: None,
                notes: None,
            },
        );
        assert!(assert_map_f64("s", "c", &actual, &expected).is_err());

        let mut inner_a = HashMap::new();
        inner_a.insert("p1".to_string(), 2.0);
        let mut outer = HashMap::new();
        outer.insert("n1".to_string(), inner_a.clone());
        let mut inner_e = HashMap::new();
        inner_e.insert("p1".to_string(), 2.0);
        let mut outer_e = HashMap::new();
        outer_e.insert("n1".to_string(), inner_e);
        assert!(assert_nested_map_f64("s", "c", &outer, &outer_e, 1e-9).is_ok());
    }

    #[test]
    fn golden_assert_with_ids_covers_bp_pct_range_expected_money() {
        let ga = GoldenAssert::with_ids("suite", "case");
        assert!(ga.bp("x", 1.0, 1.0, 1.0).is_ok());
        assert!(ga.pct("x", 100.0, 100.0, 0.1).is_ok());
        assert!(ga.range("x", 5.0, Some(0.0), Some(10.0)).is_ok());
        let ev = ExpectedValue {
            value: 3.0,
            tolerance_abs: Some(0.1),
            tolerance_bp: None,
            tolerance_pct: None,
            tolerance_rel: None,
            notes: None,
        };
        assert!(ga.expected("x", 3.01, &ev).is_ok());
        assert!(ga
            .money(
                "pv",
                Money::new(10.0, finstack_quant_core::currency::Currency::USD),
                Money::new(10.0, finstack_quant_core::currency::Currency::USD),
                0.01,
            )
            .is_ok());
    }

    #[test]
    fn golden_check_macro_collects_errors() {
        let mut errors = Vec::new();
        crate::golden_check!(errors, assert_abs("s", "c", "m", 1.0, 1.0, 0.01));
        crate::golden_check!(errors, assert_abs("s", "c", "m", 2.0, 1.0, 0.01));
        assert_eq!(errors.len(), 1);
    }
}
