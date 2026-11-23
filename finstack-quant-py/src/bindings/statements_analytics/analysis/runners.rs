//! Sensitivity, variance, scenario, backtest, and goal-seek Python runners.

use crate::bindings::core::table::PyLabeledSeries;
use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::bindings::macros::wire_methods;
use crate::bindings::statements::parse_period_id;
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::bindings::statements_analytics::typed::{
    PyBridgeChart, PyScenarioDiff, PyScenarioResults, PyScenarioSet, PySensitivityConfig,
    PySensitivityResult, PyTornadoEntry, PyVarianceConfig, PyVarianceReport,
};
use crate::errors::{serde_json_to_py, statements_to_py};
use finstack_quant_statements_analytics::analysis::{ForecastMetrics, GoalSeekResult};
use pyo3::prelude::*;

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
/// Each perturbation replaces its node's value in its one forecast period
/// (Value > Forecast > Formula). It does not re-anchor the node's own
/// forecast: later periods of a growth, seasonal or stochastic forecast keep
/// their unshocked base. Formula nodes that read the shocked cell move, in
/// its period and in later periods reached through ``lag``, ``ttm`` or
/// ``cumsum``. Add one parameter per period to shock a whole forecast path.
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
///     If the configuration is malformed, has duplicate parameters or
///     non-finite values, exceeds 128 parameters, 10,000 scenarios or
///     10 million model node-period evaluation cells, references a missing
///     perturbed parameter or target metric, or a scenario fails.
/// KeyError
///     If model evaluation cannot find required data or a formula reference.
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
pub(super) fn run_sensitivity(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    config: &Bound<'_, PyAny>,
) -> PyResult<PySensitivityResult> {
    let model = extract_model_ref(model)?.into_owned();
    let config = extract_sensitivity_config(config)?;
    py.detach(move || {
        let inner = finstack_quant_statements_analytics::analysis::run_sensitivity(&model, &config)
            .map_err(statements_to_py)?;
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
/// period : str
///     Period at which the metric is read in the baseline and every scenario
///     (e.g. ``"2025Q4"``). Required: a shock replaces one forecast cell, so
///     the metric moves only in the shocked period and in later periods whose
///     formulas read it.
///
/// Returns
/// -------
/// list[TornadoEntry] or None
///     Typed entries sorted by descending absolute swing, with parameter IDs
///     preserving the perturbed ``node@period`` key; ``None`` for a full-grid
///     run, whose scenarios combine parameters and cannot be ranked.
///
/// Raises
/// ------
/// ValueError
///     If ``period`` does not parse, ``result`` is malformed JSON, a
///     one-at-a-time run's scenario does not contain exactly one perturbed
///     parameter, its unperturbed baseline is absent, the target metric/period
///     is missing, or a metric value or impact is non-finite.
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
/// ['revenue@2025Q2']
#[pyfunction]
#[pyo3(text_signature = "(result, metric_node, period)")]
pub(super) fn generate_tornado_entries(
    result: &Bound<'_, PyAny>,
    metric_node: &str,
    period: &str,
) -> PyResult<Option<Vec<PyTornadoEntry>>> {
    let result = extract_sensitivity_result(result)?;
    let period_id = parse_period_id(period)?;
    let entries = finstack_quant_statements_analytics::analysis::generate_tornado_entries(
        &result,
        metric_node,
        period_id,
    )
    .map_err(statements_to_py)?;
    Ok(entries.map(|entries| {
        entries
            .into_iter()
            .map(PyTornadoEntry::from_inner)
            .collect()
    }))
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
///     Per-metric, per-period variance rows.
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
pub(super) fn run_variance(
    py: Python<'_>,
    base: &Bound<'_, PyAny>,
    comparison: &Bound<'_, PyAny>,
    config: &Bound<'_, PyAny>,
) -> PyResult<PyVarianceReport> {
    let base = extract_results_ref(base)?.into_owned();
    let comparison = extract_results_ref(comparison)?.into_owned();
    let config = extract_variance_config(config)?;
    py.detach(move || {
        let inner = finstack_quant_statements_analytics::analysis::run_variance(
            &base,
            &comparison,
            &config,
        )
        .map_err(statements_to_py)?;
        Ok(PyVarianceReport { inner })
    })
}

/// Evaluate all scenarios in a scenario set.
///
/// Like ``Evaluator.evaluate``, a capital structure is evaluated from an
/// empty market, which serves fixed-rate instruments; use
/// ``evaluate_scenario_set_with_market`` for instruments that need market
/// data.
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
///     incompatible with its node, a forecast override has invalid
///     parameters, or evaluation fails.
/// KeyError
///     If an override names a node missing from the model, or a
///     capital-structure instrument needs market data.
/// RuntimeError
///     If capital-structure cashflow generation or the waterfall fails.
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
pub(super) fn evaluate_scenario_set(
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

/// Evaluate all scenarios in a scenario set against a market as of a date.
///
/// Each scenario is evaluated like ``Evaluator.evaluate_with_market``: the
/// capital structure's instruments are priced from ``market`` as of
/// ``as_of`` (floating coupons, swaps, caps), so every result carries
/// ``cs_cashflows``, and explicit actuals not yet available at ``as_of`` are
/// hidden. The market is shared unchanged by every scenario.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// scenario_set : ScenarioSet | str
///     A typed scenario set or its JSON serialization.
/// market : MarketContext | str
///     Curves, fixings and FX the instruments are priced from, typed or its
///     canonical JSON.
/// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date for the instruments' cashflows and the cutoff for
///     explicit-value visibility; ISO ``YYYY-MM-DD`` strings are accepted.
///
/// Returns
/// -------
/// ScenarioResults
///     Typed mapping of scenario names to statement results.
///
/// Raises
/// ------
/// ValueError
///     If ``as_of`` is not date-like, the set is empty, a parent chain
///     cycles, an override is incompatible with its node, a forecast override
///     has invalid parameters, or evaluation fails.
/// KeyError
///     If an override names a node missing from the model, or an instrument
///     needs market data ``market`` lacks.
/// RuntimeError
///     If capital-structure cashflow generation or the waterfall fails.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.market_data import MarketContext
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import ScenarioSet, evaluate_scenario_set_with_market
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q2", None)
/// >>> _ = b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
/// >>> results = evaluate_scenario_set_with_market(
/// ...     b.build(), ScenarioSet({"base": {}, "down": {"revenue": 90.0}}), MarketContext(), "2025-01-01"
/// ... )
/// >>> results.names
/// ['base', 'down']
#[pyfunction]
#[pyo3(text_signature = "(model, scenario_set, market, as_of)")]
pub(super) fn evaluate_scenario_set_with_market(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    scenario_set: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<PyScenarioResults> {
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    let model = extract_model_ref(model)?;
    let market = crate::bindings::extract::extract_market_ref(py, market)?;
    let scenario_set = extract_scenario_set(scenario_set)?;
    let model_inner: &finstack_quant_statements::types::FinancialModelSpec = &model;
    let market_inner: &finstack_quant_core::market_data::context::MarketContext = &market;
    let inner = py
        .detach(|| scenario_set.evaluate_all_with_market(model_inner, market_inner, as_of))
        .map_err(statements_to_py)?;
    Ok(PyScenarioResults { inner })
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
pub(super) fn scenario_diff(
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
        .map(|period| parse_period_id(period))
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
pub(super) fn variance_bridge(
    base: &Bound<'_, PyAny>,
    comparison: &Bound<'_, PyAny>,
    target_metric: &str,
    period: &str,
    drivers: Vec<String>,
    baseline_label: &str,
    comparison_label: &str,
) -> PyResult<PyBridgeChart> {
    let py = base.py();
    let base = extract_results_ref(base)?.into_owned();
    let comparison = extract_results_ref(comparison)?.into_owned();
    let period = parse_period_id(period)?;
    let target_metric = target_metric.to_string();
    let baseline_label = baseline_label.to_string();
    let comparison_label = comparison_label.to_string();
    py.detach(move || {
        let driver_refs: Vec<&str> = drivers.iter().map(String::as_str).collect();
        let inner = finstack_quant_statements_analytics::analysis::variance_bridge(
            &base,
            &comparison,
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

    /// Mean absolute percentage error in percent (``5.0`` = 5%); ``None``
    /// when every absolute actual is below ``1e-10`` and excluded from MAPE.
    #[getter]
    fn mape(&self) -> Option<f64> {
        self.inner.mape
    }

    /// Number of observations with a non-zero actual used by ``mape``.
    #[getter]
    fn mape_effective_n(&self) -> usize {
        self.inner.mape_effective_n
    }

    /// Symmetric MAPE in percent, or ``None`` when every denominator
    /// ``(abs(actual) + abs(forecast)) / 2`` is below ``1e-10``.
    #[getter]
    fn smape(&self) -> Option<f64> {
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

    /// Export as a ``LabeledSeries`` indexed by metric name.
    ///
    /// Index: ``mae``, ``mape``, ``mape_effective_n``, ``smape``, ``rmse``,
    /// ``n``; counts are represented as floats and unavailable percentages
    /// become null (``NaN`` in ``to_pandas()``).
    fn to_series(&self) -> PyResult<PyLabeledSeries> {
        let labels: Vec<String> = ["mae", "mape", "mape_effective_n", "smape", "rmse", "n"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let values = vec![
            Some(self.inner.mae),
            self.inner.mape,
            Some(self.inner.mape_effective_n as f64),
            self.inner.smape,
            Some(self.inner.rmse),
            Some(self.inner.n as f64),
        ];
        Ok(
            PyLabeledSeries::from_opt_f64("forecast_metrics", labels, values)?
                .with_index_name("metric"),
        )
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ForecastMetrics", &self.inner)
    }
}

wire_methods!(PyForecastMetrics, ForecastMetrics, "ForecastMetrics");

/// Compute forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).
///
/// Parameters
/// ----------
/// actual : list[float]
///     Finite observed values in consistent units; negative values are accepted.
/// forecast : list[float]
///     Finite forecast values in the same units and aligned with ``actual``.
///
/// Returns
/// -------
/// ForecastMetrics
///     Typed metrics with ``summary()`` and ``to_series()``. ``mape`` and
///     ``smape`` are ``None`` when no denominator contributes; JSON emits ``null``.
///
/// Raises
/// ------
/// ValueError
///     If the sequences are empty, of different lengths, contain a non-finite
///     value, or metric arithmetic overflows.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import backtest_forecast
/// >>> backtest_forecast([1.0, 2.0], [1.0, 2.5]).n
/// 2
#[pyfunction]
#[pyo3(text_signature = "(actual, forecast)")]
pub(super) fn backtest_forecast(
    actual: Vec<f64>,
    forecast: Vec<f64>,
) -> PyResult<PyForecastMetrics> {
    let inner =
        finstack_quant_statements_analytics::analysis::backtest_forecast(&actual, &forecast)
            .map_err(statements_to_py)?;
    Ok(PyForecastMetrics { inner })
}

#[pyclass(
    name = "GoalSeekSpec",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekSpec")]
#[derive(Clone)]
pub(super) struct PyGoalSeekSpec {
    inner: finstack_quant_statements_analytics::analysis::GoalSeekSpec,
}

#[pymethods]
impl PyGoalSeekSpec {
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, driver, target, target_value, target_tolerance, driver_tolerance, max_iterations, search, as_of, observed_through, update_model)"
    )]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekSpec.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: finstack_quant_statements_analytics::analysis::GoalSeekSpec =
            crate::bindings::module_utils::fields_from_py(py, fields, "GoalSeekSpec")?;
        inner.validate().map_err(statements_to_py)?;
        Ok(Self { inner })
    }
}

#[pyclass(
    name = "GoalSeekResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult")]
#[derive(Clone)]
pub struct PyGoalSeekResult {
    pub(crate) inner: GoalSeekResult,
}

fn goal_value_to_py<'py>(
    py: Python<'py>,
    value: finstack_quant_statements::types::AmountOrScalar,
) -> PyResult<Bound<'py, PyAny>> {
    use finstack_quant_statements::types::AmountOrScalar;
    match value {
        AmountOrScalar::Amount(amount) => Ok(Bound::new(
            py,
            crate::bindings::core::money::PyMoney::from_inner(amount),
        )?
        .into_any()),
        AmountOrScalar::Scalar(scalar) => Ok(pyo3::types::PyFloat::new(py, scalar).into_any()),
    }
}

#[pymethods]
impl PyGoalSeekResult {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.solved_value")]
    fn solved_value<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        goal_value_to_py(py, self.inner.solved_value)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.target_value")]
    fn target_value<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        goal_value_to_py(py, self.inner.target_value)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.achieved_value")]
    fn achieved_value<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        goal_value_to_py(py, self.inner.achieved_value)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.residual")]
    fn residual<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        goal_value_to_py(py, self.inner.residual)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.target_tolerance")]
    fn target_tolerance<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        goal_value_to_py(py, self.inner.target_tolerance)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.driver_tolerance")]
    fn driver_tolerance(&self) -> f64 {
        self.inner.driver_tolerance
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.evaluations")]
    fn evaluations(&self) -> usize {
        self.inner.evaluations
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.original_run")]
    fn original_run<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.original_run)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.solved_run")]
    fn solved_run<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.solved_run)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.market_hash")]
    fn market_hash(&self) -> &str {
        &self.inner.market_hash
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.model")]
    fn model(&self) -> Option<PyFinancialModelSpec> {
        self.inner
            .model
            .clone()
            .map(PyFinancialModelSpec::from_inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|error| serde_json_to_py(error, "GoalSeekResult"))
    }

    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.from_json")]
    fn from_json(json: &str) -> PyResult<Self> {
        GoalSeekResult::from_json(json)
            .map(|inner| Self { inner })
            .map_err(statements_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "GoalSeekResult.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._analysis.runners", "goal_seek")]
#[pyfunction]
#[pyo3(signature = (model, spec, market=None))]
pub(super) fn goal_seek(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    spec: &PyGoalSeekSpec,
    market: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyGoalSeekResult> {
    let model = extract_model_ref(model)?.into_owned();
    let spec = spec.inner.clone();
    let market = crate::bindings::extract::extract_market_opt(py, market)?.unwrap_or_default();
    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::goal_seek(&model, &spec, &market)
        })
        .map_err(statements_to_py)?;
    Ok(PyGoalSeekResult { inner })
}
