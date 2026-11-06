//! `CovenantEngine` and `CovenantBreach` wrappers plus `CovenantTemplate` and
//! its expansion into typed specs.

use super::report::PyCovenantReport;
use super::spec::{PyCovenantSpec, PyCovenantWaiver};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pandas_utils::{ColumnSchema, serde_rows_to_dataframe_with_schema};
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, display_to_py, value_error};
use finstack_quant_covenants::templates::{
    CovenantTemplate, covenant_template as covenant_template_rs,
};
use finstack_quant_covenants::{
    CovenantBreach, CovenantEngine, DatedCovenantReports, DatedMetrics, HashMapMetricSource,
};
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Accept a ``dict[str, float]`` or a JSON-object string of metric values.
///
/// A string goes to the Rust metric-map parser
/// ([`HashMapMetricSource::from_json`]); a dict is converted here. Every dict
/// value must be a real number; ``bool`` is rejected so a stray ``True``
/// cannot silently evaluate as ``1.0``.
pub(crate) fn extract_metrics(obj: &Bound<'_, PyAny>) -> PyResult<HashMapMetricSource> {
    if let Ok(text) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return HashMapMetricSource::from_json(&text).map_err(core_to_py);
    }
    extract_metric_dict(obj).map(HashMapMetricSource::from_pairs)
}

/// Convert a ``dict[str, float]`` of metric values into pairs.
pub(crate) fn extract_metric_dict(obj: &Bound<'_, PyAny>) -> PyResult<Vec<(String, f64)>> {
    let dict = obj.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "metrics must be a dict[str, float] or a JSON object string, got {}",
            obj.get_type()
                .name()
                .map_or_else(|_| "?".to_string(), |n| n.to_string())
        ))
    })?;
    let mut pairs = Vec::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        let key: String = key
            .extract()
            .map_err(|_| value_error("metric keys must be strings"))?;
        if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(value_error(format!(
                "Metric '{key}' must be a number, got bool"
            )));
        }
        let number: f64 = value
            .extract()
            .map_err(|_| value_error(format!("Metric '{key}' must be a number")))?;
        pairs.push((key, number));
    }
    Ok(pairs)
}

/// Build an insertion-ordered ``dict[str, CovenantReport]`` from the engine's
/// ``IndexMap`` so the Python dict follows spec order.
pub(crate) fn reports_to_pydict<'py>(
    py: Python<'py>,
    reports: IndexMap<String, finstack_quant_covenants::CovenantReport>,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    for (key, inner) in reports {
        dict.set_item(key, PyCovenantReport { inner })?;
    }
    Ok(dict)
}

/// Columns of the frame produced by `CovenantEngine.evaluate_series`.
const SERIES_COLUMNS: [ColumnSchema<'static>; 8] = [
    ("as_of", "str"),
    ("covenant", "str"),
    ("covenant_type", "str"),
    ("passed", "bool"),
    ("actual_value", "float64"),
    ("threshold", "float64"),
    ("headroom", "float64"),
    ("details", "str"),
];

/// Flatten dated covenant reports into the long frame of `SERIES_COLUMNS`:
/// one row per (test date, covenant), dates in series order and covenants in
/// engine order. Shared by `CovenantEngine.evaluate_series` and
/// `statements_analytics.evaluate_covenants`.
pub(crate) fn dated_reports_frame(
    py: Python<'_>,
    series: Vec<DatedCovenantReports>,
) -> PyResult<Bound<'_, PyAny>> {
    let records: Vec<serde_json::Value> = series
        .into_iter()
        .flat_map(|dated| {
            let as_of = dated.as_of.to_string();
            dated.reports.into_iter().map(move |(key, report)| {
                serde_json::json!({
                    "as_of": as_of,
                    "covenant": key,
                    "covenant_type": report.covenant_type,
                    "passed": report.passed,
                    "actual_value": report.actual_value,
                    "threshold": report.threshold,
                    "headroom": report.headroom,
                    "details": report.details,
                })
            })
        })
        .collect();
    serde_rows_to_dataframe_with_schema(py, &records, &SERIES_COLUMNS)
}

