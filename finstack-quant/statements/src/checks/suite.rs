//! Named suites of checks with filtering and merge support.

use serde::{Deserialize, Serialize};

use super::builtins::{
    BalanceSheetArticulation, CashReconciliation, MissingValueCheck, NonFiniteCheck,
    RetainedEarningsReconciliation, SignConventionCheck,
};
use super::traits::{Check, CheckContext};
use super::types::{CheckCategory, CheckConfig, CheckReport, CheckResult, CheckSummary, Severity};
use crate::evaluator::{Evaluator, StatementResult};
use crate::types::FinancialModelSpec;
use crate::Result;

/// A named, self-contained collection of checks with its own configuration.
pub struct CheckSuite {
    /// Suite name for display/logging.
    name: String,
    /// Optional description.
    description: Option<String>,
    /// Checks in this suite.
    checks: Vec<Box<dyn Check>>,
    /// Configuration applied when running the suite.
    config: CheckConfig,
}

impl CheckSuite {
    /// Start building a new suite.
    #[must_use]
    pub fn builder(name: impl Into<String>) -> CheckSuiteBuilder {
        CheckSuiteBuilder {
            name: name.into(),
            description: None,
            checks: Vec::new(),
            config: CheckConfig::default(),
        }
    }

    /// Merge another suite's checks into this one, consuming the other suite.
    pub fn merge(mut self, other: CheckSuite) -> Self {
        self.checks.extend(other.checks);
        self
    }

    /// Execute all checks in the suite, applying `min_severity` and
    /// `materiality_threshold` filters from the config.
    ///
    /// Each check produces its full findings first; filtering then removes
    /// findings below the configured severity or absolute materiality.
    /// Error-severity findings are exempt from materiality filtering, so a
    /// materiality threshold can never flip a failing check to passed. A check
    /// passes when none of its retained findings has `Error` severity. The
    /// returned summary counts only retained findings, so it describes the
    /// reporting policy rather than every raw diagnostic a check generated.
    ///
    /// # Errors
    ///
    /// Propagates the first error returned by an individual check, such as a
    /// missing required model node, incompatible result data, or invalid
    /// check-specific configuration. On error no partial `CheckReport` is
    /// returned; run smaller suites when callers need per-check isolation.
    pub fn run(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
    ) -> Result<CheckReport> {
        let context = CheckContext {
            model,
            results,
            config: self.config.clone(),
        };
        let min_severity = context.config.min_severity;
        let mat_threshold = context.config.materiality_threshold;

        let mut filtered_results: Vec<CheckResult> = Vec::with_capacity(self.checks.len());

        for check in &self.checks {
            let mut result = check.execute(&context)?;

            result.findings.retain(|f| {
                if f.severity < min_severity {
                    return false;
                }
                // Materiality is a reporting filter for advisory findings
                // only. Error findings are always retained: suppressing one
                // here would flip `passed` (recomputed below) and let a
                // materiality setting silently convert a failing accounting
                // identity into a passing check. Tolerances — not materiality
                // — decide whether a diff is an Error in the first place.
                if mat_threshold > 0.0 && f.severity < Severity::Error {
                    if let Some(ref m) = f.materiality {
                        if m.absolute.abs() < mat_threshold {
                            return false;
                        }
                    }
                }
                true
            });

            result.passed = !result
                .findings
                .iter()
                .any(|f| f.severity == Severity::Error);

            filtered_results.push(result);
        }

        let total_checks = filtered_results.len();
        let passed = filtered_results.iter().filter(|r| r.passed).count();
        let failed = total_checks - passed;

        let mut errors: usize = 0;
        let mut warnings: usize = 0;
        let mut infos: usize = 0;
        for finding in filtered_results.iter().flat_map(|r| &r.findings) {
            match finding.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => infos += 1,
            }
        }

