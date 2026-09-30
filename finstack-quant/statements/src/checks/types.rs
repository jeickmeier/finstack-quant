//! Core types for the financial statement checks framework.

use finstack_quant_core::dates::PeriodId;
use serde::{Deserialize, Serialize};

use crate::types::NodeId;

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
    /// Discrepancy as a percentage of the reference value.
    pub relative_pct: f64,
    /// The reference (denominator) value used when computing `relative_pct`.
    pub reference_value: f64,
    /// Human-readable label for the reference (e.g. "total_assets").
    pub reference_label: String,
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
    /// Whether the check passed (no error-severity findings).
    pub passed: bool,
    /// Individual findings produced by the check.
    pub findings: Vec<CheckFinding>,
}

fn default_check_tolerance() -> f64 {
    0.01
}

fn default_relative_tolerance() -> f64 {
    // Statement evaluation runs in f64 (see `NumericMode::Float64`), so
    // identity checks accumulate rounding proportional to the magnitudes
    // involved. A flat absolute default alone (one cent) false-positives on
    // large balance sheets; 1e-9 relative keeps the effective tolerance at
    // f64-noise scale (e.g. $10 on a $10B sheet) without loosening small ones,
    // where the absolute floor still dominates.
    1e-9
}

/// Configuration parameters that govern check execution.
///
/// Identity checks trigger a finding when
/// `|diff| > max(default_tolerance, default_relative_tolerance * |reference|)`,
/// so an analyst can set an absolute floor (currency units) that catches
/// micro-errors on small balances plus a relative ceiling that scales with
/// larger balance sheets. A per-check tolerance override bypasses both
/// defaults and is applied verbatim.
///
/// All four fields are part of the public configuration surface and are
/// intended to be tuned per-deployment via JSON / serde overrides — e.g.,
/// raising `default_relative_tolerance` to `1e-4` (1 bp) when a downstream
/// process is known to round to the nearest hundred. The defaults
/// (`default_tolerance = 0.01`, `default_relative_tolerance = 1e-9`,
/// `materiality_threshold = 0.0`, `min_severity = Info`) are conservative:
/// the absolute cent floor catches micro-errors on small balances while the
/// relative component absorbs f64 accumulation noise on large ones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckConfig {
    /// Default **absolute** tolerance for equality comparisons, expressed in
    /// the same currency units as the node values being compared (e.g. 0.01
    /// means one cent when nodes are in whole dollars).
    #[serde(default = "default_check_tolerance")]
    pub default_tolerance: f64,
    /// Default **relative** tolerance as a fraction of the reference
    /// denominator (e.g. 1e-6 ≈ "one basis point of a basis point"). Zero
    /// disables relative tolerance.
    #[serde(default = "default_relative_tolerance")]
    pub default_relative_tolerance: f64,
    /// Findings below this absolute materiality threshold are excluded from
    /// reports. Error-severity findings are exempt: materiality filters
    /// advisory noise, it never changes a check's pass/fail verdict.
    #[serde(default)]
    pub materiality_threshold: f64,
    /// Minimum severity a finding must have to appear in the report.
    #[serde(default)]
    pub min_severity: Severity,
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            default_tolerance: 0.01,
            default_relative_tolerance: default_relative_tolerance(),
            materiality_threshold: 0.0,
            min_severity: Severity::Info,
        }
    }
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
    /// Number of checks that passed.
    pub passed: usize,
    /// Number of checks that failed (at least one error-severity finding).
    pub failed: usize,
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
    /// Individual results for each check.
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
