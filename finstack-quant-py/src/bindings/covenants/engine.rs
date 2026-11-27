//! `CovenantEngine` and `CovenantBreach` wrappers plus `CovenantTemplate` and
//! its expansion into typed specs.

use super::report::PyCovenantReport;
use super::spec::{PyCovenantSpec, PyCovenantWaiver};
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::repr_support::repr_from_serde;
use crate::bindings::table_builders::TableBuilder;
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

/// Extract a ``dict[str, float]`` or ``pandas.Series`` (mapping metric ids)
/// of metric values.
pub(crate) fn extract_metrics(obj: &Bound<'_, PyAny>) -> PyResult<HashMapMetricSource> {
    if crate::bindings::dataframe_utils::is_pandas_series(obj)? {
        let dict = obj.call_method0("to_dict")?;
        return extract_metric_dict(&dict).map(HashMapMetricSource::from_pairs);
    }
    extract_metric_dict(obj).map(HashMapMetricSource::from_pairs)
}

/// Convert a ``dict[str, float]`` of metric values into pairs.
pub(crate) fn extract_metric_dict(obj: &Bound<'_, PyAny>) -> PyResult<Vec<(String, f64)>> {
    let dict = obj.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "metrics must be a dict[str, float], got {}",
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

/// Flatten dated covenant reports into the long Arrow table: one row per
/// (test date, covenant), dates in series order and covenants in engine order.
/// Shared by `CovenantEngine.evaluate_series` and
/// `statements_analytics.evaluate_covenants`.
pub(crate) fn dated_reports_table(series: Vec<DatedCovenantReports>) -> PyResult<PyArrowTable> {
    let cap: usize = series.iter().map(|d| d.reports.len()).sum();
    let mut as_ofs = Vec::with_capacity(cap);
    let mut covenants = Vec::with_capacity(cap);
    let mut covenant_types = Vec::with_capacity(cap);
    let mut passed = Vec::with_capacity(cap);
    let mut actual_values = Vec::with_capacity(cap);
    let mut thresholds = Vec::with_capacity(cap);
    let mut headrooms = Vec::with_capacity(cap);
    let mut headrooms_raw = Vec::with_capacity(cap);
    let mut details = Vec::with_capacity(cap);

    for dated in series {
        let as_of = dated.as_of.to_string();
        for (key, report) in dated.reports {
            as_ofs.push(as_of.clone());
            covenants.push(key);
            covenant_types.push(report.covenant_type);
            passed.push(report.passed);
            actual_values.push(report.actual_value);
            thresholds.push(report.threshold);
            headrooms.push(report.headroom);
            headrooms_raw.push(report.headroom_raw);
            details.push(report.details);
        }
    }

    TableBuilder::new()
        .utf8("as_of", as_ofs)?
        .utf8("covenant", covenants)?
        .utf8("covenant_type", covenant_types)?
        .bool("passed", passed)?
        .nullable_f64("actual_value", actual_values)?
        .nullable_f64("threshold", thresholds)?
        .nullable_f64("headroom", headrooms)?
        .nullable_f64("headroom_raw", headrooms_raw)?
        .nullable_utf8("details", details)?
        .build()
}

/// Read a dated metrics frame (pandas, Polars, or Arrow) into Rust `DatedMetrics` rows.
///
/// Cells are preserved for canonical Rust validation. Duplicate metric
/// columns are rejected rather than silently overwriting observations.
pub(crate) fn extract_metric_frame(
    frame: &Bound<'_, PyAny>,
    date_col: Option<&str>,
) -> PyResult<Vec<DatedMetrics>> {
    use crate::bindings::dataframe_utils::{
        arrow_to_pyarrow_table, extract_dataframe_panel, has_arrow_c_stream, is_dataframe_like,
        is_polars_dataframe, require_polars_module, resolve_arrow_date_col,
        resolve_pandas_date_col_idx, resolve_polars_date_col, table_column_names,
    };
    if !is_dataframe_like(frame)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "metrics must be a pandas DataFrame, Polars DataFrame, or Arrow table",
        ));
    }
    let cols = table_column_names(frame)?;
    let resolved_date_name: Option<String> = if is_polars_dataframe(frame)? {
        let pl = require_polars_module(frame.py())?;
        Some(resolve_polars_date_col(&pl, frame, &cols, date_col)?)
    } else if has_arrow_c_stream(frame)? {
        let tbl = arrow_to_pyarrow_table(frame.py(), frame)?;
        let pa_types = frame.py().import("pyarrow")?.getattr("types")?;
        Some(resolve_arrow_date_col(&pa_types, &tbl, &cols, date_col)?)
    } else {
        if let Some(name) = date_col {
            resolve_pandas_date_col_idx(&cols, name)?;
        }
        date_col.map(str::to_owned)
    };
    let metric_cols: Vec<&String> = cols
        .iter()
        .filter(|c| resolved_date_name.as_deref() != Some(c.as_str()))
        .collect();
    let unique: std::collections::HashSet<&String> = metric_cols.iter().copied().collect();
    if unique.len() != metric_cols.len() {
        return Err(value_error(
            "metrics frame contains duplicate metric columns",
        ));
    }
    let panel = extract_dataframe_panel(
        frame,
        date_col,
        "metrics must be a pandas DataFrame, Polars DataFrame, or Arrow table",
    )?;
    if panel.columns.len() != panel.ticker_names.len() {
        return Err(value_error(
            "metrics frame index and values disagree in length",
        ));
    }
    Ok(panel
        .dates
        .into_iter()
        .enumerate()
        .map(|(row_idx, date)| DatedMetrics {
            date,
            metrics: panel
                .ticker_names
                .iter()
                .enumerate()
                .map(|(col_idx, name)| (name.clone(), panel.columns[col_idx][row_idx]))
                .collect(),
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

    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.covenants._engine", "CovenantEngine.from_template")]
    fn from_template(template: &PyCovenantTemplate) -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_covenants::CovenantEngine::from_template(&template.inner)
                .map_err(crate::errors::core_to_py)?,
        })
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
    /// ``metrics`` is a ``dict[str, float]`` keyed by
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
    /// ``metrics`` is a ``pandas.DataFrame``, Polars ``DataFrame``, or Arrow
    /// table holding test dates and unique metric columns; required cells must
    /// be finite. Returns a long ``ArrowTable`` with one row per
    /// ``(date, covenant)`` and columns ``as_of`` (ISO string), ``covenant``
    /// (label), ``covenant_type``, ``passed``, ``actual_value``, ``threshold``,
    /// ``headroom``, ``headroom_raw``, ``details``.
    ///
    /// Raises ``KeyError`` when a required metric is missing on any date and
    /// ``ValueError`` for an invalid engine or non-numeric frame.
    #[pyo3(signature = (metrics, *, date_col=None), text_signature = "(metrics, *, date_col=None)")]
    fn evaluate_series<'py>(
        &self,
        py: Python<'py>,
        metrics: &Bound<'py, PyAny>,
        date_col: Option<&str>,
    ) -> PyResult<PyArrowTable> {
        let rows = extract_metric_frame(metrics, date_col)?;
        let evaluated = py.detach(|| self.inner.evaluate_series(&rows).map_err(core_to_py))?;
        dated_reports_table(evaluated)
    }

    fn __len__(&self) -> usize {
        self.inner.specs.len()
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CovenantEngine", &self.inner)
    }
}

