//! Core types for the golden test framework.
//!
//! This module defines the data structures used for loading and validating
//! golden test fixtures across all finstack crates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Suite-level types
// =============================================================================

/// A golden test suite containing metadata and test cases.
///
/// This is the canonical JSON structure for golden fixtures:
///
/// ```json
/// {
///   "meta": {
///     "suite_id": "my_suite",
///     "description": "...",
///     "reference_source": { "name": "...", ... },
///     "generated": { "at": "...", "by": "..." },
///     "status": "certified",
///     "schema_version": 1
///   },
///   "cases": [ ... ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenSuite<T> {
    /// Suite-level metadata including provenance.
    pub meta: SuiteMeta,
    /// Test cases in this suite.
    pub cases: Vec<T>,
}

/// Suite-level metadata with provenance information.
///
/// All golden fixtures must include provenance to document where and when
/// the expected values were generated.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SuiteMeta {
    /// Unique identifier for this suite.
    pub suite_id: String,

    /// Human-readable description of what this suite tests.
    #[serde(default)]
    pub description: String,

    /// Reference source for expected values (e.g., ISDA, QuantLib, Excel).
    #[serde(default)]
    pub reference_source: ReferenceSource,

    /// Information about how/when this suite was generated.
    #[serde(default)]
    pub generated: GeneratedInfo,

    /// Information about validation of expected values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validated: Option<ValidatedInfo>,

    /// Suite status: "certified", "provisional", "pending_validation".
    #[serde(default = "default_status")]
    pub status: String,

    /// Schema version for forward compatibility.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Extensible metadata bag for future additions.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_status() -> String {
    "unknown".to_string()
}

fn default_schema_version() -> u32 {
    1
}

/// Reference source for expected values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReferenceSource {
    /// Name of the reference source (required).
    pub name: String,

    /// Version of the reference implementation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Vendor or organization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,

    /// URL for more information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Extensible metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Information about how/when the golden data was generated.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneratedInfo {
    /// ISO 8601 timestamp of generation.
    pub at: String,

    /// Tool or script that generated the data.
    pub by: String,

    /// Command used to regenerate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Environment information (python version, OS, etc.).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub environment: HashMap<String, String>,
}

/// Information about validation of expected values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidatedInfo {
    /// When the validation was performed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,

    /// Who performed the validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,

    /// Validation method (e.g., "manual spot-check", "automated comparison").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    /// Additional notes about validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// =============================================================================
// Case-level types
// =============================================================================

/// Optional metadata for individual test cases.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaseMeta {
    /// Notes about this specific test case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Tags for filtering/categorization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Override reference source for this case if different from suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_override: Option<ReferenceSource>,

    /// Extensible metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

// =============================================================================
// Comparison types
// =============================================================================

/// Tolerance specification for numeric comparisons.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Tolerance {
    /// Absolute tolerance (e.g., 0.01 means |actual - expected| < 0.01).
    #[serde(rename = "abs")]
    Abs(f64),

    /// Relative tolerance as a fraction (e.g., 0.001 means 0.1% relative error).
    #[serde(rename = "rel")]
    Rel(f64),

    /// Basis points tolerance (1 bp = 0.0001).
    #[serde(rename = "bp")]
    Bps(f64),

    /// Percentage tolerance (e.g., 0.1 means 0.1% relative error).
    #[serde(rename = "pct")]
    Pct(f64),
}

impl Tolerance {
    fn bp_error(actual: f64, expected: f64) -> f64 {
        (actual - expected).abs() * 10_000.0
    }

    /// Check if actual is within tolerance of expected.
    pub fn is_within(&self, actual: f64, expected: f64) -> bool {
        match self {
            Tolerance::Abs(tol) => (actual - expected).abs() <= *tol,
            Tolerance::Rel(tol) => {
                if expected.abs() < 1e-15 {
                    actual.abs() <= *tol
                } else {
                    ((actual - expected) / expected).abs() <= *tol
                }
            }
            Tolerance::Bps(tol) => Self::bp_error(actual, expected) <= *tol,
            Tolerance::Pct(tol) => {
                if expected.abs() < 1e-15 {
                    actual.abs() <= *tol
                } else {
                    ((actual - expected) / expected).abs() * 100.0 <= *tol
                }
            }
        }
    }

