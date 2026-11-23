//! Sensitivity, variance, scenario, and bridge result Python bindings.

use super::config::PySensitivityConfig;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements::parse_period_id;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::analysis::{
    BridgeChart as RustBridgeChart, BridgeStep as RustBridgeStep, ScenarioDiff as RustScenarioDiff,
    ScenarioResults, SensitivityResult as RustSensitivityResult, TornadoEntry as RustTornadoEntry,
    VarianceReport as RustVarianceReport, VarianceRow as RustVarianceRow,
};
use pyo3::prelude::*;

/// Column schema for [`PyVarianceReport`] tabular exports.
const VARIANCE_ROW_COLUMNS: [(&str, &str); 6] = [
    ("period", "str"),
    ("metric", "str"),
    ("baseline", "float64"),
    ("comparison", "float64"),
    ("abs_var", "float64"),
    ("pct_var", "float64"),
];

/// Column schema for the fixed part of [`PySensitivityResult`] tabular
/// exports; one column per perturbed parameter is inserted after ``scenario``.
const SENSITIVITY_TAIL_COLUMNS: [(&str, &str); 3] =
    [("node_id", "str"), ("period", "str"), ("value", "float64")];

/// Column schema for [`PyBridgeChart`] tabular exports.
const BRIDGE_STEP_COLUMNS: [(&str, &str); 2] = [("driver", "str"), ("contribution", "float64")];

/// One parameter's downside and upside impact in a tornado chart.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import TornadoEntry
/// >>> entry = TornadoEntry.from_json('{"parameter_id":"revenue","downside":-5.0,"upside":7.0}')
/// >>> entry.swing
/// 12.0
#[pyclass(
    name = "TornadoEntry",
    module = "finstack_quant.statements_analytics",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTornadoEntry {
    pub(crate) inner: RustTornadoEntry,
}

impl PyTornadoEntry {
    pub(crate) fn from_inner(inner: RustTornadoEntry) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTornadoEntry {
    /// Deserialize one tornado entry from canonical JSON.
    /// Statement sensitivity parameter key in ``node@period`` form; DCF entries
    /// use assumption identifiers such as ``wacc`` or ``g``.
    #[getter]
    fn parameter_id(&self) -> &str {
        &self.inner.parameter_id
    }

    /// Metric change at the parameter's minimum perturbation.
    #[getter]
    fn downside(&self) -> f64 {
        self.inner.downside
    }

    /// Metric change at the parameter's maximum perturbation.
    #[getter]
    fn upside(&self) -> f64 {
        self.inner.upside
    }

    /// Total swing magnitude, calculated as `upside - downside`.
    #[getter]
    fn swing(&self) -> f64 {
        self.inner.swing()
    }

    fn __repr__(&self) -> String {
        format!(
            "TornadoEntry(parameter_id='{}', downside={}, upside={})",
            self.inner.parameter_id, self.inner.downside, self.inner.upside
        )
    }
}

wire_methods!(PyTornadoEntry, RustTornadoEntry, "TornadoEntry");

/// Typed root result for statement sensitivity analysis.
#[pyclass(
    name = "SensitivityResult",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PySensitivityResult {
    pub(crate) inner: RustSensitivityResult,
}

#[pymethods]
impl PySensitivityResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Node identifiers of the metrics tracked by the originating config.
    #[getter]
    fn target_metrics(&self) -> Vec<String> {
        self.inner.config.target_metrics.clone()
    }

    /// Configuration the run was generated from.
    #[getter]
    fn config(&self) -> PySensitivityConfig {
        PySensitivityConfig {
            inner: self.inner.config.clone(),
        }
    }

    /// Unperturbed baseline evaluation retained by every sensitivity run;
    /// ``None`` only when absent from an imported result document.
    #[getter]
    fn baseline(&self) -> Option<PyStatementResult> {
        self.inner
            .baseline
            .clone()
            .map(|inner| PyStatementResult { inner })
    }

    /// Per-scenario ``(parameter_values, results)`` pairs in generation order,
    /// where ``parameter_values`` is a ``{"node_id@period": value}`` dict.
    #[getter]
    fn scenarios(&self) -> Vec<(std::collections::BTreeMap<String, f64>, PyStatementResult)> {
        self.inner
            .scenarios
            .iter()
            .map(|scenario| {
                (
                    scenario
                        .parameter_values
                        .iter()
                        .map(|(k, v)| (k.clone(), *v))
                        .collect(),
                    PyStatementResult {
                        inner: scenario.results.clone(),
                    },
                )
            })
            .collect()
    }

