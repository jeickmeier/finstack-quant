//! Python wrappers for statements analytics functions.
//!
//! Covers: sensitivity, variance, scenario sets, backtesting, goal seek,
//! introspection (dependency tracing, formula explanation), DCF valuation,
//! credit analysis, Monte Carlo, and reports.
//!
//! All functions that accept a financial model or statement result support
//! both JSON strings and typed Python objects (`FinancialModelSpec`,
//! `StatementResult`) for zero-overhead calls when the caller already has
//! a parsed object.

use crate::bindings::extract::{extract_market_opt, extract_model_ref, extract_results_ref};
use crate::errors::display_to_py;
use finstack_statements_analytics::analysis::CorporateValuationResult;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

fn dcf_equity_result_dict<'py>(
    py: Python<'py>,
    result: &CorporateValuationResult,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("equity_value", result.equity_value.amount())?;
    dict.set_item(
        "equity_currency",
        result.equity_value.currency().to_string(),
    )?;
    dict.set_item("enterprise_value", result.enterprise_value.amount())?;
    dict.set_item("net_debt", result.net_debt.amount())?;
    dict.set_item("terminal_value_pv", result.terminal_value_pv.amount())?;
    dict.set_item("equity_value_per_share", result.equity_value_per_share)?;
    dict.set_item("diluted_shares", result.diluted_shares)?;
    Ok(dict)
}

// ---------------------------------------------------------------------------
// Sensitivity analysis
// ---------------------------------------------------------------------------

/// Run sensitivity analysis on a financial model.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// config_json : str
///     JSON-serialized ``SensitivityConfig``.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``SensitivityResult``.
#[pyfunction]
fn run_sensitivity(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    config_json: &str,
) -> PyResult<String> {
    let model = extract_model_ref(model)?.into_owned();
    let config: finstack_statements_analytics::analysis::SensitivityConfig =
        serde_json::from_str(config_json).map_err(display_to_py)?;
    py.detach(move || {
        let analyzer = finstack_statements_analytics::analysis::SensitivityAnalyzer::new(&model);
        let result = analyzer.run(&config).map_err(display_to_py)?;
        serde_json::to_string(&result).map_err(display_to_py)
    })
}

