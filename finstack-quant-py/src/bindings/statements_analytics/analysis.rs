//! Python wrappers for statement analysis functions.
//!
//! Covers: sensitivity, variance, scenario sets, backtesting, goal seek, and
//! introspection (dependency tracing, formula explanation). DCF/LBO live in
//! `valuation.rs`, check suites in `checks.rs`, reports in `reports.rs`.
//!
//! All functions that accept a financial model or statement result support
//! both JSON strings and typed Python objects (`FinancialModelSpec`,
//! `StatementResult`) for zero-overhead calls when the caller already has
//! a parsed object. Errors from the `finstack-quant-statements` crate map to
//! `KeyError` for missing nodes, `RuntimeError` for capital-structure
//! failures and `ValueError` otherwise.

use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::bindings::pandas_utils::{
    labeled_values_to_series, serde_rows_to_dataframe_with_schema, ColumnSchema,
};
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::bindings::statements_analytics::typed::{
    PyBridgeChart, PyScenarioDiff, PyScenarioResults, PyScenarioSet, PySensitivityConfig,
    PySensitivityResult, PyTornadoEntry, PyVarianceConfig, PyVarianceReport,
};
use crate::errors::{display_to_py, serde_json_to_py, statements_to_py};
use finstack_quant_statements_analytics::analysis::{
    Explanation, ExplanationStep, ForecastMetrics,
};
use pyo3::prelude::*;

/// Column schema for `Explanation.to_dataframe`.
const EXPLANATION_COLUMNS: [ColumnSchema<'static>; 3] = [
    ("component", "str"),
    ("value", "float64"),
    ("operation", "str"),
];

fn extract_sensitivity_config(
    value: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_statements_analytics::analysis::SensitivityConfig> {
    if let Ok(config) = value.extract::<PyRef<'_, PySensitivityConfig>>() {
        return Ok(config.inner.clone());
    }
    serde_json::from_str(value.extract::<&str>()?)
        .map_err(|e| serde_json_to_py(e, "invalid SensitivityConfig JSON"))
}

fn extract_sensitivity_result(
    value: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_statements_analytics::analysis::SensitivityResult> {
    if let Ok(result) = value.extract::<PyRef<'_, PySensitivityResult>>() {
        return Ok(result.inner.clone());
    }
    serde_json::from_str(value.extract::<&str>()?)
        .map_err(|e| serde_json_to_py(e, "invalid SensitivityResult JSON"))
}

fn extract_variance_config(
    value: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_statements_analytics::analysis::VarianceConfig> {
    if let Ok(config) = value.extract::<PyRef<'_, PyVarianceConfig>>() {
        return Ok(config.inner.clone());
    }
    serde_json::from_str(value.extract::<&str>()?)
        .map_err(|e| serde_json_to_py(e, "invalid VarianceConfig JSON"))
}

fn extract_scenario_set(
    value: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_statements_analytics::analysis::ScenarioSet> {
    if let Ok(scenario_set) = value.extract::<PyRef<'_, PyScenarioSet>>() {
        return Ok(scenario_set.inner.clone());
    }
    serde_json::from_str(value.extract::<&str>()?)
        .map_err(|e| serde_json_to_py(e, "invalid ScenarioSet JSON"))
}

