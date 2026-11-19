//! Core types for the financial statement checks framework.

use finstack_quant_core::dates::PeriodId;
use serde::{Deserialize, Serialize};

use crate::types::{AmountOrScalar, NodeId};

/// Declared sign convention for a flow / magnitude input to a reconciliation
/// check.
///
/// Each reconciliation declares its input expectation so callers can
/// validate data at construction time rather than at P&L
/// reconciliation. Previously each reconciliation embedded its sign
/// assumptions implicitly (e.g. `CapexReconciliation` expected
/// `capex_cf` to be a positive magnitude while INVARIANTS.md §3
/// documented the CFS convention as negative), which produced silent
/// cross-check failures on any model that used the other convention.
///
/// # Variants
///
/// * [`Self::MagnitudePositive`] — the value is a non-negative magnitude;
///   direction (inflow vs outflow) is encoded in the reconciliation's
///   formula via `+` / `-` operators. Used by capex, depreciation,
///   dividends, and disposal magnitudes across the cross-statement
///   reconciliations.
/// * [`Self::InflowPositive`] — the value is a signed flow where positive
///   means "inflow" / "addition" and negative means "outflow" /
///   "reduction". Used by `total_cash_flow` in `CashReconciliation` and
///   `net_income` in `RetainedEarningsReconciliation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SignConventionPolicy {
    /// The value must be a non-negative magnitude (the reconciliation's
    /// formula adds or subtracts it explicitly). This is the default for
    /// historical reasons — every reconciliation in the current code
    /// assumes magnitudes are positive.
    #[default]
    MagnitudePositive,
    /// The value is signed: positive = inflow / addition, negative =
    /// outflow / reduction.
    InflowPositive,
}

impl SignConventionPolicy {
    /// Validate a value against this policy, producing an optional
    /// [`CheckFinding`] describing the violation.
    ///
    /// Returns `None` if the value is consistent with the declared policy.
    /// Returns a `Severity::Warning` finding if the value is finite but
    /// contradicts the declared sign convention. Non-finite values are
    /// out of scope here — they should be caught by data-quality checks
    /// before reaching reconciliation.
    ///
    /// The finding is a *warning* rather than an error so existing
    /// models that ship data with a different sign convention continue
    /// to run; a convention flip that materially affects the
    /// reconciliation still produces a separate reconciliation
    /// finding. Use `rust_decimal::Decimal` boundary conversions to
    /// enforce the convention strictly at ingest.
    #[must_use]
    pub fn validate(
        &self,
        value: f64,
        node_label: &str,
        period: Option<PeriodId>,
        check_id: &str,
    ) -> Option<CheckFinding> {
        if !value.is_finite() {
            return None;
        }
        match self {
            Self::MagnitudePositive => {
                if value < 0.0 {
                    Some(CheckFinding {
                        check_id: check_id.to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "'{node_label}' carries a magnitude-positive convention but \
                             observed {value:.4}; confirm upstream sign convention matches \
                             the reconciliation's expectation"
                        ),
                        period,
                        materiality: None,
                        nodes: Vec::new(),
                        comparison: None,
                        coverage: false,
                    })
                } else {
                    None
                }
            }
            Self::InflowPositive => None,
        }
    }
}

/// Severity level for a check finding, ordered from least to most severe.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
// Distinct from `finstack_quant_core::contract::Severity`, which has no `info`
// level and is published as `common/1/diagnostic.schema.json`.
#[cfg_attr(feature = "json-schema", schemars(rename = "CheckSeverity"))]
pub enum Severity {
    /// Informational finding — no action required.
    #[default]
    Info,
    /// Warning — review recommended.
    Warning,
    /// Error — indicates a likely problem that should be addressed.
    Error,
}

/// Category that groups related checks together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CheckCategory {
    /// Balance sheet balances, retained earnings flow-through, cash ties
    AccountingIdentity,
    /// Depreciation ties to PP&E, interest ties to debt schedule, etc.
    CrossStatementReconciliation,
    /// Growth rate plausibility, effective tax rate range, WC consistency
    InternalConsistency,
    /// Leverage ranges, coverage floors, FCF sign, liquidity runway
    CreditReasonableness,
    /// Missing values, NaN/Inf, sign conventions
    DataQuality,
}

/// Scope that determines which periods a check applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PeriodScope {
    /// Run the check on every period.
    AllPeriods,
    /// Run only on actual (historical) periods.
    ActualsOnly,
    /// Run only on forecast periods.
    ForecastOnly,
}