/// Generate tornado chart entries for a sensitivity result (JSON in/out).
///
/// Parameters
/// ----------
/// result_json : str
///     JSON-serialized ``SensitivityResult``.
/// metric_node : str
///     Node to extract tornado entries for.
/// period : str | None
///     Optional period string to pin the tornado to.
///
/// Returns
/// -------
/// str
///     JSON-serialized list of ``TornadoEntry``.
#[pyfunction]
#[pyo3(signature = (result_json, metric_node, period=None))]
fn generate_tornado_entries(
    result_json: &str,
    metric_node: &str,
    period: Option<&str>,
) -> PyResult<String> {
    let result: finstack_statements_analytics::analysis::SensitivityResult =
        serde_json::from_str(result_json).map_err(display_to_py)?;
    let period_id: Option<finstack_core::dates::PeriodId> = period
        .map(|p| p.parse().map_err(display_to_py))
        .transpose()?;
    let entries = finstack_statements_analytics::analysis::generate_tornado_entries(
        &result,
        metric_node,
        period_id,
    );
    serde_json::to_string(&entries).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Variance analysis
// ---------------------------------------------------------------------------

/// Run variance analysis comparing two statement results.
///
/// Parameters
/// ----------
/// base : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// comparison : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// config_json : str
///     JSON-serialized ``VarianceConfig``.
#[pyfunction]
fn run_variance(
    py: Python<'_>,
    base: &Bound<'_, PyAny>,
    comparison: &Bound<'_, PyAny>,
    config_json: &str,
) -> PyResult<String> {
    let base = extract_results_ref(base)?.into_owned();
    let comparison = extract_results_ref(comparison)?.into_owned();
    let config: finstack_statements_analytics::analysis::VarianceConfig =
        serde_json::from_str(config_json).map_err(display_to_py)?;
    py.detach(move || {
        let analyzer =
            finstack_statements_analytics::analysis::VarianceAnalyzer::new(&base, &comparison);
        let report = analyzer.compute(&config).map_err(display_to_py)?;
        serde_json::to_string(&report).map_err(display_to_py)
    })
}

// ---------------------------------------------------------------------------
// Scenario set
// ---------------------------------------------------------------------------

/// Evaluate all scenarios in a scenario set.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// scenario_set_json : str
///     JSON-serialized ``ScenarioSet``.
///
/// Returns
/// -------
/// str
///     JSON object mapping scenario name to its ``StatementResult`` JSON.
#[pyfunction]
fn evaluate_scenario_set(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    scenario_set_json: &str,
) -> PyResult<String> {
    let model = extract_model_ref(model)?.into_owned();
    let scenario_set: finstack_statements_analytics::analysis::ScenarioSet =
        serde_json::from_str(scenario_set_json).map_err(display_to_py)?;
    py.detach(move || {
        let results = scenario_set.evaluate_all(&model).map_err(display_to_py)?;
        let map: indexmap::IndexMap<&String, &finstack_statements::evaluator::StatementResult> =
            results.scenarios.iter().collect();
        serde_json::to_string(&map).map_err(display_to_py)
    })
}

// ---------------------------------------------------------------------------
// Monte Carlo
// ---------------------------------------------------------------------------

/// Run Monte Carlo simulation on a financial model (JSON in/out).
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``.
/// config_json : str
///     JSON-serialized ``MonteCarloConfig`` with ``n_paths``, ``seed``,
///     optional ``percentiles``, and optional ``include_path_data``.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``MonteCarloResults``.
#[pyfunction]
fn run_monte_carlo(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    config_json: &str,
) -> PyResult<String> {
    let model = extract_model_ref(model)?.into_owned();
    let config: finstack_statements::evaluator::MonteCarloConfig =
        serde_json::from_str(config_json).map_err(display_to_py)?;
    py.detach(move || {
        let mut evaluator = finstack_statements::evaluator::Evaluator::new();
        let results = evaluator
            .evaluate_monte_carlo(&model, &config)
            .map_err(display_to_py)?;
        serde_json::to_string(&results).map_err(display_to_py)
    })
}

// ---------------------------------------------------------------------------
// Backtesting
// ---------------------------------------------------------------------------

/// Compute forecast accuracy metrics (MAE, MAPE, RMSE).
#[pyfunction]
fn backtest_forecast<'py>(
    py: Python<'py>,
    actual: Vec<f64>,
    forecast: Vec<f64>,
) -> PyResult<Bound<'py, PyDict>> {
    let metrics = finstack_statements_analytics::analysis::backtest_forecast(&actual, &forecast)
        .map_err(display_to_py)?;
    let dict = PyDict::new(py);
    dict.set_item("mae", metrics.mae)?;
    dict.set_item("mape", metrics.mape)?;
    dict.set_item("rmse", metrics.rmse)?;
    dict.set_item("n", metrics.n)?;
    Ok(dict)
}

// ---------------------------------------------------------------------------
// Goal seek
// ---------------------------------------------------------------------------

/// Find the driver value that makes a target node reach a target value.
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``.
/// target_node : str
///     Node to optimize towards ``target_value``.
/// target_period : str
///     Period string for the target (e.g. ``"2025Q4"``).
/// target_value : float
///     Desired value for the target node.
/// driver_node : str
///     Node whose value is adjusted to reach the target.
/// driver_period : str
///     Period string for the driver.
/// update_model : bool
///     If ``True``, the solved value is written back into the model JSON.
/// bounds : tuple[float, float] | None
///     Optional search bounds (lo, hi). Bisection is used when set.
///
/// Returns
/// -------
/// tuple[float, str]
///     (solved_driver_value, updated_model_json).
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
) -> PyResult<(f64, String)> {
    let mut model = extract_model_ref(model)?.into_owned();
    let tp: finstack_core::dates::PeriodId = target_period.parse().map_err(display_to_py)?;
    let dp: finstack_core::dates::PeriodId = driver_period.parse().map_err(display_to_py)?;
    let target_node = target_node.to_owned();
    let driver_node = driver_node.to_owned();

    py.detach(move || {
        let result = finstack_statements_analytics::analysis::goal_seek(
            &mut model,
            &target_node,
            tp,
            target_value,
            &driver_node,
            dp,
            update_model,
            bounds,
        )
        .map_err(display_to_py)?;

        let updated_json = if update_model {
            serde_json::to_string(&model).map_err(display_to_py)?
        } else {
            String::new()
        };
        Ok((result, updated_json))
    })
}