        Ok(CheckReport {
            results: filtered_results,
            summary: CheckSummary {
                total_checks,
                passed,
                failed,
                errors,
                warnings,
                infos,
            },
        })
    }

    /// Run this suite against supplied results or evaluate the model first.
    ///
    /// # Arguments
    ///
    /// * `model` - Financial model whose node definitions and periods provide
    ///   check context.
    /// * `results` - Optional precomputed statement results for `model`. When
    ///   absent, a fresh [`Evaluator`] applies the canonical value, forecast,
    ///   and formula precedence before checks run.
    ///
    /// # Returns
    ///
    /// A complete report containing one result per check and aggregate counts.
    ///
    /// # Errors
    ///
    /// Returns the first model-evaluation or check-execution error. Supplied
    /// results are used directly and are never recomputed.
    pub fn run_model(
        &self,
        model: &FinancialModelSpec,
        results: Option<&StatementResult>,
    ) -> Result<CheckReport> {
        match results {
            Some(results) => self.run(model, results),
            None => {
                let evaluated = Evaluator::new().evaluate(model)?;
                self.run(model, &evaluated)
            }
        }
    }

    /// Number of checks in the suite.
    pub fn len(&self) -> usize {
        self.checks.len()
    }

    /// True when the suite contains no checks.
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    /// Identifiers of the checks in this suite, in execution order.
    ///
    /// Useful for reporting which checks a factory-built suite actually
    /// assembled, since several are conditional on the supplied mapping.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_statements::checks::builtins::NonFiniteCheck;
    /// use finstack_quant_statements::checks::CheckSuite;
    ///
    /// let suite = CheckSuite::builder("demo")
    ///     .add_check(NonFiniteCheck { nodes: vec![] })
    ///     .build();
    /// assert_eq!(suite.check_ids(), vec!["non_finite"]);
    /// ```
    pub fn check_ids(&self) -> Vec<&str> {
        self.checks.iter().map(|c| c.id()).collect()
    }

    /// Suite name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Suite description, if set.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Fluent builder for [`CheckSuite`].
pub struct CheckSuiteBuilder {
    name: String,
    description: Option<String>,
    checks: Vec<Box<dyn Check>>,
    config: CheckConfig,
}

impl CheckSuiteBuilder {
    /// Set the suite description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a check to the suite.
    pub fn add_check(mut self, check: impl Check + 'static) -> Self {
        self.checks.push(Box::new(check));
        self
    }

    /// Override the default configuration.
    pub fn config(mut self, config: CheckConfig) -> Self {
        self.config = config;
        self
    }

    /// Consume the builder and produce a [`CheckSuite`].
    pub fn build(self) -> CheckSuite {
        CheckSuite {
            name: self.name,
            description: self.description,
            checks: self.checks,
            config: self.config,
        }
    }
}

// Serializable suite spec

/// Serializable descriptor for a [`CheckSuite`] that can be saved/loaded as
/// JSON for team-wide check policies.
///
/// Both built-in and formula checks are resolved by [`CheckSuiteSpec::resolve`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CheckSuiteSpec {
    /// Suite name.
    pub name: String,
    /// Suite description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Built-in checks to include.
    #[serde(default)]
    pub builtin_checks: Vec<BuiltinCheckSpec>,
    /// User-defined formula checks.
    #[serde(default)]
    pub formula_checks: Vec<FormulaCheckSpec>,
    /// Suite configuration.
    #[serde(default)]
    pub config: CheckConfig,
}

impl CheckSuiteSpec {
    /// Resolve the spec into a runnable [`CheckSuite`].
    ///
    /// The resolved suite preserves this spec's name, description, and
    /// filtering configuration.
    ///
    /// # Errors
    ///
    /// This currently returns `Ok` for every deserialized spec because checks
    /// are materialized without runtime I/O or model access. Formula syntax and
    /// references are validated when the suite runs against a model.
    pub fn resolve(&self) -> Result<CheckSuite> {
        let mut checks: Vec<Box<dyn Check>> = self
            .builtin_checks
            .iter()
            .map(BuiltinCheckSpec::to_check)
            .collect();
        checks.extend(
            self.formula_checks
                .iter()
                .cloned()
                .map(|check| Box::new(check) as Box<dyn Check>),
        );
        Ok(CheckSuite {
            name: self.name.clone(),
            description: self.description.clone(),
            checks,
            config: self.config.clone(),
        })
    }
}