/// Read a date-indexed metrics frame into Rust `DatedMetrics` rows.
///
/// Cells are preserved for canonical Rust validation. Duplicate metric
/// columns are rejected rather than silently overwriting observations.
pub(crate) fn extract_metric_frame(frame: &Bound<'_, PyAny>) -> PyResult<Vec<DatedMetrics>> {
    let columns: Vec<String> = frame
        .getattr("columns")?
        .call_method0("tolist")?
        .extract()
        .map_err(|_| value_error("metrics frame columns must be metric-id strings"))?;
    let unique: std::collections::HashSet<_> = columns.iter().collect();
    if unique.len() != columns.len() {
        return Err(value_error(
            "metrics frame contains duplicate metric columns",
        ));
    }
    let index = frame.getattr("index")?.call_method0("tolist")?;
    let dates = index
        .try_iter()?
        .map(|item| py_to_date(&item?))
        .collect::<PyResult<Vec<_>>>()?;
    let values: Vec<Vec<f64>> = frame
        .call_method1("astype", ("float64",))?
        .call_method0("to_numpy")?
        .call_method0("tolist")?
        .extract()
        .map_err(|_| value_error("metrics frame values must be numeric"))?;
    if values.len() != dates.len() {
        return Err(value_error(
            "metrics frame index and values disagree in length",
        ));
    }
    Ok(dates
        .into_iter()
        .zip(values)
        .map(|(date, row)| DatedMetrics {
            date,
            metrics: columns.iter().cloned().zip(row).collect(),
        })
        .collect())
}

/// Covenant package: specifications, waivers, and the breach history
/// accumulated by ``evaluate_and_track``.
///
/// Build it empty and chain ``add_spec`` / ``add_waiver``, or use
/// ``CovenantEngine.from_specs(covenant_template(CovenantTemplate.lbo_standard(...)))``. Evaluate with a
/// ``dict[str, float]`` of metric values keyed by metric id; ratios are in
/// turns (``4.5`` means 4.5x) and amounts share the caller's reporting
/// currency. Serialize with ``to_json`` — only ``specs`` is required on the
/// wire; ``breach_history``, ``windows`` and ``waivers`` default to empty.
#[pyclass(
    name = "CovenantEngine",
    module = "finstack_quant.covenants",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantEngine {
    pub(crate) inner: CovenantEngine,
}