// ---------------------------------------------------------------------------
// DCF Valuation
// ---------------------------------------------------------------------------

/// Evaluate DCF valuation on a financial model.
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``. Must contain a ``"currency"``
///     key in its metadata.
/// wacc : float
///     Weighted average cost of capital in decimal form (``0.10`` = 10%).
/// terminal_value_json : str
///     JSON-serialized ``TerminalValueSpec`` (tagged enum, e.g.
///     ``{"type": "gordon_growth", "growth_rate": 0.02}``).
/// ufcf_node : str
///     Node ID containing unlevered free cash flow.
/// net_debt_override : float | None
///     Optional flat net-debt amount.
/// mid_year_convention : bool
///     Enable mid-year discounting convention.
/// shares_outstanding : float | None
///     Basic shares outstanding for per-share equity value.
/// equity_bridge_json : str | None
///     Optional JSON ``EquityBridge`` for structured bridge.
/// valuation_discounts_json : str | None
///     Optional JSON ``ValuationDiscounts`` (DLOM, DLOC).
/// market_json : str | None
///     Optional JSON ``MarketContext`` for curve-based discounting.
///
/// Returns
/// -------
/// dict
///     Result dict with ``equity_value``, ``enterprise_value``,
///     ``net_debt``, ``terminal_value_pv``, ``equity_value_per_share``,
///     ``diluted_shares`` (all floats, in model currency).
#[pyfunction]
#[pyo3(signature = (
    model,
    wacc,
    terminal_value_json,
    ufcf_node="ufcf",
    net_debt_override=None,
    mid_year_convention=false,
    shares_outstanding=None,
    equity_bridge_json=None,
    valuation_discounts_json=None,
    market=None,
))]
#[allow(clippy::too_many_arguments)]
fn evaluate_dcf<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    wacc: f64,
    terminal_value_json: &str,
    ufcf_node: &str,
    net_debt_override: Option<f64>,
    mid_year_convention: bool,
    shares_outstanding: Option<f64>,
    equity_bridge_json: Option<&str>,
    valuation_discounts_json: Option<&str>,
    market: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    use finstack_valuations::instruments::equity::dcf_equity::TerminalValueSpec;

    let model = extract_model_ref(model)?.into_owned();
    let terminal_value: TerminalValueSpec =
        serde_json::from_str(terminal_value_json).map_err(display_to_py)?;
    let ufcf_node = ufcf_node.to_owned();

    let equity_bridge = equity_bridge_json
        .map(|j| serde_json::from_str(j).map_err(display_to_py))
        .transpose()?;
    let valuation_discounts = valuation_discounts_json
        .map(|j| serde_json::from_str(j).map_err(display_to_py))
        .transpose()?;

    let options = finstack_statements_analytics::analysis::DcfOptions {
        mid_year_convention,
        equity_bridge,
        shares_outstanding,
        valuation_discounts,
        ..Default::default()
    };

    let market = extract_market_opt(market)?;

    let result = py
        .detach(move || {
            finstack_statements_analytics::analysis::evaluate_dcf_with_market(
                &model,
                wacc,
                terminal_value,
                &ufcf_node,
                net_debt_override,
                &options,
                market.as_ref(),
            )
        })
        .map_err(display_to_py)?;

    dcf_equity_result_dict(py, &result)
}

// ---------------------------------------------------------------------------
// Corporate analysis (orchestrator)
// ---------------------------------------------------------------------------