    /// Compute the error between actual and expected.
    pub fn compute_error(&self, actual: f64, expected: f64) -> f64 {
        match self {
            Tolerance::Abs(_) => (actual - expected).abs(),
            Tolerance::Rel(_) => {
                if expected.abs() < 1e-15 {
                    actual.abs()
                } else {
                    ((actual - expected) / expected).abs()
                }
            }
            Tolerance::Bps(_) => Self::bp_error(actual, expected),
            Tolerance::Pct(_) => {
                if expected.abs() < 1e-15 {
                    actual.abs()
                } else {
                    ((actual - expected) / expected).abs() * 100.0
                }
            }
        }
    }
}

/// An expected value that can be exact (with tolerance) or a range.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Expectation {
    /// Exact value with optional tolerance.
    Exact {
        /// Expected value.
        value: f64,
        /// Tolerance for comparison.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Tolerance>,
        /// Optional notes about this expectation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notes: Option<String>,
    },
    /// Range constraint (min <= actual <= max).
    Range {
        /// Minimum allowed value (inclusive).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        /// Maximum allowed value (inclusive).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        /// Optional notes about this expectation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notes: Option<String>,
    },
}

impl Expectation {
    /// Create an exact expectation with absolute tolerance.
    pub fn exact(value: f64, tolerance: f64) -> Self {
        Expectation::Exact {
            value,
            tolerance: Some(Tolerance::Abs(tolerance)),
            notes: None,
        }
    }

    /// Create an exact expectation with basis points tolerance.
    pub fn exact_bp(value: f64, tolerance_bp: f64) -> Self {
        Expectation::Exact {
            value,
            tolerance: Some(Tolerance::Bps(tolerance_bp)),
            notes: None,
        }
    }

    /// Create an exact expectation with percentage tolerance.
    pub fn exact_pct(value: f64, tolerance_pct: f64) -> Self {
        Expectation::Exact {
            value,
            tolerance: Some(Tolerance::Pct(tolerance_pct)),
            notes: None,
        }
    }

    /// Create a range expectation.
    pub fn range(min: Option<f64>, max: Option<f64>) -> Self {
        Expectation::Range {
            min,
            max,
            notes: None,
        }
    }

    /// Check if actual satisfies this expectation.
    pub fn is_satisfied(&self, actual: f64) -> bool {
        match self {
            Expectation::Exact {
                value, tolerance, ..
            } => {
                if let Some(tol) = tolerance {
                    tol.is_within(actual, *value)
                } else {
                    // Scale-aware exact comparison: relative tolerance with absolute floor
                    (actual - value).abs() <= (value.abs() * f64::EPSILON * 8.0).max(1e-15)
                }
            }
            Expectation::Range { min, max, .. } => {
                let above_min = min.is_none_or(|m| actual >= m);
                let below_max = max.is_none_or(|m| actual <= m);
                above_min && below_max
            }
        }
    }
}