    /// Export the run as a long ``ArrowTable``.
    ///
    /// Columns: ``scenario`` (0-based index), one column per perturbed
    /// parameter named ``node_id@period`` (null where a scenario does not
    /// perturb it), then ``node_id``, ``period`` and ``value`` holding each
    /// tracked metric's value in that scenario. One row per (scenario, metric,
    /// period); when no metric is tracked, one row per scenario with the
    /// metric columns null so the parameter grid stays visible.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str] | None
    ///     Node identifiers to emit; ``None`` uses the config's
    ///     ``target_metrics``.
    #[pyo3(signature = (metrics=None))]
    fn to_arrow(&self, metrics: Option<Vec<String>>) -> PyResult<PyArrowTable> {
        self.to_arrow_table(metrics)
    }

    /// Export the run as a long pandas ``DataFrame``.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str] | None
    ///     Node identifiers to emit; ``None`` uses the config's
    ///     ``target_metrics``.
    #[pyo3(signature = (metrics=None))]
    fn to_pandas<'py>(
        &self,
        py: Python<'py>,
        metrics: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table(metrics)?)?;
        PyArrowTable::to_pandas(&table, py)
    }

    /// Export the run as a long Polars ``DataFrame``.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str] | None
    ///     Node identifiers to emit; ``None`` uses the config's
    ///     ``target_metrics``.
    #[pyo3(signature = (metrics=None))]
    fn to_polars<'py>(
        &self,
        py: Python<'py>,
        metrics: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table(metrics)?)?;
        PyArrowTable::to_polars(&table, py)
    }

    /// Export the run as a long ``pyarrow.Table``.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str] | None
    ///     Node identifiers to emit; ``None`` uses the config's
    ///     ``target_metrics``.
    #[pyo3(signature = (metrics=None))]
    fn to_pyarrow<'py>(
        &self,
        py: Python<'py>,
        metrics: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table(metrics)?)?;
        PyArrowTable::to_pyarrow(&table, py)
    }

    /// Export via the Arrow PyCapsule C-stream protocol (`arrow_array_stream`).
    #[pyo3(signature = (requested_schema = None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, pyo3::types::PyCapsule>> {
        let _ = requested_schema;
        self.to_arrow_table(None)?.arrow_c_stream(py)
    }

    /// Export the Arrow schema via the Arrow PyCapsule interface (`arrow_schema`).
    fn __arrow_c_schema__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, pyo3::types::PyCapsule>> {
        self.to_arrow_table(None)?.arrow_c_schema(py)
    }

    fn get_parameter_value(&self, scenario_index: usize, parameter: &str) -> PyResult<Option<f64>> {
        self.inner
            .get_parameter_value(scenario_index, parameter)
            .map_err(statements_to_py)
    }

    fn get_value(
        &self,
        scenario_index: usize,
        node_id: &str,
        period: &str,
    ) -> PyResult<Option<f64>> {
        self.inner
            .get_value(scenario_index, node_id, &parse_period_id(period)?)
            .map_err(statements_to_py)
    }

    /// Render as an HTML table preview in Jupyter notebooks.
    fn _repr_html_(&self) -> Option<String> {
        self.to_arrow_table(None).ok().map(|t| t.render_html())
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a table exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("SensitivityResult", &self.inner)
    }
}

impl PySensitivityResult {
    fn to_arrow_table(&self, metrics: Option<Vec<String>>) -> PyResult<PyArrowTable> {
        let metrics = metrics.unwrap_or_else(|| self.inner.config.target_metrics.clone());
        let mut parameter_names: Vec<String> = Vec::new();
        for scenario in &self.inner.scenarios {
            for name in scenario.parameter_values.keys() {
                if !parameter_names.contains(name) {
                    parameter_names.push(name.clone());
                }
            }
        }
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for (index, scenario) in self.inner.scenarios.iter().enumerate() {
            let mut base = serde_json::Map::new();
            base.insert("scenario".to_string(), serde_json::json!(index));
            for (parameter, value) in &scenario.parameter_values {
                base.insert(parameter.clone(), serde_json::json!(value));
            }
            let mut emitted = false;
            for metric in &metrics {
                if let Some(series) = scenario.results.get_node(metric) {
                    for (period, cell) in series.iter() {
                        let mut row = base.clone();
                        row.insert("node_id".to_string(), serde_json::json!(metric));
                        row.insert("period".to_string(), serde_json::json!(period.to_string()));
                        row.insert("value".to_string(), serde_json::json!(cell.numeric()));
                        rows.push(serde_json::Value::Object(row));
                        emitted = true;
                    }
                }
            }
            if !emitted {
                let mut row = base;
                row.insert("node_id".to_string(), serde_json::Value::Null);
                row.insert("period".to_string(), serde_json::Value::Null);
                row.insert("value".to_string(), serde_json::Value::Null);
                rows.push(serde_json::Value::Object(row));
            }
        }
        let mut columns: Vec<(&str, &str)> = vec![("scenario", "int64")];
        columns.extend(
            parameter_names
                .iter()
                .map(|name| (name.as_str(), "float64")),
        );
        columns.extend(SENSITIVITY_TAIL_COLUMNS);
        serde_rows_to_arrow_table(&rows, &columns)
    }
}

