//! Python wrappers for the financial statement checks framework.

use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::pandas_utils::{serde_to_py, ColumnSchema};
use crate::errors::{serde_json_to_py, statements_to_py, value_error};
use finstack_quant_statements::checks::{
    BuiltinCheckSpec, CheckCategory, CheckConfig, CheckFinding, FormulaCheckSpec, Severity,
};
use pyo3::prelude::*;

/// Columns emitted by [`PyCheckReport::to_dataframe`].
const CHECK_RESULT_COLUMNS: [ColumnSchema<'static>; 5] = [
    ("check_id", "str"),
    ("check_name", "str"),
    ("category", "str"),
    ("passed", "bool"),
    ("finding_count", "int64"),
];

/// Columns emitted by [`PyCheckReport::to_findings_dataframe`].
const CHECK_FINDING_COLUMNS: [ColumnSchema<'static>; 11] = [
    ("check_id", "str"),
    ("check_name", "str"),
    ("category", "str"),
    ("severity", "str"),
    ("message", "str"),
    ("period", "str"),
    ("nodes", "str"),
    ("materiality_absolute", "float64"),
    ("materiality_relative_pct", "float64"),
    ("materiality_reference_value", "float64"),
    ("materiality_reference_label", "str"),
];

/// Parse a severity discriminant (``"info"`` / ``"warning"`` / ``"error"``).
fn parse_severity(severity: &str) -> PyResult<Severity> {
    finstack_quant_core::wire::serde_parse(severity).map_err(|e| {
        value_error(format!(
            "invalid severity {severity:?}: {e}; expected info, warning, or error"
        ))
    })
}

/// Parse a check category discriminant.
fn parse_category(category: &str) -> PyResult<CheckCategory> {
    finstack_quant_core::wire::serde_parse(category).map_err(|e| {
        value_error(format!(
            "invalid check category {category:?}: {e}; expected accounting_identity, \
             cross_statement_reconciliation, internal_consistency, credit_reasonableness, \
             or data_quality"
        ))
    })
}

/// Configuration governing how a check suite filters and tolerates findings.
///
/// Identity checks fire when
/// ``|diff| > max(default_tolerance, default_relative_tolerance * |reference|)``:
/// an absolute floor in currency units plus a relative ceiling that scales
/// with balance-sheet size. Materiality and minimum severity are reporting
/// filters that never change a check's pass/fail verdict.
#[pyclass(
    name = "CheckConfig",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCheckConfig {
    pub(super) inner: CheckConfig,
}