/// Run sensitivity analysis on a financial model.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// config : SensitivityConfig | str
///     A typed configuration or its JSON serialization.
///
/// Returns
/// -------
/// SensitivityResult
///     Typed sensitivity result with per-scenario outputs, ``baseline`` and
///     DataFrame exits.
///
/// Raises
/// ------
/// ValueError
///     If the configuration is malformed or a scenario fails to evaluate.
/// KeyError
///     If a perturbed parameter or target metric is missing from the model.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import ParameterSpec, SensitivityConfig, run_sensitivity
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q2", None)
/// >>> b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)]); b.compute("profit", "revenue * 0.5")
/// >>> cfg = SensitivityConfig("diagonal", [ParameterSpec.with_percentages("revenue", "2025Q2", 110.0, [-10.0, 10.0])], ["profit"])
/// >>> len(run_sensitivity(b.build(), cfg))
/// 2
#[pyfunction]
fn run_sensitivity(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    config: &Bound<'_, PyAny>,
) -> PyResult<PySensitivityResult> {
    let model = extract_model_ref(model)?.into_owned();
    let config = extract_sensitivity_config(config)?;
    py.detach(move || {
        let analyzer =
            finstack_quant_statements_analytics::analysis::SensitivityAnalyzer::new(&model);
        let inner = analyzer.run(&config).map_err(statements_to_py)?;
        Ok(PySensitivityResult { inner })
    })
}

/// Generate tornado chart entries for a sensitivity result.
///
/// Parameters
/// ----------
/// result : SensitivityResult | str
///     A typed sensitivity result or its JSON serialization.
/// metric_node : str
///     Node to extract tornado entries for.
/// period : str | None
///     Optional period string to pin the tornado to.
///
/// Returns
/// -------
/// list[TornadoEntry]
///     Typed entries sorted by descending absolute swing.
///
/// Raises
/// ------
/// ValueError
///     If ``period`` does not parse or ``result`` is malformed JSON.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import ParameterSpec, SensitivityConfig, generate_tornado_entries, run_sensitivity
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q2", None)
/// >>> _ = b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
/// >>> _ = b.compute("profit", "revenue * 0.5")
/// >>> cfg = SensitivityConfig("diagonal", [ParameterSpec.with_percentages("revenue", "2025Q2", 110.0, [-10.0, 10.0])], ["profit"])
/// >>> entries = generate_tornado_entries(run_sensitivity(b.build(), cfg), "profit", "2025Q2")
/// >>> [entry.parameter_id for entry in entries]
/// ['revenue']
#[pyfunction]
#[pyo3(signature = (result, metric_node, period=None))]
fn generate_tornado_entries(
    result: &Bound<'_, PyAny>,
    metric_node: &str,
    period: Option<&str>,
) -> PyResult<Vec<PyTornadoEntry>> {
    let result = extract_sensitivity_result(result)?;
    let period_id: Option<finstack_quant_core::dates::PeriodId> = period
        .map(|p| p.parse().map_err(display_to_py))
        .transpose()?;
    Ok(
        finstack_quant_statements_analytics::analysis::generate_tornado_entries(
            &result,
            metric_node,
            period_id,
        )
        .into_iter()
        .map(PyTornadoEntry::from_inner)
        .collect(),
    )
}

/// Run variance analysis comparing two statement results.
///
/// Parameters
/// ----------
/// base : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// comparison : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// config : VarianceConfig | str
///     A typed configuration or its JSON serialization.
///
/// Returns
/// -------
/// VarianceReport
///     Per-metric, per-period rows including ``driver_contribution``.
///
/// Raises
/// ------
/// ValueError
///     If the configuration is malformed.
/// KeyError
///     If a configured metric is missing at a configured period.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import Evaluator, ModelBuilder
/// >>> from finstack_quant.statements_analytics import VarianceConfig, run_variance
/// >>> def model(revenue):
/// ...     b = ModelBuilder("m"); b.periods("2025Q1..Q1", None)
/// ...     b.value("revenue", [("2025Q1", revenue)]); b.compute("profit", "revenue * 0.5")
/// ...     return Evaluator().evaluate(b.build())
/// >>> cfg = VarianceConfig("base", "actual", ["profit"], ["2025Q1"])
/// >>> run_variance(model(100.0), model(120.0), cfg).rows[0].abs_var
/// 10.0
#[pyfunction]
fn run_variance(
    py: Python<'_>,
    base: &Bound<'_, PyAny>,
    comparison: &Bound<'_, PyAny>,
    config: &Bound<'_, PyAny>,
) -> PyResult<PyVarianceReport> {
    let base = extract_results_ref(base)?.into_owned();
    let comparison = extract_results_ref(comparison)?.into_owned();
    let config = extract_variance_config(config)?;
    py.detach(move || {
        let analyzer = finstack_quant_statements_analytics::analysis::VarianceAnalyzer::new(
            &base,
            &comparison,
        );
        let inner = analyzer.compute(&config).map_err(statements_to_py)?;
        Ok(PyVarianceReport { inner })
    })
}