wire_methods!(
    PySensitivityResult,
    RustSensitivityResult,
    "SensitivityResult"
);

/// One typed variance-report row.
#[pyclass(
    name = "VarianceRow",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyVarianceRow {
    inner: RustVarianceRow,
}

#[pymethods]
impl PyVarianceRow {
    /// Period this row covers, as a period-id string (e.g. ``"2025Q1"``).
    #[getter]
    fn period(&self) -> String {
        self.inner.period.to_string()
    }

    /// Node identifier of the compared metric.
    #[getter]
    fn metric(&self) -> &str {
        &self.inner.metric
    }

    /// Metric value in the baseline scenario, in the metric's own units.
    #[getter]
    fn baseline(&self) -> f64 {
        self.inner.baseline
    }

    /// Metric value in the comparison scenario, in the metric's own units.
    #[getter]
    fn comparison(&self) -> f64 {
        self.inner.comparison
    }

    /// Absolute variance ``comparison - baseline``, in the metric's units.
    #[getter]
    fn abs_var(&self) -> f64 {
        self.inner.abs_var
    }

    /// Percentage variance ``abs_var / baseline`` as a decimal fraction
    /// (``0.1`` = +10%).
    ///
    /// ``None`` when the baseline is effectively zero, where a ratio would be
    /// undefined rather than zero; fall back to ``abs_var`` in that case.
    #[getter]
    fn pct_var(&self) -> Option<f64> {
        self.inner.pct_var
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("VarianceRow", &self.inner)
    }
}

/// Typed root variance report.
#[pyclass(
    name = "VarianceReport",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyVarianceReport {
    pub(crate) inner: RustVarianceReport,
}

#[pymethods]
impl PyVarianceReport {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    /// Label for the baseline scenario (e.g. ``"management_case"``).
    #[getter]
    fn baseline_label(&self) -> &str {
        &self.inner.baseline_label
    }

    /// Label for the comparison scenario (e.g. ``"bank_case"``).
    #[getter]
    fn comparison_label(&self) -> &str {
        &self.inner.comparison_label
    }

    /// Per-metric, per-period variance rows, in report order.
    #[getter]
    fn rows(&self) -> Vec<PyVarianceRow> {
        self.inner
            .rows
            .iter()
            .cloned()
            .map(|inner| PyVarianceRow { inner })
            .collect()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a table exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("VarianceReport", &self.inner)
    }
}

wire_methods!(PyVarianceReport, RustVarianceReport, "VarianceReport");

impl PyVarianceReport {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "period": row.period.to_string(),
                    "metric": row.metric,
                    "baseline": row.baseline,
                    "comparison": row.comparison,
                    "abs_var": row.abs_var,
                    // Emitted explicitly (not via `VarianceRow`'s serde, which
                    // skips a `None`) so the column exists even when every
                    // baseline is zero.
                    "pct_var": row.pct_var,
                })
            })
            .collect();
        serde_rows_to_arrow_table(&rows, &VARIANCE_ROW_COLUMNS)
    }
}

tabular_methods!(PyVarianceReport);

/// Typed evaluated results for a set of named scenarios.
///
/// Named after the canonical Rust type
/// (`finstack_quant_statements_analytics::analysis::ScenarioResults`).
#[pyclass(
    name = "ScenarioResults",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioResults {
    pub(crate) inner: ScenarioResults,
}

#[pymethods]
impl PyScenarioResults {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    /// Evaluated scenario names, in the order the scenario set defined them.
    #[getter]
    fn names(&self) -> Vec<String> {
        self.inner.scenarios.keys().cloned().collect()
    }