impl PyCovenantEngine {
    pub(crate) fn from_inner(inner: CovenantEngine) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenantEngine {
    /// Create an empty engine.
    #[new]
    #[pyo3(text_signature = "()")]
    fn new() -> Self {
        Self::from_inner(CovenantEngine::new())
    }

    /// Create an engine holding ``specs`` (for example a template package).
    #[staticmethod]
    #[pyo3(text_signature = "(specs)")]
    fn from_specs(specs: Vec<PyRef<'_, PyCovenantSpec>>) -> Self {
        let mut inner = CovenantEngine::new();
        for spec in specs {
            inner.add_spec(spec.inner.clone());
        }
        Self::from_inner(inner)
    }

    /// Append ``spec`` and return this engine for chaining.
    #[pyo3(text_signature = "(spec)")]
    fn add_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        spec: PyRef<'_, PyCovenantSpec>,
    ) -> PyRefMut<'py, Self> {
        slf.inner.add_spec(spec.inner.clone());
        slf
    }

    /// Record ``waiver`` and return this engine for chaining.
    #[pyo3(text_signature = "(waiver)")]
    fn add_waiver<'py>(
        mut slf: PyRefMut<'py, Self>,
        waiver: PyRef<'_, PyCovenantWaiver>,
    ) -> PyRefMut<'py, Self> {
        slf.inner.add_waiver(waiver.inner.clone());
        slf
    }

    /// Validate specs, waivers and windows without evaluating.
    ///
    /// Raises ``ValueError`` for a non-finite threshold, a negative cure
    /// period, a waiver expiring before it takes effect, or overlapping
    /// windows.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Evaluate every applicable covenant on ``as_of``.
    ///
    /// ``metrics`` is a ``dict[str, float]`` (or JSON object string) keyed by
    /// metric id; ``as_of`` a ``datetime.date``, ``pandas.Timestamp`` or ISO
    /// string. Returns a dict keyed by covenant label in spec order.
    ///
    /// Raises ``KeyError`` when a required metric is missing and
    /// ``ValueError`` when the engine is invalid, two specs share a label, or
    /// a metric value is not a number.
    #[pyo3(text_signature = "(metrics, as_of)")]
    fn evaluate<'py>(
        &self,
        py: Python<'py>,
        metrics: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let as_of = py_to_date(as_of)?;
        let source = extract_metrics(metrics)?;
        let reports = py.detach(|| self.inner.evaluate(&source, as_of).map_err(core_to_py))?;
        reports_to_pydict(py, reports)
    }

    /// Evaluate like ``evaluate`` and update ``breach_history``: a failing
    /// covenant without an active breach gains a breach record (with its cure
    /// deadline), and a later pass inside the cure period marks it cured.
    ///
    /// ``scope`` must be ``maintenance`` for scheduled testing or ``incurrence``
    /// for a completed action assessed using pro forma metrics. Inactive and
    /// waived reports do not cure historical breaches.
    /// Raises the same exceptions as ``evaluate``; on error the history is
    /// left untouched. Invalid scope raises ``ValueError``.
    #[pyo3(text_signature = "(metrics, as_of, scope)")]
    fn evaluate_and_track<'py>(
        &mut self,
        py: Python<'py>,
        metrics: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        scope: &str,
    ) -> PyResult<Bound<'py, PyDict>> {
        let scope = crate::bindings::extract::parse_label::<finstack_quant_covenants::CovenantScope>(
            scope,
        )?;
        let as_of = py_to_date(as_of)?;
        let source = extract_metrics(metrics)?;
        let reports = py.detach(|| {
            self.inner
                .evaluate_and_track(&source, as_of, scope)
                .map_err(core_to_py)
        })?;
        reports_to_pydict(py, reports)
    }

    /// Evaluate the engine on every row of a date-indexed metrics frame.
    ///
    /// ``metrics`` is a ``pandas.DataFrame`` whose index holds the test dates
    /// and whose columns are unique metric ids; required cells must be finite.
    /// Returns a long frame with one row per (date, covenant) and columns
    /// ``as_of`` (ISO string), ``covenant`` (label), ``covenant_type``,
    /// ``passed``, ``actual_value``, ``threshold``, ``headroom``, ``details``.
    ///
    /// Raises ``KeyError`` when a required metric is missing on any date and
    /// ``ValueError`` for an invalid engine or non-numeric frame.
    #[pyo3(text_signature = "(metrics)")]
    fn evaluate_series<'py>(
        &self,
        py: Python<'py>,
        metrics: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let rows = extract_metric_frame(metrics)?;
        let evaluated = py.detach(|| self.inner.evaluate_series(&rows).map_err(core_to_py))?;
        dated_reports_frame(py, evaluated)
    }

    /// Deserialize from JSON; only ``specs`` is required.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON (the ``evaluate_engine`` engine document).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Top-level covenant specifications in insertion order.
    #[getter]
    fn specs(&self) -> Vec<PyCovenantSpec> {
        self.inner
            .specs
            .iter()
            .cloned()
            .map(PyCovenantSpec::from_inner)
            .collect()
    }

    /// Recorded waivers and amendments.
    #[getter]
    fn waivers(&self) -> Vec<PyCovenantWaiver> {
        self.inner
            .waivers
            .iter()
            .cloned()
            .map(PyCovenantWaiver::from_inner)
            .collect()
    }

    /// Breaches recorded by ``evaluate_and_track`` (or loaded from JSON).
    #[getter]
    fn breach_history(&self) -> Vec<PyCovenantBreach> {
        self.inner
            .breach_history
            .iter()
            .cloned()
            .map(PyCovenantBreach::from_inner)
            .collect()
    }

    fn __len__(&self) -> usize {
        self.inner.specs.len()
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CovenantEngine", &self.inner)
    }
}