/// Run the full corporate analysis pipeline.
///
/// This uses ``CorporateAnalysisBuilder`` under the hood to evaluate
/// statements and optionally run DCF equity valuation plus credit context.
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``.
/// wacc : float | None
///     If set, enables DCF valuation at this discount rate (decimal).
/// terminal_value_json : str | None
///     JSON ``TerminalValueSpec`` (required when ``wacc`` is set).
/// net_debt_override : float | None
///     Optional flat net-debt for equity bridge.
/// coverage_node : str
///     Node used for DSCR/interest-coverage (default: ``"ebitda"``).
/// market_json : str | None
///     Optional JSON ``MarketContext``.
/// as_of : str | None
///     Optional ISO 8601 date string for valuation date.
///
/// Returns
/// -------
/// dict
///     Dict with ``statement_json`` (str), optional ``equity`` (dict of
///     scalar values), and ``credit`` (dict mapping instrument_id to
///     credit metrics JSON).
#[pyfunction]
#[pyo3(signature = (
    model,
    wacc=None,
    terminal_value_json=None,
    net_debt_override=None,
    coverage_node="ebitda",
    market=None,
    as_of=None,
))]
#[allow(clippy::too_many_arguments)]
fn run_corporate_analysis<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    wacc: Option<f64>,
    terminal_value_json: Option<&str>,
    net_debt_override: Option<f64>,
    coverage_node: &str,
    market: Option<&Bound<'py, PyAny>>,
    as_of: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    use finstack_valuations::instruments::equity::dcf_equity::TerminalValueSpec;

    let model = extract_model_ref(model)?.into_owned();
    let coverage_node = coverage_node.to_owned();

    let mut builder = finstack_statements_analytics::analysis::CorporateAnalysisBuilder::new(model)
        .coverage_node(&coverage_node);

    if let Some(w) = wacc {
        let tv_json = terminal_value_json.ok_or_else(|| {
            crate::errors::value_error("terminal_value_json required when wacc is set")
        })?;
        let tv: TerminalValueSpec = serde_json::from_str(tv_json).map_err(display_to_py)?;
        builder = builder.dcf(w, tv);
        if let Some(nd) = net_debt_override {
            builder = builder.net_debt_override(nd);
        }
    }

    if let Some(mkt) = extract_market_opt(market)? {
        builder = builder.market(mkt);
    }

    if let Some(date_str) = as_of {
        let format = time::format_description::well_known::Iso8601::DEFAULT;
        let date = time::Date::parse(date_str, &format).map_err(display_to_py)?;
        builder = builder.as_of(date);
    }

    let analysis = py
        .detach(move || builder.analyze())
        .map_err(display_to_py)?;

    let dict = PyDict::new(py);

    let stmt_json = serde_json::to_string(&analysis.statement).map_err(display_to_py)?;
    dict.set_item("statement_json", stmt_json)?;

    if let Some(ref equity) = analysis.equity {
        dict.set_item("equity", dcf_equity_result_dict(py, equity)?)?;
    }

    let credit_dict = PyDict::new(py);
    for (inst_id, credit) in &analysis.credit {
        let cred_json = serde_json::to_string(&credit).map_err(display_to_py)?;
        credit_dict.set_item(inst_id.as_str(), cred_json)?;
    }
    dict.set_item("credit", credit_dict)?;

    Ok(dict)
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

/// Generate a P&L summary report as formatted text.
///
/// Parameters
/// ----------
/// results_json : str
///     JSON-serialized ``StatementResult``.
/// line_items : list[str]
///     Node IDs to include as rows in the report.
/// periods : list[str]
///     Period strings for columns (e.g. ``["2025Q1", "2025Q2"]``).
///
/// Returns
/// -------
/// str
///     Formatted P&L summary report text.
#[pyfunction]
fn pl_summary_report(
    results: &Bound<'_, PyAny>,
    line_items: Vec<String>,
    periods: Vec<String>,
) -> PyResult<String> {
    use finstack_statements_analytics::analysis::Report;

    let results = extract_results_ref(results)?;
    let period_ids: Vec<finstack_core::dates::PeriodId> = periods
        .iter()
        .map(|p| p.parse().map_err(display_to_py))
        .collect::<PyResult<Vec<_>>>()?;
    let report = finstack_statements_analytics::analysis::PLSummaryReport::new(
        &results, line_items, period_ids,
    );
    Ok(report.to_string())
}