/// Common expected value structure used in many golden tests.
///
/// This provides a more flexible tolerance specification that can be
/// deserialized from various JSON formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedValue {
    /// The expected value.
    pub value: f64,

    /// Absolute tolerance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_abs: Option<f64>,

    /// Basis points tolerance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_bp: Option<f64>,

    /// Percentage tolerance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_pct: Option<f64>,

    /// Relative tolerance (as fraction).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_rel: Option<f64>,

    /// Notes about this expected value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl ExpectedValue {
    /// Convert to an Expectation for comparison.
    pub fn to_expectation(&self) -> Expectation {
        let tol_count = [
            self.tolerance_abs.is_some(),
            self.tolerance_bp.is_some(),
            self.tolerance_pct.is_some(),
            self.tolerance_rel.is_some(),
        ]
        .iter()
        .filter(|&&b| b)
        .count();
        debug_assert!(
            tol_count <= 1,
            "ExpectedValue has {tol_count} tolerance fields set; \
             only the first (priority: abs>bp>pct>rel) is used"
        );
        let tolerance = self
            .tolerance_abs
            .map(Tolerance::Abs)
            .or_else(|| self.tolerance_bp.map(Tolerance::Bps))
            .or_else(|| self.tolerance_pct.map(Tolerance::Pct))
            .or_else(|| self.tolerance_rel.map(Tolerance::Rel));

        Expectation::Exact {
            value: self.value,
            tolerance,
            notes: self.notes.clone(),
        }
    }

    /// Check if actual is within tolerance.
    pub fn is_within(&self, actual: f64) -> bool {
        self.to_expectation().is_satisfied(actual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tolerance_abs() {
        let tol = Tolerance::Abs(0.01);
        assert!(tol.is_within(1.005, 1.0));
        assert!(!tol.is_within(1.02, 1.0));
    }

    #[test]
    fn test_tolerance_rel() {
        let tol = Tolerance::Rel(0.01); // 1% relative
        assert!(tol.is_within(1.005, 1.0));
        assert!(!tol.is_within(1.02, 1.0));
    }

    #[test]
    fn test_tolerance_bp() {
        let tol = Tolerance::Bps(0.5);
        // 0.00004 * 10_000 = 0.4 bp < 0.5 bp tolerance
        assert!(tol.is_within(100.00004, 100.0));
        // 0.00006 * 10_000 = 0.6 bp > 0.5 bp tolerance
        assert!(!tol.is_within(100.00006, 100.0));
    }

    #[test]
    fn test_tolerance_bp_on_decimal_rates() {
        let tol = Tolerance::Bps(0.5);
        assert!(tol.is_within(0.05004, 0.05000));
        assert!(!tol.is_within(0.05006, 0.05000));
        assert!((tol.compute_error(0.05004, 0.05000) - 0.4).abs() < 1e-12);
    }

    #[test]
    fn test_tolerance_pct() {
        let tol = Tolerance::Pct(1.0); // 1%
        assert!(tol.is_within(1.005, 1.0));
        assert!(!tol.is_within(1.02, 1.0));
    }

    #[test]
    fn test_expectation_exact() {
        let exp = Expectation::exact(100.0, 0.5);
        assert!(exp.is_satisfied(100.3));
        assert!(!exp.is_satisfied(100.6));
    }

    #[test]
    fn test_expectation_range() {
        let exp = Expectation::range(Some(0.0), Some(100.0));
        assert!(exp.is_satisfied(50.0));
        assert!(!exp.is_satisfied(-1.0));
        assert!(!exp.is_satisfied(101.0));
    }

    #[test]
    fn test_expected_value_to_expectation() {
        let ev = ExpectedValue {
            value: 100.0,
            tolerance_abs: Some(0.5),
            tolerance_bp: None,
            tolerance_pct: None,
            tolerance_rel: None,
            notes: None,
        };
        assert!(ev.is_within(100.3));
        assert!(!ev.is_within(100.6));
    }

    #[test]
    fn test_suite_meta_deserialize() {
        let json = r#"{
            "suite_id": "test",
            "description": "Test suite",
            "reference_source": { "name": "ISDA", "version": "1.8.2" },
            "generated": { "at": "2025-01-15", "by": "test.py" },
            "status": "certified",
            "schema_version": 1,
            "extra": { "custom_field": "value" }
        }"#;
        let result = serde_json::from_str::<SuiteMeta>(json);
        assert!(result.is_ok(), "Should parse SuiteMeta from JSON");
        if let Ok(meta) = result {
            assert_eq!(meta.suite_id, "test");
            assert_eq!(meta.reference_source.name, "ISDA");
            assert_eq!(meta.reference_source.version, Some("1.8.2".to_string()));
            assert!(meta.extra.contains_key("custom_field"));
        }
    }
}