/// A recorded covenant breach with its cure deadline and applied consequences.
#[pyclass(
    name = "CovenantBreach",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantBreach {
    pub(crate) inner: CovenantBreach,
}

impl PyCovenantBreach {
    pub(crate) fn from_inner(inner: CovenantBreach) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenantBreach {
    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Instance label of the breached covenant.
    #[getter]
    fn covenant_id(&self) -> &str {
        &self.inner.covenant_id
    }

    /// Human-readable covenant description.
    #[getter]
    fn covenant_type(&self) -> &str {
        &self.inner.covenant_type
    }

    /// Test date on which the breach was recorded.
    #[getter]
    fn breach_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.breach_date)
    }

    /// Metric value that caused the breach, if numeric.
    #[getter]
    fn actual_value(&self) -> Option<f64> {
        self.inner.actual_value
    }

    /// Threshold in force at the breach.
    #[getter]
    fn threshold(&self) -> Option<f64> {
        self.inner.threshold
    }

    /// End of the cure period, or ``None`` when the covenant has none.
    #[getter]
    fn cure_deadline<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .cure_deadline
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Whether a later pass inside the cure period cured the breach.
    #[getter]
    fn is_cured(&self) -> bool {
        self.inner.is_cured
    }

    /// Consequences captured from the effective covenant when this breach began.
    #[getter]
    fn consequences(&self) -> Vec<super::spec::PyCovenantConsequence> {
        self.inner
            .consequences
            .iter()
            .cloned()
            .map(super::spec::PyCovenantConsequence::from_inner)
            .collect()
    }

    /// Consequences already applied for this breach.
    #[getter]
    fn applied_consequences(&self) -> Vec<super::spec::PyCovenantConsequence> {
        self.inner
            .applied_consequences
            .iter()
            .cloned()
            .map(super::spec::PyCovenantConsequence::from_inner)
            .collect()
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CovenantBreach", &self.inner)
    }
}

/// One standard covenant package and its thresholds.
///
/// Build it with the variant constructors (``CovenantTemplate.lbo_standard``,
/// ``cov_lite``, ``real_estate``, ``project_finance``) and expand it with
/// ``covenant_template``. Ratios are in turns, debt yield and LTV are decimal
/// fractions, and capex and liquidity are reporting-currency amounts.
#[pyclass(
    name = "CovenantTemplate",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantTemplate {
    pub(crate) inner: CovenantTemplate,
}