    /// Identify this result set in notebooks and logs.
    ///
    /// Rendered as the scenario count; use :meth:`names` for the full list.
    fn __repr__(&self) -> String {
        format!("ScenarioResults(scenarios={})", self.inner.scenarios.len())
    }

    fn get(&self, name: &str) -> Option<PyStatementResult> {
        self.inner
            .scenarios
            .get(name)
            .cloned()
            .map(|inner| PyStatementResult { inner })
    }

    /// Build a side-by-side comparison table across every evaluated scenario.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str]
    ///     Node identifiers to include as rows.
    ///
    /// Returns
    /// -------
    /// ArrowTable
    ///     One column per scenario, one row per (metric, period).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result set or `metrics` is empty.
    /// KeyError
    ///     If a metric in ``metrics`` is not a node of the baseline scenario.
    #[pyo3(text_signature = "(metrics)")]
    fn to_comparison_table(&self, metrics: Vec<String>) -> PyResult<PyArrowTable> {
        let table = self.comparison_table(&metrics)?;
        PyArrowTable::from_envelope(&table)
    }

    /// Export the scenario comparison as an ``ArrowTable``.
    ///
    /// Columns: ``period``, ``metric``, one column per scenario name holding
    /// that scenario's metric value, and one ``{scenario}_vs_{baseline}_frac``
    /// column per non-baseline scenario holding the relative change as a
    /// decimal fraction (``0.1`` = +10%, ``NaN`` on a near-zero baseline). The
    /// ``_frac`` suffix states the unit: multiply by 100 for percent.
    /// One row per (metric, period) pair.
    ///
    /// This is the same table as ``to_comparison_table`` — both call one Rust
    /// implementation, so the two cannot drift apart.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str]
    ///     Node identifiers to include as rows.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result set or ``metrics`` is empty.
    /// KeyError
    ///     If a metric in ``metrics`` is not a node of the baseline scenario.
    #[pyo3(text_signature = "(metrics)")]
    fn to_arrow(&self, metrics: Vec<String>) -> PyResult<PyArrowTable> {
        let table = self.comparison_table(&metrics)?;
        PyArrowTable::from_envelope(&table)
    }

    /// Export the scenario comparison as a pandas ``DataFrame``.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str]
    ///     Node identifiers to include as rows.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result set or ``metrics`` is empty.
    /// KeyError
    ///     If a metric in ``metrics`` is not a node of the baseline scenario.
    #[pyo3(text_signature = "(metrics)")]
    fn to_pandas<'py>(&self, py: Python<'py>, metrics: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
        let table = self.comparison_table(&metrics)?;
        let arrow = Bound::new(py, PyArrowTable::from_envelope(&table)?)?;
        PyArrowTable::to_pandas(&arrow, py)
    }

    /// Export the scenario comparison as a Polars ``DataFrame``.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str]
    ///     Node identifiers to include as rows.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result set or ``metrics`` is empty.
    /// KeyError
    ///     If a metric in ``metrics`` is not a node of the baseline scenario.
    #[pyo3(text_signature = "(metrics)")]
    fn to_polars<'py>(&self, py: Python<'py>, metrics: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
        let table = self.comparison_table(&metrics)?;
        let arrow = Bound::new(py, PyArrowTable::from_envelope(&table)?)?;
        PyArrowTable::to_polars(&arrow, py)
    }

    /// Export the scenario comparison as a ``pyarrow.Table``.
    ///
    /// Parameters
    /// ----------
    /// metrics : list[str]
    ///     Node identifiers to include as rows.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result set or ``metrics`` is empty.
    /// KeyError
    ///     If a metric in ``metrics`` is not a node of the baseline scenario.
    #[pyo3(text_signature = "(metrics)")]
    fn to_pyarrow<'py>(
        &self,
        py: Python<'py>,
        metrics: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = self.comparison_table(&metrics)?;
        let arrow = Bound::new(py, PyArrowTable::from_envelope(&table)?)?;
        PyArrowTable::to_pyarrow(&arrow, py)
    }
}

wire_methods!(PyScenarioResults, ScenarioResults, "ScenarioResults");

impl PyScenarioResults {
    /// Build the canonical comparison table shared by `to_comparison_table`
    /// and the tabular export methods.
    fn comparison_table(
        &self,
        metrics: &[String],
    ) -> PyResult<finstack_quant_core::table::TableEnvelope> {
        let refs: Vec<&str> = metrics.iter().map(String::as_str).collect();
        self.inner
            .to_comparison_table(&refs)
            .map_err(crate::errors::statements_to_py)
    }
}