/// Materiality context attached to a finding, describing its quantitative
/// significance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Materiality {
    /// Absolute amount of the discrepancy.
    pub absolute: f64,
    /// Discrepancy as a decimal fraction of the reference value (0.025 is 2.5%);
    /// `None` when the fraction is undefined (a zero reference) or does not apply.
    pub relative: Option<f64>,
    /// The reference (denominator) value used when computing `relative`.
    pub reference_value: f64,
    /// Human-readable label for the reference (e.g. "total_assets").
    pub reference_label: String,
}

/// The two values a check compared, their difference and the tolerance it
/// allowed.
///
/// Recorded for every numeric comparison a check performs, whether it passed
/// or failed, so the verdict can be reconstructed from exported data: the
/// comparison fails exactly when `|difference| > tolerance`.
///
/// Every value is typed in the units of the compared nodes: exact money in
/// their currency, or a scalar. `difference` is the one exact residual the
/// check computed (an exact sum for money), not a difference of the two
/// recorded sides; `tolerance` is an absolute amount in the same units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CheckComparison {
    /// Stable snake_case name of the identity that was tested (for example
    /// `"balance_sheet_articulation"` or `"cash_flow_components"`); a check
    /// that tests several identities per period uses a distinct label for each.
    pub identity: String,
    /// Period the comparison was evaluated for, if it is period-specific.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "json-schema", schemars(with = "Option<String>"))]
    pub period: Option<PeriodId>,
    /// Observed (left-hand) value, in the compared nodes' units.
    pub actual: AmountOrScalar,
    /// Value the identity requires (right-hand side), in the same units.
    pub expected: AmountOrScalar,
    /// Signed residual `actual - expected`, in the same units.
    pub difference: AmountOrScalar,
    /// Absolute tolerance applied to `|difference|`, in the same units. This
    /// is the effective tolerance: the per-check override when one is
    /// configured, otherwise
    /// `max(default_tolerance, default_relative_tolerance * |reference|)`.
    pub tolerance: AmountOrScalar,
}

impl CheckComparison {
    /// Record a comparison from float views in one unit.
    ///
    /// # Arguments
    ///
    /// * `identity` - Stable snake_case name of the identity tested.
    /// * `period` - Period the comparison belongs to.
    /// * `currency` - Currency of the compared amounts; `None` for scalars.
    /// * `values` - `(actual, expected, tolerance)`, each a finite number in
    ///   the compared units; the difference is `actual - expected`.
    ///
    /// # Errors
    ///
    /// Returns an error when a value is not a representable amount of
    /// `currency`.
    pub fn from_floats(
        identity: impl Into<String>,
        period: PeriodId,
        currency: Option<finstack_quant_core::currency::Currency>,
        (actual, expected, tolerance): (f64, f64, f64),
    ) -> crate::Result<Self> {
        let typed = |value: f64| match currency {
            Some(currency) => AmountOrScalar::amount(value, currency).map_err(crate::Error::from),
            None => Ok(AmountOrScalar::scalar(value)),
        };
        Ok(Self {
            identity: identity.into(),
            period: Some(period),
            actual: typed(actual)?,
            expected: typed(expected)?,
            difference: typed(actual - expected)?,
            tolerance: typed(tolerance)?,
        })
    }

    /// True when `|difference| <= tolerance`, i.e. the comparison passed.
    /// Money is compared exactly; values in different units never pass.
    #[must_use]
    pub fn within_tolerance(&self) -> bool {
        match (self.difference, self.tolerance) {
            (AmountOrScalar::Amount(difference), AmountOrScalar::Amount(tolerance)) => {
                difference.currency() == tolerance.currency()
                    && difference.amount_decimal().abs() <= tolerance.amount_decimal()
            }
            (AmountOrScalar::Scalar(difference), AmountOrScalar::Scalar(tolerance)) => {
                difference.abs() <= tolerance
            }
            _ => false,
        }
    }
}

/// A single finding produced by a check for a specific period or node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CheckFinding {
    /// Identifier of the check that produced this finding.
    pub check_id: String,
    /// Severity of the finding.
    pub severity: Severity,
    /// Human-readable description of the issue.
    pub message: String,
    /// Period the finding relates to, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "json-schema", schemars(with = "Option<String>"))]
    pub period: Option<PeriodId>,
    /// Materiality context, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub materiality: Option<Materiality>,
    /// Node identifiers involved in the finding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<NodeId>,
    /// The failed numeric comparison behind this finding (actual, expected and
    /// tolerance). `None` for findings that do not compare two numbers:
    /// skipped periods, missing or non-finite inputs, sign-convention
    /// warnings, and formula checks without a tolerance (a formula check
    /// with a tolerance compares its value against zero).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<CheckComparison>,
    /// True for a coverage finding: required data was missing or unavailable,
    /// so the check could not evaluate the period or cell it names. A
    /// coverage finding makes its check incomplete and is never removed by
    /// the reporting filters.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub coverage: bool,
}