/// Generate a credit assessment report as formatted text.
///
/// Parameters
/// ----------
/// results_json : str
///     JSON-serialized ``StatementResult``.
/// as_of : str
///     Period string for the assessment date (e.g. ``"2025Q1"``).
///
/// Returns
/// -------
/// str
///     Formatted credit assessment report text.
#[pyfunction]
fn credit_assessment_report(results: &Bound<'_, PyAny>, as_of: &str) -> PyResult<String> {
    use finstack_statements_analytics::analysis::Report;

    let results = extract_results_ref(results)?;
    let period: finstack_core::dates::PeriodId = as_of.parse().map_err(display_to_py)?;
    let report =
        finstack_statements_analytics::analysis::CreditAssessmentReport::new(&results, period);
    Ok(report.to_string())
}

// ---------------------------------------------------------------------------
// Introspection — DependencyTracer (class)
// ---------------------------------------------------------------------------

/// Cached dependency tracer that builds the model graph once.
///
/// Construct from a ``FinancialModelSpec`` (or JSON string) and reuse for
/// multiple introspection queries without rebuilding the dependency graph.
///
/// Examples
/// --------
/// ::
///
///     tracer = DependencyTracer(model)
///     tree = tracer.dependency_tree("gross_profit")
///     deps = tracer.direct_dependencies("gross_profit")
///     all_ = tracer.all_dependencies("gross_profit")
#[pyclass(
    name = "DependencyTracer",
    module = "finstack.statements_analytics",
    skip_from_py_object
)]
struct PyDependencyTracer {
    model: finstack_statements::FinancialModelSpec,
    graph: finstack_statements::evaluator::DependencyGraph,
}

#[pymethods]
impl PyDependencyTracer {
    /// Build a tracer from a model (typed object or JSON string).
    #[new]
    fn new(model: &Bound<'_, PyAny>) -> PyResult<Self> {
        let model = extract_model_ref(model)?.into_owned();
        let graph = finstack_statements::evaluator::DependencyGraph::from_model(&model)
            .map_err(display_to_py)?;
        Ok(Self { model, graph })
    }

    /// ASCII-formatted dependency tree for a node.
    fn dependency_tree(&self, node_id: &str) -> PyResult<String> {
        let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let tree = tracer.dependency_tree(node_id).map_err(display_to_py)?;
        Ok(finstack_statements_analytics::analysis::render_tree_ascii(
            &tree,
        ))
    }

    /// ASCII tree with node values for a given period.
    fn dependency_tree_detailed(
        &self,
        results: &Bound<'_, PyAny>,
        node_id: &str,
        period: &str,
    ) -> PyResult<String> {
        let results = extract_results_ref(results)?;
        let pid: finstack_core::dates::PeriodId = period.parse().map_err(display_to_py)?;
        let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let tree = tracer.dependency_tree(node_id).map_err(display_to_py)?;
        Ok(finstack_statements_analytics::analysis::render_tree_detailed(&tree, &results, &pid))
    }

    /// Direct dependency node IDs.
    fn direct_dependencies(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let deps = tracer.direct_dependencies(node_id).map_err(display_to_py)?;
        Ok(deps.into_iter().map(String::from).collect())
    }

    /// All transitive dependency node IDs in dependency order.
    fn all_dependencies(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        tracer.all_dependencies(node_id).map_err(display_to_py)
    }

    /// Node IDs that depend on this node.
    fn dependents(&self, node_id: &str) -> PyResult<Vec<String>> {
        let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(
            &self.model,
            &self.graph,
        );
        let deps = tracer.dependents(node_id).map_err(display_to_py)?;
        Ok(deps.into_iter().map(String::from).collect())
    }