#[pymethods]
impl PyCheckConfig {
    /// Build a check configuration.
    ///
    /// Parameters
    /// ----------
    /// default_tolerance : float, default 0.01
    ///     Absolute equality tolerance in the compared nodes' own units
    ///     (one cent when nodes are in whole dollars); finite and non-negative.
    /// default_relative_tolerance : float, default 1e-9
    ///     Relative tolerance as a decimal fraction of the reference
    ///     magnitude; finite and non-negative, with zero disabling the relative component.
    /// materiality_threshold : float, default 0.0
    ///     Advisory (info/warning) findings whose absolute materiality is
    ///     below this amount are dropped from reports. Error findings are
    ///     always retained. Must be finite and non-negative.
    /// min_severity : str, default "info"
    ///     Lowest severity retained in reports: ``"info"``, ``"warning"``,
    ///     or ``"error"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``min_severity`` is unknown or a tolerance or materiality
    ///     threshold is negative or non-finite.
    #[new]
    #[pyo3(
        signature = (default_tolerance=0.01, default_relative_tolerance=1e-9, materiality_threshold=0.0, min_severity="info"),
        text_signature = "(default_tolerance=0.01, default_relative_tolerance=1e-9, materiality_threshold=0.0, min_severity='info')"
    )]
    fn new(
        default_tolerance: f64,
        default_relative_tolerance: f64,
        materiality_threshold: f64,
        min_severity: &str,
    ) -> PyResult<Self> {
        let inner = CheckConfig {
            default_tolerance,
            default_relative_tolerance,
            materiality_threshold,
            min_severity: parse_severity(min_severity)?,
        };
        inner.validate().map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` via the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a configuration from canonical JSON (unknown fields are
    /// rejected; omitted fields take their defaults). Negative or non-finite
    /// tolerances and materiality thresholds raise ``ValueError``.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CheckConfig = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CheckConfig JSON"))?;
        inner.validate().map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Validate finite, non-negative tolerances and materiality thresholds.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any configured tolerance or materiality threshold is negative
    ///     or non-finite.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(statements_to_py)
    }

    /// Serialize this configuration to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CheckConfig"))
    }

    /// Absolute equality tolerance in the compared nodes' units.
    #[getter]
    fn default_tolerance(&self) -> f64 {
        self.inner.default_tolerance
    }

    /// Relative tolerance as a decimal fraction of the reference magnitude.
    #[getter]
    fn default_relative_tolerance(&self) -> f64 {
        self.inner.default_relative_tolerance
    }

    /// Absolute materiality floor for advisory findings.
    #[getter]
    fn materiality_threshold(&self) -> f64 {
        self.inner.materiality_threshold
    }

    /// Lowest retained severity name (``"info"``, ``"warning"``, ``"error"``).
    #[getter]
    fn min_severity(&self) -> String {
        crate::bindings::statements_analytics::serde_variant_str(&self.inner.min_severity)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CheckConfig", &self.inner)
    }
}

/// User-defined formula check: a DSL predicate or residual evaluated per period.
///
/// Without a tolerance, a finding is produced when the formula evaluates to
/// false (zero). With a tolerance, it fires when the formula's absolute
/// residual exceeds the bound. Non-finite outputs always produce a finding;
/// ``{period}`` in ``message_template`` is replaced with the period id at run time.
#[pyclass(
    name = "FormulaCheckSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFormulaCheckSpec {
    pub(super) inner: FormulaCheckSpec,
}

#[pymethods]
impl PyFormulaCheckSpec {
    /// Build a formula check.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique identifier for this check instance.
    /// name : str
    ///     Human-readable name shown in reports.
    /// formula : str
    ///     Statements DSL predicate (``"revenue > 0"``,
    ///     ``"cs.debt_balance.total <= 5 * ebitda"``). Uses the same
    ///     evaluator as calculated nodes, including time-series functions.
    /// message_template : str
    ///     Finding message; ``{period}`` is replaced at run time.
    /// category : str, default "internal_consistency"
    ///     One of ``accounting_identity``, ``cross_statement_reconciliation``,
    ///     ``internal_consistency``, ``credit_reasonableness``,
    ///     ``data_quality``.
    /// severity : str, default "error"
    ///     Severity of findings: ``"info"``, ``"warning"`` or ``"error"``.
    ///     Only error findings fail a report.
    /// tolerance : float | None
    ///     Optional finite, non-negative absolute residual bound in the formula's
    ///     result units. With ``None``, finite nonzero predicate outputs pass;
    ///     with a bound, absolute outputs at or below it pass. Validated at execution.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``category`` or ``severity`` is not a known discriminant.
    ///     Formula syntax is validated when the suite runs against a model.
    #[new]
    #[pyo3(
        signature = (id, name, formula, message_template, category="internal_consistency", severity="error", tolerance=None),
        text_signature = "(id, name, formula, message_template, category='internal_consistency', severity='error', tolerance=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: String,
        name: String,
        formula: String,
        message_template: String,
        category: &str,
        severity: &str,
        tolerance: Option<f64>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FormulaCheckSpec {
                id,
                name,
                category: parse_category(category)?,
                severity: parse_severity(severity)?,
                formula,
                message_template,
                tolerance,
            },
        })
    }

    /// Support `pickle` via the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a formula check from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid FormulaCheckSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this check to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize FormulaCheckSpec"))
    }

    /// Check identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Human-readable name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// DSL predicate evaluated per period.
    #[getter]
    fn formula(&self) -> &str {
        &self.inner.formula
    }

    /// Finding message template (``{period}`` placeholder).
    #[getter]
    fn message_template(&self) -> &str {
        &self.inner.message_template
    }

    /// Category discriminant (``"internal_consistency"``, ...).
    #[getter]
    fn category(&self) -> String {
        crate::bindings::statements_analytics::serde_variant_str(&self.inner.category)
    }

    /// Severity discriminant (``"info"``, ``"warning"``, ``"error"``).
    #[getter]
    fn severity(&self) -> String {
        crate::bindings::statements_analytics::serde_variant_str(&self.inner.severity)
    }

    /// Optional absolute tolerance, or ``None``.
    #[getter]
    fn tolerance(&self) -> Option<f64> {
        self.inner.tolerance
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("FormulaCheckSpec", &self.inner)
    }
}

/// A serializable suite specification describing which checks to run.
#[pyclass(
    name = "CheckSuiteSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCheckSuiteSpec {
    pub(super) inner: finstack_quant_statements::checks::CheckSuiteSpec,
}

#[pymethods]
impl PyCheckSuiteSpec {
    /// Build a check suite specification.
    ///
    /// Parameters
    /// ----------
    /// name : str
    ///     Suite name for display and logging.
    /// builtin_checks : list[str | dict] | None
    ///     Built-in checks to run. A bare name is enough for checks whose
    ///     fields all default (``"non_finite"``, ``"sign_convention"``); the
    ///     others need a dict with the check's ``type`` and node fields, e.g.
    ///     ``{"type": "balance_sheet_articulation", "assets_nodes":
    ///     ["assets"], "liabilities_nodes": ["liabilities"],
    ///     "equity_nodes": ["equity"]}``. See
    ///     :meth:`builtin_check_names` for the catalog.
    /// formula_checks : list[FormulaCheckSpec] | None
    ///     User-defined DSL predicate checks.
    /// config : CheckConfig | None
    ///     Tolerance and reporting filters; ``None`` uses the defaults.
    /// description : str | None
    ///     Optional suite description.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a built-in check name is unknown, a dict lacks a required
    ///     field, an entry is neither ``str`` nor ``dict``, or a configured
    ///     tolerance or materiality threshold is negative or non-finite.
    #[new]
    #[pyo3(
        signature = (name, builtin_checks=None, formula_checks=None, config=None, description=None),
        text_signature = "(name, builtin_checks=None, formula_checks=None, config=None, description=None)"
    )]
    fn new(
        py: Python<'_>,
        name: String,
        builtin_checks: Option<Vec<Bound<'_, PyAny>>>,
        formula_checks: Option<Vec<PyRef<'_, PyFormulaCheckSpec>>>,
        config: Option<PyRef<'_, PyCheckConfig>>,
        description: Option<String>,
    ) -> PyResult<Self> {
        let builtin_checks = builtin_checks
            .unwrap_or_default()
            .iter()
            .map(|entry| parse_builtin_check(py, entry))
            .collect::<PyResult<Vec<_>>>()?;
        let formula_checks = formula_checks
            .unwrap_or_default()
            .iter()
            .map(|check| check.inner.clone())
            .collect();
        let inner = finstack_quant_statements::checks::CheckSuiteSpec {
            name,
            description,
            builtin_checks,
            formula_checks,
            config: config.map(|c| c.inner.clone()).unwrap_or_default(),
        };
        inner.config.validate().map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Names of every built-in check, usable in ``builtin_checks``.
    ///
    /// Returns
    /// -------
    /// list[str]
    ///     ``["balance_sheet_articulation",
    ///     "retained_earnings_reconciliation", "cash_reconciliation",
    ///     "missing_value", "sign_convention", "non_finite"]``.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builtin_check_names() -> Vec<&'static str> {
        BuiltinCheckSpec::names().to_vec()
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a suite spec from a JSON string.
    ///
    /// Unknown fields are rejected, so a team-wide check policy that drifts
    /// from the schema fails at load rather than silently running fewer
    /// checks than intended. Negative or non-finite configuration tolerances
    /// and materiality thresholds raise ``ValueError``. Formula syntax and
    /// node references are validated later, when the suite runs against a model.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_statements::checks::CheckSuiteSpec =
            serde_json::from_str(json)
                .map_err(|e| serde_json_to_py(e, "invalid CheckSuiteSpec JSON"))?;
        inner.config.validate().map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this suite spec to compact JSON.
    ///
    /// The output is the canonical policy format — check it into version
    /// control to pin a team's validation rules and tolerances.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CheckSuiteSpec"))
    }

    /// Suite name, used for display and logging.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Suite description, or ``None``.
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    /// Built-in checks in their JSON shape (``{"type": ..., ...}`` dicts).
    #[getter]
    fn builtin_checks<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.builtin_checks)
    }

    /// User-defined formula checks.
    #[getter]
    fn formula_checks(&self) -> Vec<PyFormulaCheckSpec> {
        self.inner
            .formula_checks
            .iter()
            .cloned()
            .map(|inner| PyFormulaCheckSpec { inner })
            .collect()
    }

    /// Tolerance and reporting configuration.
    #[getter]
    fn config(&self) -> PyCheckConfig {
        PyCheckConfig {
            inner: self.inner.config.clone(),
        }
    }

    /// Number of built-in checks the spec will materialize.
    ///
    /// Built-ins are the crate-provided accounting-identity, reconciliation,
    /// and data-quality checks, selected by name in the spec.
    #[getter]
    fn builtin_check_count(&self) -> usize {
        self.inner.builtin_checks.len()
    }

    /// Number of user-defined formula checks the spec will materialize.
    ///
    /// Formula checks are DSL expressions evaluated per period; their syntax
    /// is validated when the suite runs, not when the spec is loaded.
    #[getter]
    fn formula_check_count(&self) -> usize {
        self.inner.formula_checks.len()
    }

    /// Return the representation with the name and check counts.
    fn __repr__(&self) -> String {
        format!(
            "CheckSuiteSpec(name={:?}, builtins={}, formulas={})",
            self.inner.name,
            self.inner.builtin_checks.len(),
            self.inner.formula_checks.len(),
        )
    }
}

/// Parse one ``builtin_checks`` entry: a bare name or a ``{"type": ...}`` dict.
fn parse_builtin_check(py: Python<'_>, entry: &Bound<'_, PyAny>) -> PyResult<BuiltinCheckSpec> {
    let value = if let Ok(name) = entry.extract::<String>() {
        serde_json::json!({ "type": name })
    } else if entry.is_instance_of::<pyo3::types::PyDict>() {
        crate::bindings::module_utils::py_to_json_value(py, entry, "builtin check")?
    } else {
        return Err(value_error(format!(
            "builtin_checks entries must be a check name or a dict with a 'type' key, got {}",
            entry.get_type().name()?
        )));
    };
    serde_json::from_value(value).map_err(|e| {
        value_error(format!(
            "invalid builtin check: {e}; known checks: {}",
            BuiltinCheckSpec::names().join(", ")
        ))
    })
}

/// A single finding produced by a check for a specific period or node.
#[pyclass(
    name = "CheckFinding",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCheckFinding {
    inner: CheckFinding,
}

#[pymethods]
impl PyCheckFinding {
    /// Support `pickle` via the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a finding from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CheckFinding JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this finding to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CheckFinding"))
    }

    /// Identifier of the check that produced this finding.
    #[getter]
    fn check_id(&self) -> &str {
        &self.inner.check_id
    }

    /// Severity discriminant (``"info"``, ``"warning"``, ``"error"``).
    #[getter]
    fn severity(&self) -> String {
        crate::bindings::statements_analytics::serde_variant_str(&self.inner.severity)
    }

    /// Human-readable description of the issue.
    #[getter]
    fn message(&self) -> &str {
        &self.inner.message
    }

    /// Period identifier the finding relates to (``"2025Q1"``), or ``None``
    /// when it is not period-specific.
    #[getter]
    fn period(&self) -> Option<String> {
        self.inner.period.map(|p| p.to_string())
    }

    /// Node identifiers involved in the finding.
    #[getter]
    fn nodes(&self) -> Vec<String> {
        self.inner.nodes.iter().map(ToString::to_string).collect()
    }

    /// Absolute size of the discrepancy in the compared nodes' own units,
    /// or ``None`` when the finding carries no materiality context.
    #[getter]
    fn materiality_absolute(&self) -> Option<f64> {
        self.inner.materiality.as_ref().map(|m| m.absolute)
    }

    /// Discrepancy as a **percentage** (already multiplied by 100) of the
    /// reference value, or ``None``.
    #[getter]
    fn materiality_relative_pct(&self) -> Option<f64> {
        self.inner.materiality.as_ref().map(|m| m.relative_pct)
    }

    /// Reference (denominator) value used for the relative measure, or
    /// ``None``.
    #[getter]
    fn materiality_reference_value(&self) -> Option<f64> {
        self.inner.materiality.as_ref().map(|m| m.reference_value)
    }

    /// Label of the reference value (e.g. ``"total_assets"``), or ``None``.
    #[getter]
    fn materiality_reference_label(&self) -> Option<String> {
        self.inner
            .materiality
            .as_ref()
            .map(|m| m.reference_label.clone())
    }

    /// Return ``CheckFinding(check_id=..., severity=..., period=..., message=...)``.
    fn __repr__(&self) -> String {
        format!(
            "CheckFinding(check_id={:?}, severity={:?}, period={}, message={:?})",
            self.inner.check_id,
            self.severity(),
            self.period()
                .map_or_else(|| "None".to_string(), |p| format!("{p:?}")),
            self.inner.message,
        )
    }
}

/// Validation check report aggregating results and summary statistics.
#[pyclass(
    name = "CheckReport",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCheckReport {
    pub(crate) inner: finstack_quant_statements::checks::CheckReport,
}

impl PyCheckReport {
    pub(crate) fn from_inner(inner: finstack_quant_statements::checks::CheckReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCheckReport {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a check report from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_statements::checks::CheckReport = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CheckReport JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this report to compact JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CheckReport"))
    }

    /// Whether the whole report passed: no error-severity finding was
    /// retained by any check.
    ///
    /// Warnings and info findings do not fail a report. A suite's
    /// materiality threshold never suppresses error findings, so it cannot
    /// flip this to ``True``. Delegates to the canonical Rust
    /// ``CheckReport::has_errors``.
    #[getter]
    fn passed(&self) -> bool {
        !self.inner.has_errors()
    }

    /// Number of checks that ran, one per row of :meth:`to_dataframe`.
    ///
    /// Reads the canonical ``CheckSummary.total_checks`` counter.
    #[getter]
    fn total_checks(&self) -> usize {
        self.inner.summary.total_checks
    }

    /// Whether the report contains at least one error-severity finding.
    ///
    /// Delegates to the canonical Rust ``CheckReport::has_errors``.
    #[pyo3(text_signature = "($self)")]
    fn has_errors(&self) -> bool {
        self.inner.has_errors()
    }

    /// Whether the report contains at least one warning-severity finding.
    ///
    /// Delegates to the canonical Rust ``CheckReport::has_warnings``.
    #[pyo3(text_signature = "($self)")]
    fn has_warnings(&self) -> bool {
        self.inner.has_warnings()
    }

    /// Return all retained findings of one severity.
    ///
    /// Delegates to the canonical Rust ``CheckReport::findings_by_severity``.
    ///
    /// Parameters
    /// ----------
    /// severity : str
    ///     One of ``"info"``, ``"warning"``, ``"error"`` (the canonical
    ///     snake_case severity discriminants).
    ///
    /// Returns
    /// -------
    /// list[CheckFinding]
    ///     One typed finding per matching retained finding, in check order.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``severity`` is not a canonical severity discriminant.
    #[pyo3(text_signature = "($self, severity)")]
    fn findings_by_severity(&self, severity: &str) -> PyResult<Vec<PyCheckFinding>> {
        let severity = parse_severity(severity)?;
        Ok(self
            .inner
            .findings_by_severity(severity)
            .into_iter()
            .map(|finding| PyCheckFinding {
                inner: finding.clone(),
            })
            .collect())
    }

    /// Every retained finding across all checks, in check order.
    #[getter]
    fn findings(&self) -> Vec<PyCheckFinding> {
        self.inner
            .results
            .iter()
            .flat_map(|result| result.findings.iter())
            .map(|finding| PyCheckFinding {
                inner: finding.clone(),
            })
            .collect()
    }

    /// Number of retained findings across all checks.
    ///
    /// Counts what survived the suite's ``min_severity`` and
    /// ``materiality_threshold`` reporting filters, not every raw diagnostic
    /// the checks produced — it matches the row count of
    /// :meth:`to_findings_dataframe`.
    #[getter]
    fn total_findings(&self) -> usize {
        self.inner.summary.errors + self.inner.summary.warnings + self.inner.summary.infos
    }

    /// Number of retained error-severity findings across all checks.
    #[getter]
    fn total_errors(&self) -> usize {
        self.inner.summary.errors
    }

    /// Number of retained warning-severity findings across all checks.
    #[getter]
    fn total_warnings(&self) -> usize {
        self.inner.summary.warnings
    }

    /// Export one row per executed check as a pandas ``DataFrame``.
    ///
    /// Columns: ``check_id`` (stable identifier), ``check_name``
    /// (human-readable name), ``category`` (snake_case group, one of
    /// ``accounting_identity``, ``cross_statement_reconciliation``,
    /// ``internal_consistency``, ``credit_reasonableness``,
    /// ``data_quality``), ``passed`` (``True`` when the check retained no
    /// error-severity finding), ``finding_count`` (retained findings for this
    /// check, after the suite's reporting filters).
    ///
    /// One row per check result, so the row count equals
    /// :attr:`total_checks`. A report with no checks still yields the
    /// documented columns with zero rows. Per-finding detail — severity,
    /// message, and the observed-vs-reference materiality — lives in
    /// :meth:`to_findings_dataframe`.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .results
            .iter()
            .map(|result| {
                serde_json::json!({
                    "check_id": result.check_id,
                    "check_name": result.check_name,
                    "category": result.category,
                    "passed": result.passed,
                    "finding_count": result.findings.len(),
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &CHECK_RESULT_COLUMNS)
    }

    /// Export one row per retained finding as a pandas ``DataFrame``.
    ///
    /// Columns: ``check_id``, ``check_name``, ``category`` (as in
    /// :meth:`to_dataframe`), ``severity`` (``info``, ``warning`` or
    /// ``error``), ``message`` (human-readable description), ``period``
    /// (period identifier string such as ``"2025Q1"``, ``None`` when the
    /// finding is not period-specific), ``nodes`` (comma-joined node
    /// identifiers involved, empty string when none),
    /// ``materiality_absolute`` (size of the discrepancy in the compared
    /// nodes' own units — currency amounts for monetary nodes),
    /// ``materiality_relative_pct`` (that discrepancy as a **percentage** of
    /// the reference value, i.e. already multiplied by 100, unlike the
    /// decimal-fraction rates used elsewhere in this module),
    /// ``materiality_reference_value`` (the denominator used) and
    /// ``materiality_reference_label`` (what that denominator is, e.g.
    /// ``total_assets``).
    ///
    /// The four materiality columns are ``None`` for findings that carry no
    /// materiality context. The row count equals :attr:`total_findings`; a
    /// report with no findings still yields the documented columns with zero
    /// rows.
    #[pyo3(text_signature = "($self)")]
    fn to_findings_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .results
            .iter()
            .flat_map(|result| {
                result.findings.iter().map(move |finding| {
                    let materiality = finding.materiality.as_ref();
                    serde_json::json!({
                        "check_id": finding.check_id,
                        "check_name": result.check_name,
                        "category": result.category,
                        "severity": finding.severity,
                        "message": finding.message,
                        "period": finding.period.map(|p| p.to_string()),
                        "nodes": finding
                            .nodes
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                        "materiality_absolute": materiality.map(|m| m.absolute),
                        "materiality_relative_pct": materiality.map(|m| m.relative_pct),
                        "materiality_reference_value": materiality.map(|m| m.reference_value),
                        "materiality_reference_label": materiality
                            .map(|m| m.reference_label.clone()),
                    })
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &CHECK_FINDING_COLUMNS)
    }

    /// Return the representation with check, error and warning counts.
    fn __repr__(&self) -> String {
        format!(
            "CheckReport(checks={}, passed={}, errors={}, warnings={})",
            self.inner.summary.total_checks,
            if self.inner.has_errors() {
                "False"
            } else {
                "True"
            },
            self.inner.summary.errors,
            self.inner.summary.warnings,
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Register check types on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCheckConfig>()?;
    m.add_class::<PyFormulaCheckSpec>()?;
    m.add_class::<PyCheckSuiteSpec>()?;
    m.add_class::<PyCheckFinding>()?;
    m.add_class::<PyCheckReport>()?;
    Ok(())
}