/// Variance between two named scenarios in an evaluated scenario set.
#[pyclass(
    name = "ScenarioDiff",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioDiff {
    pub(crate) inner: RustScenarioDiff,
}

#[pymethods]
impl PyScenarioDiff {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    /// Name of the scenario used as the baseline of the diff.
    #[getter]
    fn baseline(&self) -> &str {
        &self.inner.baseline
    }

    /// Identify this diff in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScenarioDiff", &self.inner)
    }

    /// Name of the scenario compared against the baseline.
    #[getter]
    fn comparison(&self) -> &str {
        &self.inner.comparison
    }

    /// Underlying variance report between the two named scenarios.
    #[getter]
    fn variance(&self) -> PyVarianceReport {
        PyVarianceReport {
            inner: self.inner.variance.clone(),
        }
    }
}

wire_methods!(PyScenarioDiff, RustScenarioDiff, "ScenarioDiff");

impl PyScenarioDiff {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        self.variance().to_arrow_table()
    }
}

tabular_methods!(PyScenarioDiff);

/// One driver step in a bridge decomposition.
#[pyclass(
    name = "BridgeStep",
    module = "finstack_quant.statements_analytics",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBridgeStep {
    pub(crate) inner: RustBridgeStep,
}

#[pymethods]
impl PyBridgeStep {
    /// Driver node identifier (e.g. ``"revenue"``).
    #[getter]
    fn driver(&self) -> &str {
        &self.inner.driver
    }

    /// This driver's raw delta between the two scenarios, in the *driver's*
    /// own units.
    ///
    /// Contributions are not sensitivities of the target metric, so they
    /// generally do not sum to the target variance — see
    /// ``BridgeChart.unexplained``.
    #[getter]
    fn contribution(&self) -> f64 {
        self.inner.contribution
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("BridgeStep", &self.inner)
    }
}

/// Bridge decomposition of a metric's variance across named drivers.
#[pyclass(
    name = "BridgeChart",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyBridgeChart {
    pub(crate) inner: RustBridgeChart,
}

#[pymethods]
impl PyBridgeChart {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    /// Node identifier of the metric this bridge decomposes (e.g.
    /// ``"ebitda"``).
    #[getter]
    fn target_metric(&self) -> &str {
        &self.inner.target_metric
    }

    /// Period the bridge covers, as a period-id string (e.g. ``"2025Q1"``).
    #[getter]
    fn period(&self) -> String {
        self.inner.period.to_string()
    }

    /// Label for the baseline scenario (e.g. ``"management_case"``).
    #[getter]
    fn baseline_label(&self) -> &str {
        &self.inner.baseline_label
    }

    /// Label for the comparison scenario (e.g. ``"bank_case"``).
    #[getter]
    fn comparison_label(&self) -> &str {
        &self.inner.comparison_label
    }

    /// Target-metric value in the baseline scenario, in the metric's units.
    #[getter]
    fn baseline_value(&self) -> f64 {
        self.inner.baseline_value
    }

    /// Target-metric value in the comparison scenario, in the metric's units.
    #[getter]
    fn comparison_value(&self) -> f64 {
        self.inner.comparison_value
    }

    /// Ordered driver contributions making up the bridge.
    #[getter]
    fn steps(&self) -> Vec<PyBridgeStep> {
        self.inner
            .steps
            .iter()
            .map(|step| PyBridgeStep {
                inner: step.clone(),
            })
            .collect()
    }

    /// Residual variance not explained by the driver deltas.
    ///
    /// Driver contributions are raw deltas in driver units rather than
    /// sensitivities of the target metric, so they generally do not sum to
    /// the target variance. This term makes that gap explicit.
    #[getter]
    fn unexplained(&self) -> f64 {
        self.inner.unexplained
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a table exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("BridgeChart", &self.inner)
    }
}

wire_methods!(PyBridgeChart, RustBridgeChart, "BridgeChart");

impl PyBridgeChart {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .steps
            .iter()
            .map(|step| {
                serde_json::json!({
                    "driver": step.driver,
                    "contribution": step.contribution,
                })
            })
            .collect();
        serde_rows_to_arrow_table(&rows, &BRIDGE_STEP_COLUMNS)
    }
}

tabular_methods!(PyBridgeChart);