/// Evaluate all scenarios in a scenario set.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// scenario_set : ScenarioSet | str
///     A typed scenario set or its JSON serialization.
///
/// Returns
/// -------
/// ScenarioResults
///     Typed mapping of scenario names to statement results.
///
/// Raises
/// ------
/// ValueError
///     If the set is empty, a parent chain cycles, an override is
///     incompatible with its node, or evaluation fails.
/// KeyError
///     If an override names a node missing from the model.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import ScenarioSet, evaluate_scenario_set
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q2", None); b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
/// >>> results = evaluate_scenario_set(b.build(), ScenarioSet({"base": {}, "down": {"revenue": 90.0}}))
/// >>> results.names
/// ['base', 'down']
#[pyfunction]
fn evaluate_scenario_set(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    scenario_set: &Bound<'_, PyAny>,
) -> PyResult<PyScenarioResults> {
    let model = extract_model_ref(model)?.into_owned();
    let scenario_set = extract_scenario_set(scenario_set)?;
    py.detach(move || {
        let inner = scenario_set
            .evaluate_all(&model)
            .map_err(statements_to_py)?;
        Ok(PyScenarioResults { inner })
    })
}

/// Compare two evaluated scenarios metric-by-metric.
///
/// Parameters
/// ----------
/// scenario_set : ScenarioSet | str
///     A typed scenario set or its JSON serialization.
/// results : ScenarioResults
///     Output of ``evaluate_scenario_set`` for the same scenario set.
/// baseline : str
///     Name of the scenario to treat as the baseline.
/// comparison : str
///     Name of the scenario to compare against the baseline.
/// metrics : list[str]
///     Node identifiers to compare. Must be non-empty.
/// periods : list[str]
///     Period identifiers (e.g. ``"2025Q1"``). Must be non-empty.
///
/// Returns
/// -------
/// ScenarioDiff
///     Baseline and comparison names alongside the variance report.
///
/// Raises
/// ------
/// ValueError
///     If ``metrics`` or ``periods`` is empty, a scenario name is unknown, or
///     a period fails to parse.
/// KeyError
///     If a metric is missing at a period in either scenario.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import ScenarioSet, evaluate_scenario_set, scenario_diff
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q1", None)
/// >>> _ = b.value("revenue", [("2025Q1", 100.0)])
/// >>> _ = b.compute("profit", "revenue * 0.5")
/// >>> model = b.build()
/// >>> scenarios = ScenarioSet({"base": {}, "down": {"revenue": 90.0}})
/// >>> results = evaluate_scenario_set(model, scenarios)
/// >>> scenario_diff(scenarios, results, "base", "down", ["profit"], ["2025Q1"]).variance.rows[0].abs_var
/// -5.0
#[pyfunction]
#[pyo3(text_signature = "(scenario_set, results, baseline, comparison, metrics, periods)")]
fn scenario_diff(
    py: Python<'_>,
    scenario_set: &Bound<'_, PyAny>,
    results: PyRef<'_, PyScenarioResults>,
    baseline: &str,
    comparison: &str,
    metrics: Vec<String>,
    periods: Vec<String>,
) -> PyResult<PyScenarioDiff> {
    let scenario_set = extract_scenario_set(scenario_set)?;
    let results = results.inner.clone();
    let periods = periods
        .iter()
        .map(|period| period.parse().map_err(display_to_py))
        .collect::<PyResult<Vec<_>>>()?;
    let baseline = baseline.to_string();
    let comparison = comparison.to_string();
    py.detach(move || {
        let inner = scenario_set
            .diff(&results, &baseline, &comparison, &metrics, &periods)
            .map_err(statements_to_py)?;
        Ok(PyScenarioDiff { inner })
    })
}