    fn __repr__(&self) -> String {
        format!("DependencyTracer(nodes={})", self.model.nodes.len())
    }
}

/// Get direct dependencies for a node.
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``.
/// node_id : str
///     Node whose direct dependencies to list.
///
/// Returns
/// -------
/// list[str]
///     Direct dependency node IDs.
#[pyfunction]
fn direct_dependencies(model: &Bound<'_, PyAny>, node_id: &str) -> PyResult<Vec<String>> {
    let model = extract_model_ref(model)?;
    let graph = finstack_statements::evaluator::DependencyGraph::from_model(&model)
        .map_err(display_to_py)?;
    let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(&model, &graph);
    let deps = tracer.direct_dependencies(node_id).map_err(display_to_py)?;
    Ok(deps.into_iter().map(String::from).collect())
}

/// Get all transitive dependencies for a node.
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``.
/// node_id : str
///     Node whose transitive dependencies to list.
///
/// Returns
/// -------
/// list[str]
///     All transitive dependency node IDs in dependency order.
#[pyfunction]
fn all_dependencies(model: &Bound<'_, PyAny>, node_id: &str) -> PyResult<Vec<String>> {
    let model = extract_model_ref(model)?;
    let graph = finstack_statements::evaluator::DependencyGraph::from_model(&model)
        .map_err(display_to_py)?;
    let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(&model, &graph);
    tracer.all_dependencies(node_id).map_err(display_to_py)
}

/// Get nodes that depend on this node (reverse dependencies).
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``.
/// node_id : str
///     Node whose dependents to list.
///
/// Returns
/// -------
/// list[str]
///     Node IDs that depend on this node.
#[pyfunction]
fn dependents(model: &Bound<'_, PyAny>, node_id: &str) -> PyResult<Vec<String>> {
    let model = extract_model_ref(model)?;
    let graph = finstack_statements::evaluator::DependencyGraph::from_model(&model)
        .map_err(display_to_py)?;
    let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(&model, &graph);
    let deps = tracer.dependents(node_id).map_err(display_to_py)?;
    Ok(deps.into_iter().map(String::from).collect())
}

// ---------------------------------------------------------------------------
// Introspection — FormulaExplainer
// ---------------------------------------------------------------------------

/// Explain a formula for a specific node and period.
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``.
/// results_json : str
///     JSON-serialized ``StatementResult``.
/// node_id : str
///     Node whose formula to explain.
/// period : str
///     Period string.
///
/// Returns
/// -------
/// dict
///     Explanation dict with ``node_id``, ``period_id``, ``final_value``,
///     ``node_type``, ``formula_text``, and ``breakdown`` (list of component dicts).
#[pyfunction]
fn explain_formula<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    results: &Bound<'py, PyAny>,
    node_id: &str,
    period: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let model = extract_model_ref(model)?;
    let results = extract_results_ref(results)?;
    let pid: finstack_core::dates::PeriodId = period.parse().map_err(display_to_py)?;

    let explainer =
        finstack_statements_analytics::analysis::FormulaExplainer::new(&model, &results);
    let explanation = explainer.explain(node_id, &pid).map_err(display_to_py)?;

    let dict = PyDict::new(py);
    dict.set_item("node_id", &explanation.node_id)?;
    dict.set_item("period_id", explanation.period_id.to_string())?;
    dict.set_item("final_value", explanation.final_value)?;
    dict.set_item("node_type", format!("{:?}", explanation.node_type))?;
    dict.set_item("formula_text", &explanation.formula_text)?;

    let steps: Vec<Bound<'py, PyDict>> = explanation
        .breakdown
        .iter()
        .map(|step| {
            let d = PyDict::new(py);
            d.set_item("component", &step.component)?;
            d.set_item("value", step.value)?;
            d.set_item("operation", &step.operation)?;
            Ok(d)
        })
        .collect::<PyResult<Vec<_>>>()?;
    dict.set_item("breakdown", PyList::new(py, steps)?)?;

    Ok(dict)
}