/// Tagged enum describing any built-in check in a serializable form.
///
/// Each variant wraps its check struct, so the struct is the single schema;
/// the JSON shape is the struct's fields plus a `type` tag. Convert into a
/// boxed [`Check`] via [`BuiltinCheckSpec::to_check`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuiltinCheckSpec {
    /// Balance sheet articulation: Assets = Liabilities + Equity.
    BalanceSheetArticulation(BalanceSheetArticulation),
    /// Retained earnings reconciliation across periods.
    RetainedEarningsReconciliation(RetainedEarningsReconciliation),
    /// Cash balance reconciliation: Cash(t) = Cash(t-1) + TotalCF(t).
    CashReconciliation(CashReconciliation),
    /// Flags required nodes that lack values in applicable periods.
    MissingValue(MissingValueCheck),
    /// Flags values with unexpected signs.
    SignConvention(SignConventionCheck),
    /// Detects NaN or infinite values.
    NonFinite(NonFiniteCheck),
}

impl BuiltinCheckSpec {
    /// Serde `type` tags of every built-in check, in declaration order.
    ///
    /// These are the strings a suite spec's `builtin_checks[].type` field
    /// accepts. Host bindings expose the list so callers can discover the
    /// catalog without reading the schema.
    pub const fn names() -> &'static [&'static str] {
        &[
            "balance_sheet_articulation",
            "retained_earnings_reconciliation",
            "cash_reconciliation",
            "missing_value",
            "sign_convention",
            "non_finite",
        ]
    }

    /// Convert this spec into a boxed [`Check`] implementation.
    pub fn to_check(&self) -> Box<dyn Check> {
        match self {
            Self::BalanceSheetArticulation(check) => Box::new(check.clone()),
            Self::RetainedEarningsReconciliation(check) => Box::new(check.clone()),
            Self::CashReconciliation(check) => Box::new(check.clone()),
            Self::MissingValue(check) => Box::new(check.clone()),
            Self::SignConvention(check) => Box::new(check.clone()),
            Self::NonFinite(check) => Box::new(check.clone()),
        }
    }
}

/// Serializable, runnable user-defined formula-check specification.
///
/// Full suite definitions (built-in + formula) can be stored as a single JSON
/// document and resolved directly with [`CheckSuiteSpec::resolve`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FormulaCheckSpec {
    /// Unique identifier for this check instance.
    pub id: String,
    /// Human-readable name shown in reports.
    pub name: String,
    /// Category grouping.
    pub category: CheckCategory,
    /// Severity assigned to findings when the formula fails.
    pub severity: Severity,
    /// Statements DSL expression to evaluate (e.g. `"revenue > 0"`).
    ///
    /// Formula checks use the same evaluator as calculated statement nodes,
    /// including time-series functions and `cs.*` capital-structure references.
    pub formula: String,
    /// Template for the finding message (`{period}` is replaced at runtime).
    pub message_template: String,
    /// Numeric tolerance for floating-point comparisons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every advertised builtin name must be the serde `type` tag of exactly
    /// one variant, so the discovery list cannot drift from the wire format.
    #[test]
    fn builtin_names_match_serde_tags() {
        // A known tag with missing fields fails on the *fields*; an unknown
        // tag fails on the variant. Only the latter means the list drifted.
        for name in BuiltinCheckSpec::names() {
            let attempt: std::result::Result<BuiltinCheckSpec, serde_json::Error> =
                serde_json::from_value(serde_json::json!({ "type": name }));
            if let Err(err) = attempt {
                assert!(
                    !err.to_string().contains("unknown variant"),
                    "'{name}' is not a BuiltinCheckSpec tag: {err}"
                );
            }
        }
        let unknown: std::result::Result<BuiltinCheckSpec, serde_json::Error> =
            serde_json::from_value(serde_json::json!({ "type": "no_such_check" }));
        assert!(unknown
            .expect_err("unknown tag must fail")
            .to_string()
            .contains("unknown variant"));
        // Tag-only construction succeeds for the checks whose fields all
        // default, which is what a host's `builtin_checks=["non_finite"]`
        // shorthand relies on.
        for name in ["non_finite", "sign_convention"] {
            serde_json::from_value::<BuiltinCheckSpec>(serde_json::json!({ "type": name }))
                .expect("defaultable builtin");
        }
    }
}