crate::bindings::macros::wire_methods!(PyCovenantEngine, CovenantEngine, "CovenantEngine");
crate::bindings::macros::py_getters!(PyCovenantEngine |s| {
    /// Top-level covenant specifications in insertion order.
    specs -> Vec<PyCovenantSpec> = s
        .inner
        .specs
        .iter()
        .cloned()
        .map(PyCovenantSpec::from_inner)
        .collect();

    /// Recorded waivers and amendments.
    waivers -> Vec<PyCovenantWaiver> = s
        .inner
        .waivers
        .iter()
        .cloned()
        .map(PyCovenantWaiver::from_inner)
        .collect();

    /// Breaches recorded by ``evaluate_and_track`` (or loaded from JSON).
    breach_history -> Vec<PyCovenantBreach> = s
        .inner
        .breach_history
        .iter()
        .cloned()
        .map(PyCovenantBreach::from_inner)
        .collect();
});

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

crate::bindings::macros::wire_methods!(PyCovenantBreach, CovenantBreach, "CovenantBreach");
crate::bindings::macros::py_getters!(PyCovenantBreach |s| {
    /// Instance label of the breached covenant.
    covenant_id -> &str = &s.inner.covenant_id;

    /// Human-readable covenant description.
    covenant_type -> &str = &s.inner.covenant_type;

    /// Metric value that caused the breach, if numeric.
    actual_value -> Option<f64> = s.inner.actual_value;

    /// Threshold in force at the breach.
    threshold -> Option<f64> = s.inner.threshold;

    /// Whether a later pass inside the cure period cured the breach.
    is_cured -> bool = s.inner.is_cured;

    /// Consequences captured from the effective covenant when this breach began.
    consequences -> Vec<super::spec::PyCovenantConsequence> = s
        .inner
        .consequences
        .iter()
        .cloned()
        .map(super::spec::PyCovenantConsequence::from_inner)
        .collect();

    /// Consequences already applied for this breach.
    applied_consequences -> Vec<super::spec::PyCovenantConsequence> = s
        .inner
        .applied_consequences
        .iter()
        .cloned()
        .map(super::spec::PyCovenantConsequence::from_inner)
        .collect();
});

