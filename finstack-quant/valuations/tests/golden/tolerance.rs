//! Tolerance comparator for golden fixture metrics.
//!
//! A metric with both absolute and relative tolerances passes when either
//! tolerance is satisfied, matching the fixture schema contract.

use crate::golden::schema::ToleranceEntry;

const REL_DENOM_MIN: f64 = 1e-12;
/// Result of comparing one actual metric against its reference value.
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Metric name.
    pub metric: String,
    /// Actual value produced by finstack_quant.
    pub actual: f64,
    /// Reference value from the fixture.
    pub expected: f64,
    /// Absolute difference.
    pub abs_diff: f64,
    /// Relative difference, stabilized for expected values near zero.
    pub rel_diff: f64,
    /// True when the configured tolerance policy passed.
    pub passed: bool,
    /// Tolerance used for this comparison.
    pub used_tolerance: ToleranceEntry,
}

impl ComparisonResult {
    /// Format a failure message with enough context to diagnose the metric drift.
    pub fn failure_message(&self, fixture_path: &str) -> String {
        format!(
            "Golden mismatch in {}\n  metric: {}\n  actual: {:.12}\n  expected: {:.12}\n  abs_diff: {:.6e}\n  rel_diff: {:.6e}\n  tolerance: abs={:?}, rel={:?}",
            fixture_path,
            self.metric,
            self.actual,
            self.expected,
            self.abs_diff,
            self.rel_diff,
            self.used_tolerance.abs,
            self.used_tolerance.rel,
        )
    }
}

/// Compare one metric using strict tolerance semantics.
///
/// Panics when neither tolerance is set because that indicates a malformed fixture.
pub fn compare(metric: &str, actual: f64, expected: f64, tol: &ToleranceEntry) -> ComparisonResult {
    let abs_diff = (actual - expected).abs();
    let rel_diff = abs_diff / expected.abs().max(REL_DENOM_MIN);
    let abs_pass = tol.abs.is_some_and(|abs| abs_diff <= abs);
    let rel_pass = tol.rel.is_some_and(|rel| rel_diff <= rel);

    assert!(
        tol.abs.is_some() || tol.rel.is_some(),
        "ToleranceEntry for metric '{metric}' has neither abs nor rel; malformed fixture"
    );

    ComparisonResult {
        metric: metric.to_string(),
        actual,
        expected,
        abs_diff,
        rel_diff,
        passed: abs_pass || rel_pass,
        used_tolerance: tol.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs_only(abs: f64) -> ToleranceEntry {
        ToleranceEntry {
            abs: Some(abs),
            rel: None,
            tolerance_reason: None,
        }
    }

    fn rel_only(rel: f64) -> ToleranceEntry {
        ToleranceEntry {
            abs: None,
            rel: Some(rel),
            tolerance_reason: None,
        }
    }

    fn both(abs: f64, rel: f64) -> ToleranceEntry {
        ToleranceEntry {
            abs: Some(abs),
            rel: Some(rel),
            tolerance_reason: None,
        }
    }

    fn both_with_reason(abs: f64, rel: f64, reason: &str) -> ToleranceEntry {
        ToleranceEntry {
            abs: Some(abs),
            rel: Some(rel),
            tolerance_reason: Some(reason.to_string()),
        }
    }

    #[test]
    fn abs_only_pass() {
        let result = compare("x", 1.005, 1.0, &abs_only(0.01));
        assert!(result.passed);
    }

    #[test]
    fn abs_only_fail() {
        let result = compare("x", 1.5, 1.0, &abs_only(0.01));
        assert!(!result.passed);
    }

    #[test]
    fn rel_only_pass() {
        let result = compare("x", 1.0001, 1.0, &rel_only(1e-3));
        assert!(result.passed);
    }

    #[test]
    fn rel_handles_zero_expected() {
        let result = compare("x", 1e-15, 0.0, &rel_only(1e-3));
        assert!(
            result.passed,
            "expected pass, got {}",
            result.failure_message("test")
        );
    }

    #[test]
    fn either_abs_or_rel_passes_by_default() {
        let result = compare("x", 1_000_000.5, 1_000_000.0, &both(0.01, 1e-6));
        assert!(result.passed);
    }

    #[test]
    fn tolerance_reason_is_not_needed_for_or_semantics() {
        let result = compare(
            "x",
            1_000_000.5,
            1_000_000.0,
            &both_with_reason(0.01, 1e-6, "vendor screen rounding"),
        );
        assert!(result.passed);
    }

    #[test]
    fn neither_passes() {
        let result = compare("x", 100.0, 1.0, &both(0.01, 1e-6));
        assert!(!result.passed);
    }

    #[test]
    #[should_panic(expected = "neither abs nor rel")]
    fn empty_tolerance_panics() {
        let empty = ToleranceEntry {
            abs: None,
            rel: None,
            tolerance_reason: None,
        };
        compare("x", 1.0, 1.0, &empty);
    }
}
