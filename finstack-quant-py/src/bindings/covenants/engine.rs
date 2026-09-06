//! `CovenantEngine` and `CovenantBreach` wrappers plus the typed template
//! builders.

use super::report::PyCovenantReport;
use super::spec::{PyCovenantSpec, PyCovenantWaiver};
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, ColumnSchema};
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, display_to_py, value_error};
use finstack_quant_covenants::{CovenantBreach, CovenantEngine, HashMapMetricSource};
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Date-indexed metric rows: one `(date, [(metric_id, value)])` entry per row.
type MetricFrameRows = Vec<(time::Date, Vec<(String, f64)>)>;

/// Accept a ``dict[str, float]`` or a JSON-object string of metric values.
///
/// Every value must be a real number; ``bool`` is rejected so a stray
/// ``True`` cannot silently evaluate as ``1.0``.
pub(crate) fn extract_metrics(obj: &Bound<'_, PyAny>) -> PyResult<Vec<(String, f64)>> {
    if let Ok(text) = obj.extract::<std::borrow::Cow<'_, str>>() {
        let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text)
            .map_err(|e| value_error(format!("Invalid metric map JSON: {e}")))?;
        return map
            .into_iter()
            .map(|(key, value)| {
                value
                    .as_f64()
                    .map(|number| (key.clone(), number))
                    .ok_or_else(|| {
                        value_error(format!("Metric '{key}' must be a finite JSON number"))
                    })
            })
            .collect();
    }
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

/// Read a date-indexed metrics frame into ``(date, [(metric, value)])`` rows.
///
/// Cells are preserved for canonical Rust validation. Duplicate metric
/// columns are rejected rather than silently overwriting observations.
pub(crate) fn extract_metric_frame(frame: &Bound<'_, PyAny>) -> PyResult<MetricFrameRows> {
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
        .map(|item| extract_date(&item?))
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
        .map(|(date, row)| {
            let pairs = columns
                .iter()
                .zip(row)
                .map(|(name, value)| (name.clone(), value))
                .collect();
            (date, pairs)
        })
        .collect())
}

/// Covenant package: specifications, waivers, and the breach history
/// accumulated by ``evaluate_and_track``.
///
/// Build it empty and chain ``add_spec`` / ``add_waiver``, or use
/// ``CovenantEngine.from_specs(lbo_standard(...))``. Evaluate with a
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
        let as_of = extract_date(as_of)?;
        let source = HashMapMetricSource::from_pairs(extract_metrics(metrics)?);
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
        let scope = super::spec::parse_scope(scope)?;
        let as_of = extract_date(as_of)?;
        let source = HashMapMetricSource::from_pairs(extract_metrics(metrics)?);
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
        let records = py.detach(|| {
            let mut records = Vec::new();
            for (as_of, pairs) in rows {
                let source = HashMapMetricSource::from_pairs(pairs);
                let reports = self.inner.evaluate(&source, as_of).map_err(core_to_py)?;
                for (key, report) in reports {
                    records.push(serde_json::json!({
                        "as_of": as_of.to_string(),
                        "covenant": key,
                        "covenant_type": report.covenant_type,
                        "passed": report.passed,
                        "actual_value": report.actual_value,
                        "threshold": report.threshold,
                        "headroom": report.headroom,
                        "details": report.details,
                    }));
                }
            }
            Ok::<_, PyErr>(records)
        })?;
        serde_rows_to_dataframe_with_schema(py, &records, &SERIES_COLUMNS)
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

fn wrap_specs(
    specs: finstack_quant_core::Result<Vec<finstack_quant_covenants::CovenantSpec>>,
) -> PyResult<Vec<PyCovenantSpec>> {
    specs
        .map_err(core_to_py)
        .map(|specs| specs.into_iter().map(PyCovenantSpec::from_inner).collect())
}

/// Standard leveraged-buyout covenant package as typed specs.
///
/// Quarterly maintenance tests for maximum gross Debt/EBITDA
/// (``initial_leverage``, turns), minimum interest coverage
/// (``interest_coverage``, turns) and minimum fixed-charge coverage
/// (``fixed_charge_coverage``, turns), plus an annual maximum-capex test
/// (``max_capex``, reporting-currency amount). Leverage and interest
/// coverage carry 30-day cure periods; a leverage breach steps the rate up
/// 200bp and a coverage breach blocks distributions.
///
/// Raises ``ValueError`` when any input is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(initial_leverage, interest_coverage, fixed_charge_coverage, max_capex)")]
pub(crate) fn lbo_standard(
    initial_leverage: f64,
    interest_coverage: f64,
    fixed_charge_coverage: f64,
    max_capex: f64,
) -> PyResult<Vec<PyCovenantSpec>> {
    wrap_specs(finstack_quant_covenants::templates::lbo_standard(
        initial_leverage,
        interest_coverage,
        fixed_charge_coverage,
        max_capex,
    ))
}

/// Covenant-lite leveraged-loan package as typed specs: incurrence-only
/// maximum total leverage (``max_leverage``, turns), maximum senior leverage
/// (``max_senior_leverage``, turns) and a negative covenant on additional
/// secured debt.
///
/// Raises ``ValueError`` when any input is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(max_leverage, max_senior_leverage)")]
pub(crate) fn cov_lite(
    max_leverage: f64,
    max_senior_leverage: f64,
) -> PyResult<Vec<PyCovenantSpec>> {
    wrap_specs(finstack_quant_covenants::templates::cov_lite(
        max_leverage,
        max_senior_leverage,
    ))
}

/// Commercial real-estate package as typed specs: minimum DSCR
/// (``min_dscr``, turns; 30-day cure, 100% cash sweep), minimum debt yield
/// (``min_debt_yield``, decimal fraction) and maximum LTV (``max_ltv``,
/// decimal fraction; 50% cash sweep).
///
/// Raises ``ValueError`` when any input is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(min_dscr, min_debt_yield, max_ltv)")]
pub(crate) fn real_estate(
    min_dscr: f64,
    min_debt_yield: f64,
    max_ltv: f64,
) -> PyResult<Vec<PyCovenantSpec>> {
    wrap_specs(finstack_quant_covenants::templates::real_estate(
        min_dscr,
        min_debt_yield,
        max_ltv,
    ))
}

/// Project-finance package as typed specs: default DSCR (``min_dscr``,
/// turns; 60-day cure, event of default), distribution lock-up DSCR
/// (``distribution_lockup_dscr``, turns), minimum debt-service reserve
/// (``min_liquidity``, reporting-currency amount) and maximum net
/// Debt/EBITDA (``max_net_leverage``, turns).
///
/// Raises ``ValueError`` when any input is NaN, infinite or negative.
#[pyfunction]
#[pyo3(text_signature = "(min_dscr, distribution_lockup_dscr, min_liquidity, max_net_leverage)")]
pub(crate) fn project_finance(
    min_dscr: f64,
    distribution_lockup_dscr: f64,
    min_liquidity: f64,
    max_net_leverage: f64,
) -> PyResult<Vec<PyCovenantSpec>> {
    wrap_specs(finstack_quant_covenants::templates::project_finance(
        min_dscr,
        distribution_lockup_dscr,
        min_liquidity,
        max_net_leverage,
    ))
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