/// Decompose a metric's scenario variance across named drivers.
///
/// Driver contributions are raw deltas in *driver* units rather than
/// sensitivities of the target metric, so they generally do not sum to the
/// target variance. The gap is reported in ``BridgeChart.unexplained``.
///
/// Parameters
/// ----------
/// base : StatementResult | str
///     Baseline evaluated statement result, or its JSON serialization.
/// comparison : StatementResult | str
///     Comparison evaluated statement result, or its JSON serialization.
/// target_metric : str
///     Node identifier whose variance is being explained.
/// period : str
///     Period identifier (e.g. ``"2025Q4"``).
/// drivers : list[str]
///     Node identifiers treated as explanatory drivers.
/// baseline_label : str
///     Display label for the baseline column.
/// comparison_label : str
///     Display label for the comparison column.
///
/// Returns
/// -------
/// BridgeChart
///     Ordered driver contributions plus the unexplained residual.
///
/// Raises
/// ------
/// ValueError
///     If the period fails to parse.
/// KeyError
///     If the target or any driver is missing from either result at ``period``.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import Evaluator, ModelBuilder
/// >>> from finstack_quant.statements_analytics import variance_bridge
/// >>> def model(revenue):
/// ...     b = ModelBuilder("m"); b.periods("2025Q1..Q1", None)
/// ...     b.value("revenue", [("2025Q1", revenue)]); b.compute("profit", "revenue * 0.5")
/// ...     return Evaluator().evaluate(b.build())
/// >>> chart = variance_bridge(model(100.0), model(120.0), "profit", "2025Q1", ["revenue"], "base", "actual")
/// >>> [(step.driver, step.contribution) for step in chart.steps], chart.unexplained
/// ([('revenue', 20.0)], -10.0)
#[pyfunction]
#[pyo3(
    text_signature = "(base, comparison, target_metric, period, drivers, baseline_label, comparison_label)"
)]
#[allow(clippy::too_many_arguments)]
fn variance_bridge(
    py: Python<'_>,
    base: &Bound<'_, PyAny>,
    comparison: &Bound<'_, PyAny>,
    target_metric: &str,
    period: &str,
    drivers: Vec<String>,
    baseline_label: &str,
    comparison_label: &str,
) -> PyResult<PyBridgeChart> {
    let base = extract_results_ref(base)?.into_owned();
    let comparison = extract_results_ref(comparison)?.into_owned();
    let period = period.parse().map_err(display_to_py)?;
    let target_metric = target_metric.to_string();
    let baseline_label = baseline_label.to_string();
    let comparison_label = comparison_label.to_string();
    py.detach(move || {
        let analyzer = finstack_quant_statements_analytics::analysis::VarianceAnalyzer::new(
            &base,
            &comparison,
        );
        let driver_refs: Vec<&str> = drivers.iter().map(String::as_str).collect();
        let inner = analyzer
            .bridge_decomposition(
                &target_metric,
                period,
                &driver_refs,
                &baseline_label,
                &comparison_label,
            )
            .map_err(statements_to_py)?;
        Ok(PyBridgeChart { inner })
    })
}

/// Forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import backtest_forecast
/// >>> metrics = backtest_forecast([100.0, 110.0], [98.0, 112.0])
/// >>> metrics.n, metrics.mae
/// (2, 2.0)
#[pyclass(
    name = "ForecastMetrics",
    module = "finstack_quant.statements_analytics",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyForecastMetrics {
    pub(crate) inner: ForecastMetrics,
}

#[pymethods]
impl PyForecastMetrics {
    /// Mean absolute error in data units.
    #[getter]
    fn mae(&self) -> f64 {
        self.inner.mae
    }