/// Get a detailed text explanation for a formula.
///
/// Parameters
/// ----------
/// model_json : str
///     JSON-serialized ``FinancialModelSpec``.
/// results_json : str
///     JSON-serialized ``StatementResult``.
/// node_id : str
///     Node whose formula to explain.
/// period : str
///     Period string.
///
/// Returns
/// -------
/// str
///     Human-readable multi-line explanation.
#[pyfunction]
fn explain_formula_text(
    model: &Bound<'_, PyAny>,
    results: &Bound<'_, PyAny>,
    node_id: &str,
    period: &str,
) -> PyResult<String> {
    let model = extract_model_ref(model)?;
    let results = extract_results_ref(results)?;
    let pid: finstack_core::dates::PeriodId = period.parse().map_err(display_to_py)?;

    let explainer =
        finstack_statements_analytics::analysis::FormulaExplainer::new(&model, &results);
    let explanation = explainer.explain(node_id, &pid).map_err(display_to_py)?;
    Ok(explanation.to_string_detailed())
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// Run checks from a suite spec against a model (JSON in/out).
///
/// Resolves both built-in and formula checks from the spec, evaluates the
/// model, and returns a full check report.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///   A ``FinancialModelSpec`` object or a JSON string.
/// suite_spec_json : str
///   JSON-serialized ``CheckSuiteSpec``.
/// results : StatementResult | str | None
///   Pre-computed evaluation results.  When provided the model is not
///   re-evaluated, avoiding redundant work.
///
/// Returns
/// -------
/// str
///   JSON-serialized ``CheckReport``.
#[pyfunction]
#[pyo3(signature = (model, suite_spec_json, results=None))]
fn run_checks(
    model: &Bound<'_, PyAny>,
    suite_spec_json: &str,
    results: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let model = extract_model_ref(model)?;
    let spec: finstack_statements::checks::CheckSuiteSpec =
        serde_json::from_str(suite_spec_json).map_err(display_to_py)?;

    let mut suite = spec.resolve().map_err(display_to_py)?;

    if !spec.formula_checks.is_empty() {
        let mut fc_builder = finstack_statements::checks::CheckSuite::builder("_formula_checks");
        for fc_spec in &spec.formula_checks {
            fc_builder =
                fc_builder.add_check(finstack_statements_analytics::analysis::FormulaCheck {
                    id: fc_spec.id.clone(),
                    name: fc_spec.name.clone(),
                    category: fc_spec.category,
                    severity: fc_spec.severity,
                    formula: fc_spec.formula.clone(),
                    message_template: fc_spec.message_template.clone(),
                    tolerance: fc_spec.tolerance,
                });
        }
        suite = suite.merge(fc_builder.build());
    }

    let eval_result;
    let results_ref: &finstack_statements::evaluator::StatementResult = match results {
        Some(r) => {
            eval_result = extract_results_ref(r)?;
            &eval_result
        }
        None => {
            let mut evaluator = finstack_statements::evaluator::Evaluator::new();
            eval_result = crate::bindings::extract::ResultAccess::Owned(Box::new(
                evaluator.evaluate(&model).map_err(display_to_py)?,
            ));
            &eval_result
        }
    };
    let report = suite.run(&model, results_ref).map_err(display_to_py)?;
    serde_json::to_string(&report).map_err(display_to_py)
}

/// Run three-statement checks using a node mapping (JSON in/out).
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///   A ``FinancialModelSpec`` object or a JSON string.
/// mapping_json : str
///   JSON-serialized ``ThreeStatementMapping``.
/// results : StatementResult | str | None
///   Pre-computed evaluation results.  Skips re-evaluation when provided.
///
/// Returns
/// -------
/// str
///   JSON-serialized ``CheckReport``.
#[pyfunction]
#[pyo3(signature = (model, mapping_json, results=None))]
fn run_three_statement_checks(
    model: &Bound<'_, PyAny>,
    mapping_json: &str,
    results: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let model = extract_model_ref(model)?;
    let mapping: finstack_statements_analytics::analysis::ThreeStatementMapping =
        serde_json::from_str(mapping_json).map_err(display_to_py)?;
    let suite = finstack_statements_analytics::analysis::three_statement_checks(mapping);

    let eval_result;
    let results_ref: &finstack_statements::evaluator::StatementResult = match results {
        Some(r) => {
            eval_result = extract_results_ref(r)?;
            &eval_result
        }
        None => {
            let mut evaluator = finstack_statements::evaluator::Evaluator::new();
            eval_result = crate::bindings::extract::ResultAccess::Owned(Box::new(
                evaluator.evaluate(&model).map_err(display_to_py)?,
            ));
            &eval_result
        }
    };
    let report = suite.run(&model, results_ref).map_err(display_to_py)?;
    serde_json::to_string(&report).map_err(display_to_py)
}

/// Run credit underwriting checks using a node mapping (JSON in/out).
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///   A ``FinancialModelSpec`` object or a JSON string.
/// mapping_json : str
///   JSON-serialized ``CreditMapping``.
/// results : StatementResult | str | None
///   Pre-computed evaluation results.  Skips re-evaluation when provided.
///
/// Returns
/// -------
/// str
///   JSON-serialized ``CheckReport``.
#[pyfunction]
#[pyo3(signature = (model, mapping_json, results=None))]
fn run_credit_underwriting_checks(
    model: &Bound<'_, PyAny>,
    mapping_json: &str,
    results: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let model = extract_model_ref(model)?;
    let mapping: finstack_statements_analytics::analysis::CreditMapping =
        serde_json::from_str(mapping_json).map_err(display_to_py)?;
    let suite = finstack_statements_analytics::analysis::credit_underwriting_checks(mapping);

    let eval_result;
    let results_ref: &finstack_statements::evaluator::StatementResult = match results {
        Some(r) => {
            eval_result = extract_results_ref(r)?;
            &eval_result
        }
        None => {
            let mut evaluator = finstack_statements::evaluator::Evaluator::new();
            eval_result = crate::bindings::extract::ResultAccess::Owned(Box::new(
                evaluator.evaluate(&model).map_err(display_to_py)?,
            ));
            &eval_result
        }
    };
    let report = suite.run(&model, results_ref).map_err(display_to_py)?;
    serde_json::to_string(&report).map_err(display_to_py)
}

/// Render a check report as plain text.
///
/// Parameters
/// ----------
/// report_json : str
///   JSON-serialized ``CheckReport``.
///
/// Returns
/// -------
/// str
///   Human-readable plain-text report.
#[pyfunction]
fn render_check_report_text(report_json: &str) -> PyResult<String> {
    let report: finstack_statements::checks::CheckReport =
        serde_json::from_str(report_json).map_err(display_to_py)?;
    Ok(finstack_statements_analytics::analysis::CheckReportRenderer::render_text(&report))
}

/// Render a check report as HTML with inline styles.
///
/// Parameters
/// ----------
/// report_json : str
///   JSON-serialized ``CheckReport``.
///
/// Returns
/// -------
/// str
///   HTML-formatted report suitable for Jupyter notebooks.
#[pyfunction]
fn render_check_report_html(report_json: &str) -> PyResult<String> {
    let report: finstack_statements::checks::CheckReport =
        serde_json::from_str(report_json).map_err(display_to_py)?;
    Ok(finstack_statements_analytics::analysis::CheckReportRenderer::render_html(&report))
}

/// Register analysis functions and classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDependencyTracer>()?;
    m.add_function(pyo3::wrap_pyfunction!(run_sensitivity, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(generate_tornado_entries, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_variance, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_scenario_set, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_monte_carlo, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(backtest_forecast, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(goal_seek, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_dcf, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_corporate_analysis, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(pl_summary_report, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(credit_assessment_report, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(direct_dependencies, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(all_dependencies, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dependents, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(explain_formula, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(explain_formula_text, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_checks, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_three_statement_checks, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_credit_underwriting_checks, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(render_check_report_text, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(render_check_report_html, m)?)?;
    Ok(())
}