/// Verdict of a check, or of a whole report, decided from every outcome
/// before any reporting filter is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    /// Work was evaluated, required coverage was complete and nothing failed.
    Pass,
    /// At least one error-severity finding.
    Fail,
    /// Nothing failed, but required data was missing or unavailable for at
    /// least one period or cell.
    Incomplete,
    /// Nothing was evaluated: an empty suite, or a check with no applicable
    /// period or cell.
    Unevaluated,
}

impl CheckOutcome {
    /// Stable snake_case label, identical to the serde wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Incomplete => "incomplete",
            Self::Unevaluated => "unevaluated",
        }
    }
}

/// Outcome of a single check execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CheckResult {
    /// Identifier of the check.
    pub check_id: String,
    /// Human-readable name of the check.
    pub check_name: String,
    /// Category this check belongs to.
    pub category: CheckCategory,
    /// Verdict of the check, decided from all of its findings before the
    /// suite's reporting filters: `fail` on an error-severity finding, else
    /// `incomplete` on a coverage finding, else `unevaluated` when
    /// `evaluated` is zero, else `pass`.
    pub outcome: CheckOutcome,
    /// Number of periods or cells the check examined.
    pub evaluated: usize,
    /// Findings produced by the check. The suite's `min_severity` and
    /// `materiality_threshold` filters remove advisory findings only: an
    /// error-severity or coverage finding is always retained.
    pub findings: Vec<CheckFinding>,
    /// Every numeric comparison the check evaluated, passing and failing
    /// alike, in evaluation order. A passing check therefore still reports
    /// the actual value, expected value and tolerance it was judged on.
    /// Unlike `findings`, this list is not reduced by the suite's
    /// `min_severity` / `materiality_threshold` reporting filters. Empty for
    /// checks that do not compare two numbers, for periods a check skipped,
    /// and for reports serialized before this field existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comparisons: Vec<CheckComparison>,
}

impl CheckResult {
    /// Assemble a check's result and decide its outcome.
    ///
    /// # Arguments
    ///
    /// * `check` - The check that ran; supplies the identifier, display name
    ///   and category.
    /// * `evaluated` - Number of periods or cells the check examined; zero
    ///   makes a check without findings unevaluated.
    /// * `findings` - Every finding the check produced, unfiltered.
    /// * `comparisons` - Every numeric comparison the check evaluated.
    #[must_use]
    pub fn new(
        check: &dyn super::Check,
        evaluated: usize,
        findings: Vec<CheckFinding>,
        comparisons: Vec<CheckComparison>,
    ) -> Self {
        let outcome = if findings.iter().any(|f| f.severity == Severity::Error) {
            CheckOutcome::Fail
        } else if findings.iter().any(|f| f.coverage) {
            CheckOutcome::Incomplete
        } else if evaluated == 0 {
            CheckOutcome::Unevaluated
        } else {
            CheckOutcome::Pass
        };
        Self {
            check_id: check.id().to_string(),
            check_name: check.name().to_string(),
            category: check.category(),
            outcome,
            evaluated,
            findings,
            comparisons,
        }
    }

    /// True when the outcome is [`CheckOutcome::Pass`].
    #[must_use]
    pub fn passed(&self) -> bool {
        self.outcome == CheckOutcome::Pass
    }
}

/// Configuration parameters that govern check execution.
///
/// The identity tolerance is zero unless one is stated: an identity then
/// holds only when its two sides are equal. An explicit tolerance allows
/// `|difference| <= max(default_tolerance, default_relative_tolerance *
/// |reference|)`, where the reference is the identity owner's (total assets
/// for the balance sheet, for example); a difference equal to the allowance
/// is within tolerance. A per-check absolute override replaces both terms.
///
/// `materiality_threshold` and `min_severity` are reporting filters for
/// advisory findings. They never change a verdict, and they never remove an
/// error-severity finding, a coverage finding or a recorded comparison.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CheckConfig {
    /// **Absolute** tolerance for equality comparisons, in the units of the
    /// compared values (0.01 is one cent when nodes are in whole dollars).
    /// Zero, the default, requires exact equality.
    #[serde(default)]
    pub default_tolerance: f64,
    /// **Relative** tolerance as a fraction of the identity's reference
    /// value (1e-9 allows 10 on a reference of 1e10). Zero, the default,
    /// disables the relative term.
    #[serde(default)]
    pub default_relative_tolerance: f64,
    /// Advisory findings whose absolute materiality is below this threshold
    /// are left out of the report. Error-severity and coverage findings are
    /// always reported.
    #[serde(default)]
    pub materiality_threshold: f64,
    /// Minimum severity an advisory finding must have to appear in the
    /// report. Coverage findings are always reported.
    #[serde(default)]
    pub min_severity: Severity,
}