    /// Mean absolute percentage error in percent (``5.0`` = 5%); ``NaN``
    /// when every actual is zero.
    #[getter]
    fn mape(&self) -> f64 {
        self.inner.mape
    }

    /// Number of observations with a non-zero actual used by ``mape``.
    #[getter]
    fn mape_effective_n(&self) -> usize {
        self.inner.mape_effective_n
    }

    /// Symmetric MAPE in percent.
    #[getter]
    fn smape(&self) -> f64 {
        self.inner.smape
    }

    /// Root mean squared error in data units.
    #[getter]
    fn rmse(&self) -> f64 {
        self.inner.rmse
    }

    /// Number of observations.
    #[getter]
    fn n(&self) -> usize {
        self.inner.n
    }

    /// One-line human-readable summary (Rust ``ForecastMetrics::summary``).
    fn summary(&self) -> String {
        self.inner.summary()
    }

    /// Export as a pandas ``Series`` indexed by metric name.
    ///
    /// Index: ``mae``, ``mape``, ``mape_effective_n``, ``smape``, ``rmse``,
    /// ``n``; counts are cast to float.
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = ["mae", "mape", "mape_effective_n", "smape", "rmse", "n"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let values = vec![
            self.inner.mae,
            self.inner.mape,
            self.inner.mape_effective_n as f64,
            self.inner.smape,
            self.inner.rmse,
            self.inner.n as f64,
        ];
        labeled_values_to_series(py, &labels, values, "forecast_metrics")
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "ForecastMetrics"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``ForecastMetrics`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid ForecastMetrics JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ForecastMetrics", &self.inner)
    }
}

/// Compute forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).
///
/// Parameters
/// ----------
/// actual : list[float]
///     Observed values.
/// forecast : list[float]
///     Forecast values; same length as ``actual``.
///
/// Returns
/// -------
/// ForecastMetrics
///     Typed metrics with ``summary()`` and ``to_series()``.
///
/// Raises
/// ------
/// ValueError
///     If the sequences are empty or of different lengths.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import backtest_forecast
/// >>> backtest_forecast([1.0, 2.0], [1.0, 2.5]).n
/// 2
#[pyfunction]
#[pyo3(text_signature = "(actual, forecast)")]
fn backtest_forecast(actual: Vec<f64>, forecast: Vec<f64>) -> PyResult<PyForecastMetrics> {
    let inner =
        finstack_quant_statements_analytics::analysis::backtest_forecast(&actual, &forecast)
            .map_err(statements_to_py)?;
    Ok(PyForecastMetrics { inner })
}

/// Result of a goal-seek solve.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import goal_seek
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q1", None)
/// >>> _ = b.value("revenue", [("2025Q1", 100.0)])
/// >>> _ = b.compute("profit", "revenue * 0.5")
/// >>> result = goal_seek(b.build(), "profit", "2025Q1", 60.0, "revenue", "2025Q1")
/// >>> round(result.solved_value, 6), result.model is None
/// (120.0, False)
#[pyclass(
    name = "GoalSeekResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyGoalSeekResult {
    solved_value: f64,
    model: Option<finstack_quant_statements::FinancialModelSpec>,
}

#[pymethods]
impl PyGoalSeekResult {
    /// Driver value that reaches the target.
    #[getter]
    fn solved_value(&self) -> f64 {
        self.solved_value
    }

    /// Model with the solved driver written in, or ``None`` when
    /// ``update_model=False``.
    #[getter]
    fn model(&self) -> Option<PyFinancialModelSpec> {
        self.model.clone().map(PyFinancialModelSpec::from_inner)
    }

    fn __float__(&self) -> f64 {
        self.solved_value
    }

    fn __repr__(&self) -> String {
        format!(
            "GoalSeekResult(solved_value={}, model={})",
            self.solved_value,
            if self.model.is_some() {
                "FinancialModelSpec(...)"
            } else {
                "None"
            }
        )
    }
}

/// Find the driver value that makes a target node reach a target value.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// target_node : str
///     Node to drive towards ``target_value``.
/// target_period : str
///     Period string for the target (e.g. ``"2025Q4"``).
/// target_value : float
///     Desired value for the target node.
/// driver_node : str
///     Node whose value is adjusted to reach the target.
/// driver_period : str
///     Period string for the driver.
/// update_model : bool
///     If ``True``, the solved value is written back into the returned model.
///     Default ``True``.
/// bounds : tuple[float, float] | None
///     Optional search bounds ``(lo, hi)``; bisection is used when set.
///
/// Returns
/// -------
/// GoalSeekResult
///     ``solved_value`` plus ``model`` (the updated ``FinancialModelSpec`` or
///     ``None``). ``float(result)`` yields the solved value.
///
/// Raises
/// ------
/// ValueError
///     If a period does not parse, the solver fails to converge, or the
///     bracket does not contain a root, or the final objective residual exceeds 1e-9 times max(1, abs(target_value)). Failure leaves the model unchanged.
/// KeyError
///     If ``target_node`` or ``driver_node`` is missing from the model.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import goal_seek
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q1", None); b.value("revenue", [("2025Q1", 100.0)])
/// >>> b.compute("profit", "revenue * 0.5")
/// >>> round(goal_seek(b.build(), "profit", "2025Q1", 60.0, "revenue", "2025Q1").solved_value, 6)
/// 120.0
#[pyfunction]
#[pyo3(signature = (model, target_node, target_period, target_value, driver_node, driver_period, update_model=true, bounds=None))]
#[allow(clippy::too_many_arguments)]
fn goal_seek(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    target_node: &str,
    target_period: &str,
    target_value: f64,
    driver_node: &str,
    driver_period: &str,
    update_model: bool,
    bounds: Option<(f64, f64)>,
) -> PyResult<PyGoalSeekResult> {
    let mut model = extract_model_ref(model)?.into_owned();
    let tp: finstack_quant_core::dates::PeriodId = target_period.parse().map_err(display_to_py)?;
    let dp: finstack_quant_core::dates::PeriodId = driver_period.parse().map_err(display_to_py)?;
    let target_node = target_node.to_owned();
    let driver_node = driver_node.to_owned();

    py.detach(move || {
        let solved_value = finstack_quant_statements_analytics::analysis::goal_seek(
            &mut model,
            &target_node,
            tp,
            target_value,
            &driver_node,
            dp,
            update_model,
            bounds,
        )
        .map_err(statements_to_py)?;

        Ok(PyGoalSeekResult {
            solved_value,
            model: update_model.then_some(model),
        })
    })
}

/// Cached dependency tracer that builds the model graph once.
///
/// Construct from a ``FinancialModelSpec`` (or JSON string) and reuse for
/// multiple introspection queries without rebuilding the dependency graph.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import DependencyTracer
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q1", None); b.value("revenue", [("2025Q1", 100.0)])
/// >>> b.compute("profit", "revenue * 0.5")
/// >>> DependencyTracer(b.build()).direct_dependencies("profit")
/// ['revenue']
#[pyclass(
    name = "DependencyTracer",
    module = "finstack_quant.statements_analytics",
    skip_from_py_object
)]
struct PyDependencyTracer {
    model: finstack_quant_statements::FinancialModelSpec,
    graph: finstack_quant_statements::evaluator::DependencyGraph,
}

#[pymethods]
impl PyDependencyTracer {
    /// Build a tracer from a model (typed object or JSON string).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the model JSON is malformed or the graph contains a cycle.
    #[new]
    fn new(model: &Bound<'_, PyAny>) -> PyResult<Self> {
        let model = extract_model_ref(model)?.into_owned();
        let graph = finstack_quant_statements::evaluator::DependencyGraph::from_model(&model)
            .map_err(statements_to_py)?;
        Ok(Self { model, graph })
    }

    /// ASCII-formatted dependency tree for a node.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    fn dependency_tree(&self, node_id: &str) -> PyResult<String> {
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let tree = tracer.dependency_tree(node_id).map_err(statements_to_py)?;
        Ok(finstack_quant_statements_analytics::analysis::render_tree_ascii(&tree))
    }

    /// ASCII tree with node values for a given period.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    /// ValueError
    ///     If ``period`` does not parse.
    fn dependency_tree_detailed(
        &self,
        results: &Bound<'_, PyAny>,
        node_id: &str,
        period: &str,
    ) -> PyResult<String> {
        let results = extract_results_ref(results)?;
        let pid: finstack_quant_core::dates::PeriodId = period.parse().map_err(display_to_py)?;
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let tree = tracer.dependency_tree(node_id).map_err(statements_to_py)?;
        Ok(
            finstack_quant_statements_analytics::analysis::render_tree_detailed(
                &tree, &results, &pid,
            ),
        )
    }

    /// Direct dependency node IDs.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    fn direct_dependencies(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let deps = tracer
            .direct_dependencies(node_id)
            .map_err(statements_to_py)?;
        Ok(deps.into_iter().map(String::from).collect())
    }

    /// All transitive dependency node IDs in dependency order.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    fn all_dependencies(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        tracer.all_dependencies(node_id).map_err(statements_to_py)
    }

    /// Node IDs that depend on this node.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the model.
    fn dependents(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_quant_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let deps = tracer.dependents(node_id).map_err(statements_to_py)?;
        Ok(deps.into_iter().map(String::from).collect())
    }

    fn __repr__(&self) -> String {
        format!("DependencyTracer(nodes={})", self.model.nodes.len())
    }
}

/// One component of a formula explanation.
#[pyclass(
    name = "ExplanationStep",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyExplanationStep {
    pub(crate) inner: ExplanationStep,
}

#[pymethods]
impl PyExplanationStep {
    /// Component node id or literal text.
    #[getter]
    fn component(&self) -> &str {
        &self.inner.component
    }

    /// Component value at the explained period.
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value
    }

    /// Operation applied to the component (``"+"``, ``"*"``, ...), or ``None``.
    #[getter]
    fn operation(&self) -> Option<&str> {
        self.inner.operation.as_deref()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ExplanationStep", &self.inner)
    }
}

/// Explanation of how a node's value was derived at one period.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import Explanation
/// >>> e = Explanation.from_json('{"node_id":"profit","period_id":"2025Q1","final_value":50.0,'
/// ...     '"node_type":"calculated","formula_text":"revenue * 0.5","breakdown":[]}')
/// >>> e.node_id, e.final_value
/// ('profit', 50.0)
#[pyclass(
    name = "Explanation",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyExplanation {
    pub(crate) inner: Explanation,
}

#[pymethods]
impl PyExplanation {
    /// Explained node id.
    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Period-id string of the explanation.
    #[getter]
    fn period_id(&self) -> String {
        self.inner.period_id.to_string()
    }

    /// Node value at the period.
    #[getter]
    fn final_value(&self) -> f64 {
        self.inner.final_value
    }

    /// Node type serde name (e.g. ``"calculated"``, ``"input"``).
    #[getter]
    fn node_type(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.node_type)
            .map_err(crate::errors::core_to_py)
    }

    /// Formula text, or ``None`` for non-formula nodes.
    #[getter]
    fn formula_text(&self) -> Option<&str> {
        self.inner.formula_text.as_deref()
    }

    /// Component breakdown in evaluation order.
    #[getter]
    fn breakdown(&self) -> Vec<PyExplanationStep> {
        self.inner
            .breakdown
            .iter()
            .cloned()
            .map(|inner| PyExplanationStep { inner })
            .collect()
    }

    /// Human-readable multi-line explanation.
    fn to_text(&self) -> String {
        self.inner.to_string_detailed()
    }

    /// Export the breakdown as a pandas ``DataFrame``.
    ///
    /// Columns: ``component``, ``value``, ``operation`` (``None`` when absent).
    /// One row per breakdown step in evaluation order.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .breakdown
            .iter()
            .map(|step| {
                serde_json::json!({
                    "component": step.component,
                    "value": step.value,
                    "operation": step.operation,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &EXPLANATION_COLUMNS)
    }

    /// Serialize to canonical JSON (identical to the WASM ``explainFormula`` output).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "Explanation"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``Explanation`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid Explanation JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("Explanation", &self.inner)
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Explain a formula for a specific node and period.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// results : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// node_id : str
///     Node whose formula to explain.
/// period : str
///     Period string.
///
/// Returns
/// -------
/// Explanation
///     Typed explanation with ``breakdown`` steps, ``to_text()`` and
///     ``to_dataframe()``; ``to_json()`` matches the WASM ``explainFormula``.
///
/// Raises
/// ------
/// KeyError
///     If ``node_id`` is not in the model or has no value at ``period``.
/// ValueError
///     If ``period`` does not parse or a payload is malformed JSON.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder, Evaluator
/// >>> from finstack_quant.statements_analytics import explain_formula
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q1", None); b.value("revenue", [("2025Q1", 100.0)])
/// >>> b.compute("profit", "revenue * 0.5")
/// >>> model = b.build()
/// >>> explain_formula(model, Evaluator().evaluate(model), "profit", "2025Q1").final_value
/// 50.0
#[pyfunction]
fn explain_formula(
    model: &Bound<'_, PyAny>,
    results: &Bound<'_, PyAny>,
    node_id: &str,
    period: &str,
) -> PyResult<PyExplanation> {
    let model = extract_model_ref(model)?;
    let results = extract_results_ref(results)?;
    let pid: finstack_quant_core::dates::PeriodId = period.parse().map_err(display_to_py)?;

    let explainer =
        finstack_quant_statements_analytics::analysis::FormulaExplainer::new(&model, &results);
    let inner = explainer.explain(node_id, &pid).map_err(statements_to_py)?;
    Ok(PyExplanation { inner })
}

/// Get a detailed text explanation for a formula.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// results : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// node_id : str
///     Node whose formula to explain.
/// period : str
///     Period string.
///
/// Returns
/// -------
/// str
///     Human-readable multi-line explanation (``explain_formula(...).to_text()``).
///
/// Raises
/// ------
/// KeyError
///     If ``node_id`` is not in the model or has no value at ``period``.
/// ValueError
///     If ``period`` does not parse or a payload is malformed JSON.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import Evaluator, ModelBuilder
/// >>> from finstack_quant.statements_analytics import explain_formula_text
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q1", None)
/// >>> _ = b.value("revenue", [("2025Q1", 100.0)])
/// >>> _ = b.compute("profit", "revenue * 0.5")
/// >>> model = b.build()
/// >>> explain_formula_text(model, Evaluator().evaluate(model), "profit", "2025Q1").splitlines()[0]
/// 'profit [2025Q1] = 50.00'
#[pyfunction]
fn explain_formula_text(
    model: &Bound<'_, PyAny>,
    results: &Bound<'_, PyAny>,
    node_id: &str,
    period: &str,
) -> PyResult<String> {
    Ok(explain_formula(model, results, node_id, period)?.to_text())
}

/// Register analysis functions and classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDependencyTracer>()?;
    m.add_class::<PyForecastMetrics>()?;
    m.add_class::<PyGoalSeekResult>()?;
    m.add_class::<PyExplanationStep>()?;
    m.add_class::<PyExplanation>()?;
    m.add_function(pyo3::wrap_pyfunction!(run_sensitivity, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(generate_tornado_entries, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_variance, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_scenario_set, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_diff, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(variance_bridge, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(backtest_forecast, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(goal_seek, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(explain_formula, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(explain_formula_text, m)?)?;
    Ok(())
}
