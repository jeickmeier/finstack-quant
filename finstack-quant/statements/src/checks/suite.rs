//! Named suites of checks with filtering and merge support.

use serde::{Deserialize, Serialize};

use super::builtins::{
    BalanceSheetArticulation, CashReconciliation, MissingValueCheck, NonFiniteCheck,
    RetainedEarningsReconciliation, SignConventionCheck,
};
use super::traits::{Check, CheckContext};
use super::types::{
    CheckCategory, CheckConfig, CheckOutcome, CheckReport, CheckResult, CheckSummary, Severity,
};
use crate::Result;
use crate::evaluator::{Evaluator, ObservationCutoff, StatementResult};
use crate::types::FinancialModelSpec;

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
    ///
    /// The other suite's checks are appended in their declared order.
    ///
    /// # Arguments
    ///
    /// * `other` - Suite to append. Its configuration must equal this
    ///   suite's, and its check identifiers must not repeat any of this
    ///   suite's.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when the two configurations differ or
    /// when a check identifier would appear twice.
    pub fn merge(mut self, other: CheckSuite) -> Result<Self> {
        if self.config != other.config {
            return Err(crate::Error::invalid_input(format!(
                "check suites '{}' and '{}' have different configurations and cannot be merged",
                self.name, other.name
            )));
        }
        self.checks.extend(other.checks);
        unique_check_ids(&self.name, &self.checks)?;
        Ok(self)
    }

    /// Run every check in declared order against verified results.
    ///
    /// Each check's verdict is decided from all of its findings; the report
    /// verdict from all check verdicts. The `min_severity` and
    /// `materiality_threshold` filters then remove advisory findings only:
    /// an error-severity finding, a coverage finding and every recorded
    /// comparison stay in the report, so no filter can change a verdict or
    /// hide a failure.
    ///
    /// # Arguments
    ///
    /// * `model` - Validated model the results were evaluated from.
    /// * `results` - Results of an evaluation run of `model`. Their recorded
    ///   provenance must identify this model's content, its source snapshot
    ///   and period plan, and a run of it; hand-built results are rejected.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error for results with missing or mismatched
    /// provenance, and propagates the first configuration or execution error
    /// of a check, such as a missing required node, incompatible units, or an
    /// invalid check-specific setting. On error no partial report is
    /// returned.
    pub fn run(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
    ) -> Result<CheckReport> {
        self.config.validate()?;
        verify_supplied_results(model, results, None)?;
        let context = CheckContext {
            model,
            results,
            config: self.config.clone(),
        };
        let min_severity = context.config.min_severity;
        let mat_threshold = context.config.materiality_threshold;

        let mut results: Vec<CheckResult> = Vec::with_capacity(self.checks.len());
        for check in &self.checks {
            // The verdict is decided inside `CheckResult::new` from every
            // finding. The reporting filters below remove advisory findings
            // only: an error-severity finding, a coverage finding and every
            // recorded comparison stay visible, and no filter changes an
            // outcome.
            let mut result = check.execute(&context)?;
            result.findings.retain(|f| {
                if f.severity == Severity::Error || f.coverage {
                    return true;
                }
                if f.severity < min_severity {
                    return false;
                }
                !(mat_threshold > 0.0
                    && f.materiality
                        .as_ref()
                        .is_some_and(|m| m.absolute.abs() < mat_threshold))
            });
            results.push(result);
        }

        let count = |outcome: CheckOutcome| results.iter().filter(|r| r.outcome == outcome).count();
        let (passed, failed, incomplete, unevaluated) = (
            count(CheckOutcome::Pass),
            count(CheckOutcome::Fail),
            count(CheckOutcome::Incomplete),
            count(CheckOutcome::Unevaluated),
        );
        // An empty suite, or one in which no check had anything to examine,
        // is unevaluated: a pass requires evaluated work.
        let outcome = if failed > 0 {
            CheckOutcome::Fail
        } else if incomplete > 0 {
            CheckOutcome::Incomplete
        } else if passed > 0 {
            CheckOutcome::Pass
        } else {
            CheckOutcome::Unevaluated
        };
        let severity_count = |severity: Severity| {
            results
                .iter()
                .flat_map(|r| &r.findings)
                .filter(|f| f.severity == severity)
                .count()
        };
        let summary = CheckSummary {
            total_checks: results.len(),
            passed,
            failed,
            incomplete,
            unevaluated,
            errors: severity_count(Severity::Error),
            warnings: severity_count(Severity::Warning),
            infos: severity_count(Severity::Info),
        };
        Ok(CheckReport {
            outcome,
            results,
            summary,
        })
    }

    /// Run this suite against supplied results or evaluate the model first.
    ///
    /// # Arguments
    ///
    /// * `model` - Financial model whose node definitions and periods provide
    ///   check context.
    /// * `observation_cutoff` - The requested evaluation context: which
    ///   observations the run may read.
    /// * `results` - Optional results of that run. They are accepted only
    ///   with verified identity: this model's content, source snapshot and
    ///   period plan, and the requested cutoff. When absent, the model is
    ///   evaluated under `observation_cutoff`.
    ///
    /// # Returns
    ///
    /// A complete report containing one result per check and aggregate counts.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error for supplied results with missing or
    /// mismatched provenance, and the first model-evaluation or
    /// check-execution error. Supplied results are never recomputed.
    pub fn run_model(
        &self,
        model: &FinancialModelSpec,
        observation_cutoff: &ObservationCutoff,
        results: Option<&StatementResult>,
    ) -> Result<CheckReport> {
        match results {
            Some(results) => {
                verify_supplied_results(model, results, Some(observation_cutoff))?;
                self.run(model, results)
            }
            None => {
                let evaluated =
                    Evaluator::new().evaluate_under_cutoff(model, observation_cutoff)?;
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
    /// # fn main() -> finstack_quant_statements::Result<()> {
    /// let suite = CheckSuite::builder("demo")
    ///     .add_check(NonFiniteCheck { nodes: finstack_quant_statements::checks::builtins::NodeSelection::all() })
    ///     .build()?;
    /// assert_eq!(suite.check_ids(), vec!["non_finite"]);
    /// # Ok(())
    /// # }
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
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when two checks share an identifier.
    pub fn build(self) -> Result<CheckSuite> {
        unique_check_ids(&self.name, &self.checks)?;
        Ok(CheckSuite {
            name: self.name,
            description: self.description,
            checks: self.checks,
            config: self.config,
        })
    }
}

/// Reject a suite in which two checks share an identifier: results are
/// reported per check, so an identifier names exactly one of them.
fn unique_check_ids(suite: &str, checks: &[Box<dyn Check>]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for check in checks {
        if !seen.insert(check.id()) {
            return Err(crate::Error::invalid_input(format!(
                "check suite '{suite}' has two checks with the identifier '{}'",
                check.id()
            )));
        }
    }
    Ok(())
}

/// Accept supplied results only as a verified run of `model`.
///
/// The results must record provenance, and that provenance must be the one
/// an evaluation of this model content, source snapshot and period plan
/// produces under its recorded cutoff and valuation date, and under the
/// requested cutoff when one is given.
fn verify_supplied_results(
    model: &FinancialModelSpec,
    results: &StatementResult,
    requested: Option<&ObservationCutoff>,
) -> Result<()> {
    let mismatch = |what: &str| {
        crate::Error::invalid_input(format!(
            "supplied results are not a verified run of model '{}': {what}. Evaluate the \
             model, or pass no results to have it evaluated",
            model.id
        ))
    };
    let provenance = results
        .provenance
        .as_ref()
        .ok_or_else(|| mismatch("they record no provenance"))?;
    if requested.is_some_and(|cutoff| *cutoff != provenance.observation_cutoff) {
        return Err(mismatch(
            "their observation cutoff is not the requested one",
        ));
    }
    let expected = crate::evaluator::RunProvenance::for_run(
        model,
        provenance.observation_cutoff,
        provenance.valuation_date,
    )?;
    if expected != *provenance {
        return Err(mismatch(
            "their model content, source snapshot or run identity differs",
        ));
    }
    if results.periods.as_ref() != Some(&model.periods) {
        return Err(mismatch("their period plan differs"));
    }
    Ok(())
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
    /// Returns an error when configured tolerances or materiality thresholds
    /// are negative or non-finite. Formula syntax and references are validated
    /// when the suite runs against a model.
    pub fn resolve(&self) -> Result<CheckSuite> {
        self.config.validate()?;
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
        unique_check_ids(&self.name, &checks)?;
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
///
/// Each period ends in one of three verdicts: pass, fail (a finding with the
/// configured `severity`), or insufficient data. A period has insufficient
/// data when a same-period input is missing or non-finite, or when the
/// formula consumes a non-finite value, such as a lag or rolling window that
/// reaches before the first period. It is reported as a `warning` finding
/// naming the inputs, records no comparison, and never fails the check. A
/// division by zero over finite inputs is not missing data and still fails.
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
    /// Finite, nonnegative absolute residual bound in the formula's result units.
    /// When present, the formula passes when its absolute result is at most
    /// this bound. When absent, the formula is a predicate and any finite
    /// nonzero result passes.
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
        assert!(
            unknown
                .expect_err("unknown tag must fail")
                .to_string()
                .contains("unknown variant")
        );
        // The sign check's groups default to empty; the validity check's
        // node selection is explicit and has no default.
        serde_json::from_value::<BuiltinCheckSpec>(
            serde_json::json!({ "type": "sign_convention" }),
        )
        .expect("defaultable builtin");
        serde_json::from_value::<BuiltinCheckSpec>(serde_json::json!({ "type": "non_finite" }))
            .expect_err("the node selection is required");
        serde_json::from_value::<BuiltinCheckSpec>(
            serde_json::json!({ "type": "non_finite", "nodes": "all" }),
        )
        .expect("explicit selection of all nodes");
    }
}