impl CheckConfig {
    /// Validate numeric tolerances and the advisory materiality threshold.
    ///
    /// All three numeric settings must be finite and nonnegative. Zero is
    /// accepted for exact comparisons or to disable a relative/materiality term.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error for a negative, NaN, or infinite setting.
    pub fn validate(&self) -> crate::Result<()> {
        for (name, value) in [
            ("default_tolerance", self.default_tolerance),
            (
                "default_relative_tolerance",
                self.default_relative_tolerance,
            ),
            ("materiality_threshold", self.materiality_threshold),
        ] {
            validate_nonnegative_finite(name, value)?;
        }
        Ok(())
    }
}

pub(crate) fn validate_nonnegative_finite(name: &str, value: f64) -> crate::Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(crate::Error::invalid_input(format!(
            "{name} must be finite and nonnegative; got {value}"
        )));
    }
    Ok(())
}

/// Return the effective tolerance to apply to a diff, given an optional
/// per-check absolute override and a reference magnitude used for the
/// relative tolerance.
///
/// `|diff| > effective_tolerance(...)` means the check should fire.
///
/// A per-check `absolute_override` is the analyst's deliberate choice and is
/// used verbatim. Only the config-default path blends in the relative
/// component, which keeps the *default* scale-aware for the f64 evaluation
/// engine without silently loosening an explicitly configured tolerance.
///
/// Verbatim also means the override forgoes the relative f64-noise cushion:
/// an override tighter than ~1e-9 of the reference magnitude (e.g. `1e-6` on
/// a `1e12` balance) can fire on pure floating-point accumulation noise
/// rather than a real identity break. Size explicit overrides relative to
/// the magnitudes being reconciled.
#[must_use]
pub(crate) fn effective_tolerance(
    config: &CheckConfig,
    absolute_override: Option<f64>,
    reference: f64,
) -> f64 {
    match absolute_override {
        Some(tolerance) => tolerance,
        None => config
            .default_tolerance
            .max(config.default_relative_tolerance * reference.abs()),
    }
}

/// Aggregate counts for a completed check run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CheckSummary {
    /// Total number of checks executed.
    pub total_checks: usize,
    /// Number of checks whose outcome is `pass`.
    pub passed: usize,
    /// Number of checks whose outcome is `fail` (an error-severity finding).
    pub failed: usize,
    /// Number of checks whose outcome is `incomplete` (a coverage finding).
    pub incomplete: usize,
    /// Number of checks whose outcome is `unevaluated` (nothing examined).
    pub unevaluated: usize,
    /// Total number of error-severity findings across all checks.
    pub errors: usize,
    /// Total number of warning-severity findings across all checks.
    pub warnings: usize,
    /// Total number of info-severity findings across all checks.
    pub infos: usize,
}

/// Full report aggregating all [`CheckResult`]s from a check run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CheckReport {
    /// Verdict of the suite from the unfiltered check outcomes: `fail` when
    /// any check failed, else `incomplete` when any check is incomplete,
    /// else `pass` when at least one check passed, else `unevaluated` (an
    /// empty suite, or one with no applicable check).
    pub outcome: CheckOutcome,
    /// Individual results for each check, in declared order.
    pub results: Vec<CheckResult>,
    /// Aggregate summary.
    pub summary: CheckSummary,
}

impl CheckReport {
    /// Return all findings matching the given severity.
    pub fn findings_by_severity(&self, severity: Severity) -> Vec<&CheckFinding> {
        self.results
            .iter()
            .flat_map(|r| &r.findings)
            .filter(|f| f.severity == severity)
            .collect()
    }

    /// True if the report contains at least one error-severity finding.
    pub fn has_errors(&self) -> bool {
        self.summary.errors > 0
    }

    /// True if the report contains at least one warning-severity finding.
    pub fn has_warnings(&self) -> bool {
        self.summary.warnings > 0
    }

    /// Number of retained findings across all checks.
    ///
    /// Counts the findings that survived the suite's `min_severity` and
    /// `materiality_threshold` reporting filters, read from `results`, so it
    /// equals the number of findings a host lists for the report (and the
    /// summary's `errors + warnings + infos` for a report produced by a run).
    #[must_use]
    pub fn total_findings(&self) -> usize {
        self.results
            .iter()
            .map(|result| result.findings.len())
            .sum()
    }
}