#[pymethods]
impl PyCovenantBreach {
    /// Test date on which the breach was recorded.
    #[getter]
    fn breach_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.breach_date)
    }

    /// End of the cure period, or ``None`` when the covenant has none.
    #[getter]
    fn cure_deadline<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .cure_deadline
            .map(|date| date_to_py(py, date))
            .transpose()
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

    fn __repr__(&self) -> String {
        repr_from_serde("CovenantTemplate", &self.inner)
    }
}

crate::bindings::macros::wire_methods!(PyCovenantTemplate, CovenantTemplate, "CovenantTemplate");

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

/// Flatten ``evaluate`` output into an ``ArrowTable`` with one row per covenant.
///
/// ``reports`` is the ``dict[str, CovenantReport]`` returned by
/// ``CovenantEngine.evaluate``. Columns:
/// ``covenant`` (dict key), ``covenant_type``, ``passed``, ``actual_value``,
/// ``threshold``, ``headroom``, ``headroom_raw``, ``details``.
///
/// Raises ``TypeError`` when a value is not a ``CovenantReport``.
#[pyfunction]
#[pyo3(text_signature = "(reports)")]
pub(crate) fn reports_table(reports: &Bound<'_, PyDict>) -> PyResult<PyArrowTable> {
    let n = reports.len();
    let mut covenants = Vec::with_capacity(n);
    let mut covenant_types = Vec::with_capacity(n);
    let mut passed = Vec::with_capacity(n);
    let mut actual_values = Vec::with_capacity(n);
    let mut thresholds = Vec::with_capacity(n);
    let mut headrooms = Vec::with_capacity(n);
    let mut headrooms_raw = Vec::with_capacity(n);
    let mut details = Vec::with_capacity(n);

    for (key, value) in reports.iter() {
        let key: String = key.extract()?;
        let report = value.extract::<PyRef<'_, PyCovenantReport>>()?;
        covenants.push(key);
        covenant_types.push(report.inner.covenant_type.clone());
        passed.push(report.inner.passed);
        actual_values.push(report.inner.actual_value);
        thresholds.push(report.inner.threshold);
        headrooms.push(report.inner.headroom);
        headrooms_raw.push(report.inner.headroom_raw);
        details.push(report.inner.details.clone());
    }

    TableBuilder::new()
        .utf8("covenant", covenants)?
        .utf8("covenant_type", covenant_types)?
        .bool("passed", passed)?
        .nullable_f64("actual_value", actual_values)?
        .nullable_f64("threshold", thresholds)?
        .nullable_f64("headroom", headrooms)?
        .nullable_f64("headroom_raw", headrooms_raw)?
        .nullable_utf8("details", details)?
        .build()
}