#[pymethods]
impl PyCovenantTemplate {
    /// Standard leveraged-buyout package: quarterly maximum Debt/EBITDA
    /// (``initial_leverage``, turns), minimum interest and fixed-charge
    /// coverage (turns), and an annual maximum capex (reporting-currency
    /// amount).
    #[staticmethod]
    #[pyo3(
        text_signature = "(initial_leverage, interest_coverage, fixed_charge_coverage, max_capex)"
    )]
    fn lbo_standard(
        initial_leverage: f64,
        interest_coverage: f64,
        fixed_charge_coverage: f64,
        max_capex: f64,
    ) -> Self {
        Self {
            inner: CovenantTemplate::LboStandard {
                initial_leverage,
                interest_coverage,
                fixed_charge_coverage,
                max_capex,
            },
        }
    }

    /// Covenant-lite package: incurrence-only maximum total and senior
    /// leverage (turns) and a negative covenant on additional secured debt.
    #[staticmethod]
    #[pyo3(text_signature = "(max_leverage, max_senior_leverage)")]
    fn cov_lite(max_leverage: f64, max_senior_leverage: f64) -> Self {
        Self {
            inner: CovenantTemplate::CovLite {
                max_leverage,
                max_senior_leverage,
            },
        }
    }

    /// Commercial real-estate package: minimum DSCR (turns), minimum debt
    /// yield and maximum LTV (decimal fractions).
    #[staticmethod]
    #[pyo3(text_signature = "(min_dscr, min_debt_yield, max_ltv)")]
    fn real_estate(min_dscr: f64, min_debt_yield: f64, max_ltv: f64) -> Self {
        Self {
            inner: CovenantTemplate::RealEstate {
                min_dscr,
                min_debt_yield,
                max_ltv,
            },
        }
    }

    /// Project-finance package: default and distribution lock-up DSCR
    /// (turns), minimum debt-service reserve (reporting-currency amount) and
    /// maximum net Debt/EBITDA (turns).
    #[staticmethod]
    #[pyo3(
        text_signature = "(min_dscr, distribution_lockup_dscr, min_liquidity, max_net_leverage)"
    )]
    fn project_finance(
        min_dscr: f64,
        distribution_lockup_dscr: f64,
        min_liquidity: f64,
        max_net_leverage: f64,
    ) -> Self {
        Self {
            inner: CovenantTemplate::ProjectFinance {
                min_dscr,
                distribution_lockup_dscr,
                min_liquidity,
                max_net_leverage,
            },
        }
    }

    /// Package wire tag: ``lbo_standard``, ``cov_lite``, ``real_estate`` or
    /// ``project_finance``.
    #[getter]
    fn kind(&self) -> PyResult<String> {
        let wire = serde_json::to_value(self.inner).map_err(display_to_py)?;
        wire["kind"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| value_error("CovenantTemplate serialized without a kind tag"))
    }

    /// Deserialize from canonical JSON (strict: unknown fields are rejected).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CovenantTemplate", &self.inner)
    }
}

/// Expand a ``CovenantTemplate`` into its typed covenant specs.
///
/// Raises ``ValueError`` when any threshold is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(template)")]
pub(crate) fn covenant_template(template: PyCovenantTemplate) -> PyResult<Vec<PyCovenantSpec>> {
    covenant_template_rs(&template.inner)
        .map_err(core_to_py)
        .map(|specs| specs.into_iter().map(PyCovenantSpec::from_inner).collect())
}

/// Flatten ``evaluate`` output into one frame row per covenant.
///
/// ``reports`` is the ``dict[str, CovenantReport]`` returned by
/// ``CovenantEngine.evaluate`` or ``evaluate_engine``. Columns:
/// ``covenant`` (dict key), ``covenant_type``, ``passed``, ``actual_value``,
/// ``threshold``, ``headroom``, ``details``.
///
/// Raises ``TypeError`` when a value is not a ``CovenantReport``.
#[pyfunction]
#[pyo3(text_signature = "(reports)")]
pub(crate) fn reports_to_dataframe<'py>(
    py: Python<'py>,
    reports: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut rows = Vec::with_capacity(reports.len());
    for (key, value) in reports.iter() {
        let key: String = key.extract()?;
        let report = value.extract::<PyRef<'_, PyCovenantReport>>()?;
        rows.push(serde_json::json!({
            "covenant": key,
            "covenant_type": report.inner.covenant_type,
            "passed": report.inner.passed,
            "actual_value": report.inner.actual_value,
            "threshold": report.inner.threshold,
            "headroom": report.inner.headroom,
            "details": report.inner.details,
        }));
    }
    serde_rows_to_dataframe_with_schema(
        py,
        &rows,
        &[
            ("covenant", "str"),
            ("covenant_type", "str"),
            ("passed", "bool"),
            ("actual_value", "float64"),
            ("threshold", "float64"),
            ("headroom", "float64"),
            ("details", "str"),
        ],
    )
}
