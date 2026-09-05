//! WASM bindings for the `finstack-quant-statements-analytics` crate.
//!
//! Exposes financial statement analysis functions that accept and return
//! JSON strings, suitable for consumption from JavaScript/TypeScript.

mod comps;

pub use comps::{
    compute_multiple, peer_stats, percentile_rank, regression_fair_value, score_relative_value,
    z_score,
};

use crate::api::statements::parse_validated_model;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Run a sensitivity analysis on a financial model.
///
/// Accepts JSON strings for the model spec and sensitivity configuration,
/// evaluates all perturbation scenarios, and returns the `SensitivityResult`
/// as a structured JavaScript object. `generateTornadoEntries` still takes the
/// result as JSON, so pass `JSON.stringify(result)` when chaining the two.
///
/// # Errors
///
/// Rejects malformed model or configuration JSON, invalid sensitivity modes or
/// parameter perturbations, missing model nodes or periods, model-evaluation
/// failures, or failure to serialize the sensitivity result to JavaScript.
/// @param model_json - Financial-model specification JSON.
/// @param config_json - Configuration JSON for this call.
#[wasm_bindgen(js_name = runSensitivity)]
pub fn run_sensitivity(model_json: &str, config_json: &str) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;

    let config: finstack_quant_statements_analytics::analysis::SensitivityConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;

    let analyzer = finstack_quant_statements_analytics::analysis::SensitivityAnalyzer::new(&model);
    let result = analyzer.run(&config).map_err(to_js_err)?;

    to_js_value(&result)
}

/// Run a variance analysis comparing two evaluated statement results.
///
/// Returns the variance report as a structured JavaScript object.
///
/// # Errors
///
/// Rejects malformed result or configuration JSON, empty metric or period
/// selections, a requested value missing from either result, or failure to
/// serialize the variance report to JavaScript.
/// @param base_json - Base statement-result JSON.
/// @param comparison_json - Comparison statement-result JSON.
/// @param config_json - Configuration JSON for this call.
#[wasm_bindgen(js_name = runVariance)]
pub fn run_variance(
    base_json: &str,
    comparison_json: &str,
    config_json: &str,
) -> Result<JsValue, JsValue> {
    let base: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(base_json).map_err(to_js_err)?;

    let comparison: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(comparison_json).map_err(to_js_err)?;

    let config: finstack_quant_statements_analytics::analysis::VarianceConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;

    let analyzer =
        finstack_quant_statements_analytics::analysis::VarianceAnalyzer::new(&base, &comparison);
    let report = analyzer.compute(&config).map_err(to_js_err)?;

    to_js_value(&report)
}

/// Evaluate all scenarios in a scenario set against a base model.
///
/// Returns a structured JavaScript object mapping scenario names to their
/// statement results.
///
/// # Errors
///
/// Rejects malformed model or scenario-set JSON, an empty scenario set,
/// invalid parent chains, overrides of missing nodes, failure to evaluate any
/// scenario, or failure to serialize the result map to JavaScript.
/// @param model_json - Financial-model specification JSON.
/// @param scenario_set_json - Scenario-set JSON keyed by scenario name.
#[wasm_bindgen(js_name = evaluateScenarioSet)]
pub fn evaluate_scenario_set(
    model_json: &str,
    scenario_set_json: &str,
) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;

    let scenario_set: finstack_quant_statements_analytics::analysis::ScenarioSet =
        serde_json::from_str(scenario_set_json).map_err(to_js_err)?;

    let results = scenario_set.evaluate_all(&model).map_err(to_js_err)?;

    let map: indexmap::IndexMap<&String, &finstack_quant_statements::evaluator::StatementResult> =
        results.scenarios.iter().collect();
    to_js_value(&map)
}

/// Compute forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).
///
/// Takes two float arrays (actual, forecast) and returns the serde form of
/// the Rust `ForecastMetrics` (`mae`, `mape`, `mape_effective_n`, `smape`,
/// `rmse`, `n`).
///
/// # Errors
///
/// Rejects inputs that cannot be decoded as numeric JavaScript arrays, arrays
/// with unequal lengths, empty arrays, or metrics that cannot be serialized to
/// JavaScript.
/// @param actual - Actual realized values aligned one-for-one with the forecast series.
/// @param forecast - Forecast values aligned one-for-one with the actual realized series.
#[wasm_bindgen(js_name = backtestForecast)]
pub fn backtest_forecast(actual: JsValue, forecast: JsValue) -> Result<JsValue, JsValue> {
    let actual_vec: Vec<f64> = serde_wasm_bindgen::from_value(actual).map_err(to_js_err)?;
    let forecast_vec: Vec<f64> = serde_wasm_bindgen::from_value(forecast).map_err(to_js_err)?;

    let metrics = finstack_quant_statements_analytics::analysis::backtest_forecast(
        &actual_vec,
        &forecast_vec,
    )
    .map_err(to_js_err)?;
    to_js_value(&metrics)
}

/// Generate tornado chart entries for a sensitivity result.
///
/// # Errors
///
/// Rejects malformed `result_json`, an invalid optional `period` identifier, or
/// failure to convert the entries to JavaScript. A missing metric produces no
/// entry rather than rejecting.
/// @returns Structured tornado entries sorted by descending absolute swing.
/// @param result_json - Result JSON produced by a prior call.
/// @param metric_node - Statement metric node identifier selected for the requested analysis.
/// @param period - Model period label for the requested statement value or calculation.
#[wasm_bindgen(js_name = generateTornadoEntries)]
pub fn generate_tornado_entries(
    result_json: &str,
    metric_node: &str,
    period: Option<String>,
) -> Result<JsValue, JsValue> {
    let result: finstack_quant_statements_analytics::analysis::SensitivityResult =
        serde_json::from_str(result_json).map_err(to_js_err)?;
    let period_id: Option<finstack_quant_core::dates::PeriodId> =
        period.map(|p| p.parse().map_err(to_js_err)).transpose()?;
    let entries = finstack_quant_statements_analytics::analysis::generate_tornado_entries(
        &result,
        metric_node,
        period_id,
    );
    to_js_value(&entries)
}

/// Rank the headline DCF assumptions by enterprise-value impact.
///
/// The statement model is evaluated once; each shocked point re-runs only the
/// DCF. Returns JSON with the baseline enterprise value, tornado entries as
/// deltas versus that baseline sorted by descending absolute swing, and the
/// effective (possibly clamped) shock levels, as a structured JavaScript
/// object.
///
/// # Errors
///
/// Rejects malformed model or terminal-value JSON, model-evaluation failures,
/// a missing UFCF series or model currency, inconsistent WACC or terminal-value
/// assumptions, missing bridge inputs, valuation failures, or failure to
/// serialize the sensitivity result.
/// @param model_json - Financial-model specification JSON.
/// @param wacc - Baseline weighted average cost of capital in decimal form (0.10 = 10%).
/// @param terminal_value_json - Terminal-value spec JSON selecting whether growth or the exit multiple is shocked.
/// @param ufcf_node - Node identifier holding unlevered free cash flow for the forecast periods.
/// @param net_debt_override - Optional flat net-debt amount used instead of the model-derived bridge.
/// @param wacc_sensitivity_bump - Absolute shock applied to WACC and to the terminal growth rate, in decimal (0.01 = +/-100 bp).
/// @param wacc_denominator_epsilon - Minimum spread preserved between WACC and the terminal growth rate so 1/(wacc - g) stays defined, in decimal.
/// @param max_stable_growth_rate - Maximum perpetual stable growth rate; omitted uses the canonical 5% default.
/// @param exit_multiple_bump - Absolute shock applied to an exit multiple, in turns of the multiple (1.0 = +/-1.0x).
/// @param mid_year_convention - Whether every DCF re-run uses the mid-year discounting convention.
/// @param market_json - Optional canonical market-context JSON used for statement evaluation, not WACC discounting.
#[wasm_bindgen(js_name = dcfSensitivity)]
#[allow(clippy::too_many_arguments)]
pub fn dcf_sensitivity(
    model_json: &str,
    wacc: f64,
    terminal_value_json: &str,
    ufcf_node: &str,
    net_debt_override: Option<f64>,
    wacc_sensitivity_bump: Option<f64>,
    wacc_denominator_epsilon: Option<f64>,
    max_stable_growth_rate: Option<f64>,
    exit_multiple_bump: Option<f64>,
    mid_year_convention: Option<bool>,
    market_json: Option<String>,
) -> Result<JsValue, JsValue> {
    use finstack_quant_statements_analytics::analysis::{DcfOptions, ExitMultipleBump};

    let model = parse_validated_model(model_json)?;
    let terminal_value: finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec =
        serde_json::from_str(terminal_value_json).map_err(to_js_err)?;
    let market: Option<finstack_quant_core::market_data::context::MarketContext> = market_json
        .map(|json| serde_json::from_str(&json).map_err(to_js_err))
        .transpose()?;

    let defaults = DcfOptions::default();
    let options = DcfOptions {
        mid_year_convention: mid_year_convention.unwrap_or(defaults.mid_year_convention),
        wacc_sensitivity_bump: wacc_sensitivity_bump.unwrap_or(defaults.wacc_sensitivity_bump),
        wacc_denominator_epsilon: wacc_denominator_epsilon
            .unwrap_or(defaults.wacc_denominator_epsilon),
        max_stable_growth_rate: max_stable_growth_rate.unwrap_or(defaults.max_stable_growth_rate),
        exit_multiple_bump: exit_multiple_bump
            .map_or(defaults.exit_multiple_bump, ExitMultipleBump::Absolute),
        ..DcfOptions::default()
    };

    let result = finstack_quant_statements_analytics::analysis::dcf_sensitivity(
        &model,
        wacc,
        terminal_value,
        ufcf_node,
        net_debt_override,
        &options,
        market.as_ref(),
    )
    .map_err(to_js_err)?;

    // Serde form of the Rust `DcfSensitivityResult`; `baseline_enterprise_value`
    // is a `Money` wire object (`{amount, currency}`).
    to_js_value(&result)
}

/// Evaluate a leveraged-buyout transaction against a statement model.
///
/// Entry enterprise value is priced at the model's first period, the sponsor
/// equity check is solved as the sources-and-uses residual, and exit proceeds
/// are the exit enterprise value less the modelled net debt at the exit
/// period. IRR is out of scope: pair the returned `exit_equity_proceeds` with
/// the equity outflow at close and call `portfolio.mwrXirr`.
///
/// # Errors
///
/// Rejects malformed model or tranche JSON, an invalid `exit_period`, model
/// evaluation or lookup failures, a missing model currency or period,
/// non-finite transaction inputs or model values, negative tranche amounts, a
/// non-positive sponsor equity check, check-suite failures, or failure to
/// serialize the result to JavaScript. The result is a structured JavaScript
/// object.
/// @param model_json - Financial-model specification JSON.
/// @param entry_multiple - Entry valuation multiple applied to the entry metric (8.5 = 8.5x).
/// @param entry_metric_node - Node identifier supplying the entry valuation metric, read at the model's first period.
/// @param exit_multiple - Exit valuation multiple applied to the exit metric (9.5 = 9.5x).
/// @param exit_metric_node - Node identifier supplying the exit valuation metric, read at the exit period.
/// @param exit_net_debt_node - Node identifier supplying net debt outstanding at the exit period, where a modelled amortisation schedule lands.
/// @param exit_period - Model period label at which the sponsor exits, e.g. "2029".
/// @param sources_json - Canonical JSON array of funded debt tranches at close, each {"name", "amount"} in the model currency.
/// @param transaction_fees - Transaction fees and expenses funded at close, in the model currency.
#[wasm_bindgen(js_name = evaluateLbo)]
#[allow(clippy::too_many_arguments)]
pub fn evaluate_lbo(
    model_json: &str,
    entry_multiple: f64,
    entry_metric_node: &str,
    exit_multiple: f64,
    exit_metric_node: &str,
    exit_net_debt_node: &str,
    exit_period: &str,
    sources_json: &str,
    transaction_fees: f64,
) -> Result<JsValue, JsValue> {
    use finstack_quant_statements_analytics::analysis::{LboConfig, LboTranche};

    let model = parse_validated_model(model_json)?;
    let sources: Vec<LboTranche> = serde_json::from_str(sources_json).map_err(to_js_err)?;
    let exit_period: finstack_quant_core::dates::PeriodId =
        exit_period.parse().map_err(to_js_err)?;

    let config = LboConfig {
        entry_multiple,
        entry_metric_node: entry_metric_node.to_owned(),
        transaction_fees,
        sources,
        exit_multiple,
        exit_metric_node: exit_metric_node.to_owned(),
        exit_net_debt_node: exit_net_debt_node.to_owned(),
        exit_period,
        check_mappings: None,
    };

    let result = finstack_quant_statements_analytics::analysis::evaluate_lbo(&model, &config)
        .map_err(to_js_err)?;

    // Serde form of the Rust `LboResult`; monetary fields are `Money` wire
    // objects (`{amount, currency}`).
    to_js_value(&result)
}

/// Weighted-average cost of capital (WACC).
///
/// Blends the required return on equity with the after-tax cost of debt:
/// `WACC = w_E * r_E + w_D * r_D * (1 - T)`.
///
/// # Errors
///
/// Rejects any non-finite input, negative capital weights, weights that do not
/// sum to one within tolerance, or a `tax_rate` outside `[0, 1]`.
/// @param equity_weight - Equity share of total capital as a decimal fraction (0.6 = 60% equity-funded).
/// @param cost_of_equity - Required return on equity in decimal form, typically from CAPM (0.115 = 11.5%).
/// @param debt_weight - Debt share of total capital as a decimal fraction; must sum with the equity weight to 1.0.
/// @param cost_of_debt - Pre-tax marginal borrowing yield in decimal form, before the interest tax shield (0.06 = 6%).
/// @param tax_rate - Marginal corporate tax rate as a decimal fraction in [0, 1] (0.25 = 25%).
#[wasm_bindgen(js_name = wacc)]
pub fn wacc(
    equity_weight: f64,
    cost_of_equity: f64,
    debt_weight: f64,
    cost_of_debt: f64,
    tax_rate: f64,
) -> Result<f64, JsValue> {
    finstack_quant_statements_analytics::analysis::wacc(
        equity_weight,
        cost_of_equity,
        debt_weight,
        cost_of_debt,
        tax_rate,
    )
    .map_err(to_js_err)
}

/// Find the driver value that makes a target node reach a target value.
///
/// # Errors
///
/// Rejects malformed `model_json`, invalid target or driver period identifiers,
/// exactly one supplied bound, missing target or driver nodes or periods,
/// non-finite or unordered bounds, model-evaluation or solver-convergence
/// failures, or failure to serialize the result or updated model.
/// @param model_json - Financial-model specification JSON.
/// @param target_node - Statement node identifier whose value is driven toward the target.
/// @param target_period - Model period label in which the goal-seek target is evaluated.
/// @param target_value - Numeric target value the goal-seek routine attempts to reach.
/// @param driver_node - Statement node identifier adjusted by the goal-seek routine.
/// @param driver_period - Model period label of the adjustable goal-seek driver.
/// @param update_model - Whether to return the model with the solved driver value applied.
/// @param bounds_lo - Lower numeric bound allowed for the goal-seek driver.
/// @param bounds_hi - Upper numeric bound allowed for the goal-seek driver.
#[wasm_bindgen(js_name = goalSeek)]
#[allow(clippy::too_many_arguments)]
pub fn goal_seek(
    model_json: &str,
    target_node: &str,
    target_period: &str,
    target_value: f64,
    driver_node: &str,
    driver_period: &str,
    update_model: bool,
    bounds_lo: Option<f64>,
    bounds_hi: Option<f64>,
) -> Result<JsValue, JsValue> {
    let mut model = parse_validated_model(model_json)?;
    let tp: finstack_quant_core::dates::PeriodId = target_period.parse().map_err(to_js_err)?;
    let dp: finstack_quant_core::dates::PeriodId = driver_period.parse().map_err(to_js_err)?;
    let bounds = goal_seek_bounds(bounds_lo, bounds_hi).map_err(|e| JsValue::from_str(&e))?;

    let result = finstack_quant_statements_analytics::analysis::goal_seek(
        &mut model,
        target_node,
        tp,
        target_value,
        driver_node,
        dp,
        update_model,
        bounds,
    )
    .map_err(to_js_err)?;

    // Only re-serialize the (potentially mutated) model when the caller
    // asked for the update; otherwise `model` is unchanged and the JSON is
    // wasted work + a confusing `updated_model_json` on non-updating calls.
    let out = if update_model {
        let updated_json = serde_json::to_string(&model).map_err(to_js_err)?;
        serde_json::json!({
            "solved_value": result,
            "updated_model_json": updated_json,
        })
    } else {
        serde_json::json!({ "solved_value": result })
    };
    to_js_value(&out)
}

/// Validate that goal-seek bounds are either both present or both absent.
///
/// Kept JsValue-free so the rejection logic is unit-testable on native
/// targets (constructing a `JsValue` aborts off-wasm32).
fn goal_seek_bounds(
    bounds_lo: Option<f64>,
    bounds_hi: Option<f64>,
) -> Result<Option<(f64, f64)>, String> {
    match (bounds_lo, bounds_hi) {
        (Some(lo), Some(hi)) => Ok(Some((lo, hi))),
        (None, None) => Ok(None),
        _ => Err(
            "goalSeek: bounds_lo and bounds_hi must be provided together \
             (got exactly one bound)"
                .to_string(),
        ),
    }
}

/// Trace dependencies for a node and return ASCII tree.
///
/// # Errors
///
/// Rejects malformed `model_json`, formulas or clauses whose dependencies
/// cannot be parsed, unknown formula references, a missing `node_id` or
/// reachable dependency, or a dependency cycle.
/// @param model_json - Financial-model specification JSON.
/// @param node_id - Stable node identifier used to select the required domain object.
#[wasm_bindgen(js_name = traceDependencies)]
pub fn trace_dependencies(model_json: &str, node_id: &str) -> Result<String, JsValue> {
    let model = parse_validated_model(model_json)?;
    let graph = finstack_quant_statements::evaluator::DependencyGraph::from_model(&model)
        .map_err(to_js_err)?;
    let tracer =
        finstack_quant_statements_analytics::analysis::DependencyTracer::new(&model, &graph);
    let tree = tracer.dependency_tree(node_id).map_err(to_js_err)?;
    Ok(finstack_quant_statements_analytics::analysis::render_tree_ascii(&tree))
}

/// Explain a formula for a specific node and period (JSON in/out).
///
/// # Errors
///
/// Rejects malformed model or result JSON, an invalid `period` identifier, a
/// missing model node or node-period result, an invalid formula used to build
/// the breakdown, or failure to serialize the explanation to JavaScript.
/// @param model_json - Financial-model specification JSON.
/// @param results_json - Evaluated statement-result JSON.
/// @param node_id - Stable node identifier used to select the required domain object.
/// @param period - Model period label for the requested statement value or calculation.
#[wasm_bindgen(js_name = explainFormula)]
pub fn explain_formula(
    model_json: &str,
    results_json: &str,
    node_id: &str,
    period: &str,
) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let pid: finstack_quant_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let explainer =
        finstack_quant_statements_analytics::analysis::FormulaExplainer::new(&model, &results);
    let explanation = explainer.explain(node_id, &pid).map_err(to_js_err)?;
    to_js_value(&explanation)
}

/// Explain a formula for a specific node and period as formatted text.
///
/// # Errors
///
/// Rejects malformed model or result JSON, an invalid `period` identifier, a
/// missing model node or node-period result, or an invalid formula used to
/// build the explanation breakdown.
/// @param model_json - Financial-model specification JSON.
/// @param results_json - Evaluated statement-result JSON.
/// @param node_id - Stable node identifier used to select the required domain object.
/// @param period - Model period label for the requested statement value or calculation.
#[wasm_bindgen(js_name = explainFormulaText)]
pub fn explain_formula_text(
    model_json: &str,
    results_json: &str,
    node_id: &str,
    period: &str,
) -> Result<String, JsValue> {
    let model = parse_validated_model(model_json)?;
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let pid: finstack_quant_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let explainer =
        finstack_quant_statements_analytics::analysis::FormulaExplainer::new(&model, &results);
    let explanation = explainer.explain(node_id, &pid).map_err(to_js_err)?;
    Ok(explanation.to_string_detailed())
}

/// Generate a P&L summary report as formatted text.
///
/// # Errors
///
/// Rejects malformed `results_json`, `line_items` or `periods` values that are
/// not JavaScript string arrays, or any period string that is not a valid
/// statement period identifier.
/// @param results_json - Evaluated statement-result JSON.
/// @param line_items - Ordered statement line-item definitions included in the summary report.
/// @param periods - Ordered period labels or observations aligned with the supplied data.
#[wasm_bindgen(js_name = plSummaryReportText)]
pub fn pl_summary_report_text(
    results_json: &str,
    line_items: JsValue,
    periods: JsValue,
) -> Result<String, JsValue> {
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let items: Vec<String> = serde_wasm_bindgen::from_value(line_items).map_err(to_js_err)?;
    let period_strs: Vec<String> = serde_wasm_bindgen::from_value(periods).map_err(to_js_err)?;
    let period_ids: Vec<finstack_quant_core::dates::PeriodId> = period_strs
        .iter()
        .map(|p| p.parse().map_err(to_js_err))
        .collect::<Result<Vec<_>, _>>()?;
    let report = finstack_quant_statements_analytics::analysis::PLSummaryReport::new(
        &results, items, period_ids,
    );
    Ok(report.to_string())
}

/// Generate a credit assessment report as formatted text.
///
/// # Errors
///
/// Rejects malformed `results_json` or an `period` value that is not a valid
/// statement period identifier.
/// @param results_json - Evaluated statement-result JSON.
/// @param period - Statement period identifier, such as `2025Q4` or `2025A`.
#[wasm_bindgen(js_name = creditAssessmentReportText)]
pub fn credit_assessment_report_text(results_json: &str, period: &str) -> Result<String, JsValue> {
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let period: finstack_quant_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let report = finstack_quant_statements_analytics::analysis::CreditAssessmentReport::new(
        &results, period,
    );
    Ok(report.to_string())
}

/// Compute a structured credit assessment (leverage, coverage, FCF).
///
/// Returns a structured JavaScript object.
///
/// # Errors
///
/// Rejects malformed `results_json`, an `period` value that is not a valid
/// statement period identifier, or failure to serialize the assessment to
/// JavaScript.
/// @param results_json - Evaluated statement-result JSON.
/// @param period - Statement period identifier, such as `2025Q4` or `2025A`.
#[wasm_bindgen(js_name = creditAssessment)]
pub fn credit_assessment(results_json: &str, period: &str) -> Result<JsValue, JsValue> {
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let period: finstack_quant_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let assessment =
        finstack_quant_statements_analytics::analysis::CreditAssessment::compute(&results, period);
    to_js_value(&assessment)
}

/// Run checks from a suite spec against a model.
///
/// Evaluates the model when results are absent, resolves built-in and formula
/// checks, and returns a structured check report.
///
/// # Errors
///
/// Rejects malformed model, suite, or supplied result JSON; check-suite
/// resolution failures; model-evaluation failures when results are omitted;
/// missing nodes, incompatible data, or invalid check configuration during
/// execution; or failure to convert the report to JavaScript.
/// @returns Structured check report with individual results and aggregate summary.
/// @param model_json - Financial-model specification JSON.
/// @param suite_spec_json - Check-suite specification JSON.
/// @param results_json - Evaluated statement-result JSON.
#[wasm_bindgen(js_name = runChecks)]
pub fn run_checks(
    model_json: &str,
    suite_spec_json: &str,
    results_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;
    let spec: finstack_quant_statements::checks::CheckSuiteSpec =
        serde_json::from_str(suite_spec_json).map_err(to_js_err)?;
    let suite = spec.resolve().map_err(to_js_err)?;
    let results = parse_optional_results(results_json)?;
    let report = suite
        .run_model(&model, results.as_ref())
        .map_err(to_js_err)?;
    to_js_value(&report)
}

/// Run three-statement checks using node mappings.
///
/// Accepts a model and a mapping JSON, builds the appropriate check suite, and
/// evaluates the model only when precomputed results are absent.
///
/// # Errors
///
/// Rejects malformed model, mapping, or supplied result JSON; model-evaluation
/// failures when results are omitted; missing mapped nodes, incompatible data,
/// or invalid check configuration; or failure to convert the report to JavaScript.
/// @returns Structured three-statement check report with results and aggregate summary.
/// @param model_json - Financial-model specification JSON.
/// @param mapping_json - Node-mapping JSON from statement nodes to check inputs.
/// @param results_json - Evaluated statement-result JSON.
#[wasm_bindgen(js_name = runThreeStatementChecks)]
pub fn run_three_statement_checks(
    model_json: &str,
    mapping_json: &str,
    results_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;
    let mapping: finstack_quant_statements_analytics::analysis::ThreeStatementMapping =
        serde_json::from_str(mapping_json).map_err(to_js_err)?;
    let suite = finstack_quant_statements_analytics::analysis::three_statement_checks(mapping);
    let results = parse_optional_results(results_json)?;
    let report = suite
        .run_model(&model, results.as_ref())
        .map_err(to_js_err)?;
    to_js_value(&report)
}

/// Run credit underwriting checks using credit-specific mappings.
///
/// # Errors
///
/// Rejects malformed model, mapping, or supplied result JSON; model-evaluation
/// failures when results are omitted; missing mapped nodes, incompatible data,
/// or invalid check configuration; or failure to convert the report to JavaScript.
/// @returns Structured credit-underwriting check report with results and aggregate summary.
/// @param model_json - Financial-model specification JSON.
/// @param mapping_json - Node-mapping JSON from statement nodes to check inputs.
/// @param results_json - Evaluated statement-result JSON.
#[wasm_bindgen(js_name = runCreditUnderwritingChecks)]
pub fn run_credit_underwriting_checks(
    model_json: &str,
    mapping_json: &str,
    results_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;
    let mapping: finstack_quant_statements_analytics::analysis::CreditMapping =
        serde_json::from_str(mapping_json).map_err(to_js_err)?;
    let suite = finstack_quant_statements_analytics::analysis::credit_underwriting_checks(mapping);
    let results = parse_optional_results(results_json)?;
    let report = suite
        .run_model(&model, results.as_ref())
        .map_err(to_js_err)?;
    to_js_value(&report)
}

fn parse_optional_results(
    results_json: Option<String>,
) -> Result<Option<finstack_quant_statements::evaluator::StatementResult>, JsValue> {
    results_json
        .map(|json| serde_json::from_str(&json).map_err(to_js_err))
        .transpose()
}

/// Render a check report as plain text.
///
/// # Errors
///
/// Rejects `report_json` when it is malformed or incompatible with the check
/// report schema.
/// @param report_json - Check-report JSON.
#[wasm_bindgen(js_name = renderCheckReportText)]
pub fn render_check_report_text(report_json: &str) -> Result<String, JsValue> {
    let report: finstack_quant_statements::checks::CheckReport =
        serde_json::from_str(report_json).map_err(to_js_err)?;
    Ok(finstack_quant_statements_analytics::analysis::CheckReportRenderer::render_text(&report))
}

/// Render a check report as HTML.
///
/// # Errors
///
/// Rejects `report_json` when it is malformed or incompatible with the check
/// report schema.
/// @param report_json - Check-report JSON.
#[wasm_bindgen(js_name = renderCheckReportHtml)]
pub fn render_check_report_html(report_json: &str) -> Result<String, JsValue> {
    let report: finstack_quant_statements::checks::CheckReport =
        serde_json::from_str(report_json).map_err(to_js_err)?;
    Ok(finstack_quant_statements_analytics::analysis::CheckReportRenderer::render_html(&report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::StatementResult;
    use finstack_quant_statements::types::AmountOrScalar;

    fn test_model_json() -> String {
        let q1 = PeriodId::quarter(2024, 1).expect("valid period fixture");
        let model = ModelBuilder::new("test_model")
            .periods("2024Q1..Q2", None)
            .expect("periods")
            .value(
                "revenue",
                &[
                    (q1, AmountOrScalar::scalar(100_000.0)),
                    (
                        PeriodId::quarter(2024, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(110_000.0),
                    ),
                ],
            )
            .value(
                "cogs",
                &[
                    (q1, AmountOrScalar::scalar(40_000.0)),
                    (
                        PeriodId::quarter(2024, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(44_000.0),
                    ),
                ],
            )
            .compute("gross_profit", "revenue - cogs")
            .expect("compute")
            .build()
            .expect("build");
        serde_json::to_string(&model).expect("serialize")
    }

    fn evaluated_results() -> (String, String) {
        let model_json = test_model_json();
        let model: finstack_quant_statements::FinancialModelSpec =
            serde_json::from_str(&model_json).expect("parse");
        let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
        let results = evaluator.evaluate(&model).expect("evaluate");
        let results_json = serde_json::to_string(&results).expect("serialize results");
        (model_json, results_json)
    }

    #[test]
    fn credit_assessment_report_accepts_minimal_results() {
        let results = StatementResult::default();
        let results_json = serde_json::to_string(&results).expect("serialize results");
        let text = credit_assessment_report_text(&results_json, "2024").expect("report");
        assert!(text.contains("Credit Assessment"));
    }

    #[test]
    fn trace_dependencies_renders_for_simple_model() {
        let model_json = test_model_json();
        let tree = trace_dependencies(&model_json, "gross_profit").expect("trace");
        assert!(!tree.is_empty());
        assert!(tree.contains("revenue") || tree.contains("gross_profit"));
    }

    #[test]
    fn explain_formula_text_succeeds() {
        let (model_json, results_json) = evaluated_results();
        let explanation =
            explain_formula_text(&model_json, &results_json, "gross_profit", "2024Q1")
                .expect("explain");
        assert!(!explanation.is_empty());
    }

    #[test]
    fn credit_assessment_report_with_data() {
        let (_, results_json) = evaluated_results();
        let text = credit_assessment_report_text(&results_json, "2024Q1").expect("report");
        assert!(text.contains("Credit Assessment"));
    }

    #[test]
    fn credit_assessment_returns_structured_json() {
        // `credit_assessment` now returns a `JsValue`, unconstructible off
        // wasm32; assert the serializable shape it hands to `to_js_value`.
        let (_, results_json) = evaluated_results();
        let results: finstack_quant_statements::evaluator::StatementResult =
            serde_json::from_str(&results_json).expect("parse results");
        let assessment = finstack_quant_statements_analytics::analysis::CreditAssessment::compute(
            &results,
            "2024Q1".parse().expect("period"),
        );
        let parsed = serde_json::to_value(&assessment).expect("serialize");
        assert!(parsed.get("period").is_some());
        assert!(parsed.get("series").map(|s| s.is_array()).unwrap_or(false));
    }

    #[test]
    fn run_sensitivity_diagonal() {
        let model_json = test_model_json();
        let config = finstack_quant_statements_analytics::analysis::SensitivityConfig {
            mode: finstack_quant_statements_analytics::analysis::SensitivityMode::Diagonal,
            parameters: vec![
                finstack_quant_statements_analytics::analysis::ParameterSpec {
                    node_id: "revenue".to_string(),
                    period_id: PeriodId::quarter(2024, 1).expect("valid period fixture"),
                    base_value: 100_000.0,
                    perturbations: vec![-0.1, 0.0, 0.1],
                },
            ],
            target_metrics: vec!["gross_profit".to_string()],
        };
        // `run_sensitivity` returns a `JsValue`; exercise the analyzer it
        // delegates to and assert the serializable shape.
        let model: finstack_quant_statements::FinancialModelSpec =
            serde_json::from_str(&model_json).expect("parse model");
        let analyzer =
            finstack_quant_statements_analytics::analysis::SensitivityAnalyzer::new(&model);
        let result = analyzer.run(&config).expect("sensitivity");
        let parsed = serde_json::to_value(&result).expect("serialize");
        assert!(parsed.is_object() || parsed.is_array());
    }

    #[test]
    fn generate_tornado_from_sensitivity() {
        let model_json = test_model_json();
        let config = finstack_quant_statements_analytics::analysis::SensitivityConfig {
            mode: finstack_quant_statements_analytics::analysis::SensitivityMode::Tornado,
            parameters: vec![
                finstack_quant_statements_analytics::analysis::ParameterSpec {
                    node_id: "revenue".to_string(),
                    period_id: PeriodId::quarter(2024, 1).expect("valid period fixture"),
                    base_value: 100_000.0,
                    perturbations: vec![-0.1, 0.1],
                },
            ],
            target_metrics: vec!["gross_profit".to_string()],
        };
        let model: finstack_quant_statements::FinancialModelSpec =
            serde_json::from_str(&model_json).expect("parse model");
        let analyzer =
            finstack_quant_statements_analytics::analysis::SensitivityAnalyzer::new(&model);
        let result = analyzer.run(&config).expect("sensitivity");
        let entries = finstack_quant_statements_analytics::analysis::generate_tornado_entries(
            &result,
            "gross_profit",
            None,
        );
        assert!(!entries.is_empty());
    }

    #[test]
    fn run_variance_between_two_results() {
        let (model_json, _) = evaluated_results();
        let model: finstack_quant_statements::FinancialModelSpec =
            serde_json::from_str(&model_json).expect("parse model");
        let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
        let base = evaluator.evaluate(&model).expect("eval base");
        let comparison = evaluator.evaluate(&model).expect("eval comparison");
        let config = finstack_quant_statements_analytics::analysis::VarianceConfig {
            baseline_label: "base".to_string(),
            comparison_label: "comp".to_string(),
            metrics: vec!["revenue".to_string(), "gross_profit".to_string()],
            periods: vec![PeriodId::quarter(2024, 1).expect("valid period fixture")],
        };
        // `run_variance` returns a `JsValue`; assert the report it serializes.
        let analyzer = finstack_quant_statements_analytics::analysis::VarianceAnalyzer::new(
            &base,
            &comparison,
        );
        let report = analyzer.compute(&config).expect("variance");
        let parsed = serde_json::to_value(&report).expect("serialize");
        assert!(parsed.is_object());
    }

    #[test]
    fn evaluate_scenario_set_with_override() {
        let model_json = test_model_json();
        let mut overrides = indexmap::IndexMap::new();
        overrides.insert(
            "revenue".to_string(),
            finstack_quant_statements::types::AmountOrScalar::scalar(200_000.0),
        );
        let scenario_set = finstack_quant_statements_analytics::analysis::ScenarioSet {
            scenarios: indexmap::indexmap! {
                "upside".to_string() => finstack_quant_statements_analytics::analysis::ScenarioDefinition {
                    parent: None,
                    overrides,
                    period_overrides: indexmap::IndexMap::new(),
                },
            },
        };
        // `evaluate_scenario_set` returns a `JsValue`; assert the map it
        // serializes.
        let model: finstack_quant_statements::FinancialModelSpec =
            serde_json::from_str(&model_json).expect("parse model");
        let results = scenario_set.evaluate_all(&model).expect("eval");
        let parsed = serde_json::to_value(&results.scenarios).expect("serialize");
        assert!(parsed.is_object());
        assert!(parsed.get("upside").is_some());
    }

    #[test]
    fn evaluate_scenario_set_rejects_removed_model_id() {
        // Error paths build a `JsValue`, which aborts off wasm32; assert the
        // serde rejection the binding surfaces.
        assert!(
            serde_json::from_str::<finstack_quant_statements_analytics::analysis::ScenarioSet>(
                r#"{"scenarios":{"base":{"model_id":"legacy"}}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn run_checks_includes_formula_checks() {
        let model_json = test_model_json();
        let spec_json = serde_json::json!({
            "name": "formula suite",
            "builtin_checks": [],
            "formula_checks": [{
                "id": "revenue_positive",
                "name": "Revenue must be positive",
                "category": "internal_consistency",
                "severity": "error",
                "formula": "revenue > 0",
                "message_template": "Revenue not positive in {period}",
                "tolerance": null
            }]
        })
        .to_string();
        let model: finstack_quant_statements::FinancialModelSpec =
            serde_json::from_str(&model_json).expect("model");
        let spec: finstack_quant_statements::checks::CheckSuiteSpec =
            serde_json::from_str(&spec_json).expect("spec");
        let report = spec
            .resolve()
            .expect("resolve")
            .run_model(&model, None)
            .expect("run checks");

        assert_eq!(report.results[0].check_id, "revenue_positive");
    }

    #[test]
    fn goal_seek_rejects_half_specified_bounds() {
        let msg = goal_seek_bounds(Some(0.0), None).expect_err("half-specified bounds must error");
        assert!(msg.contains("bounds_lo and bounds_hi"), "got: {msg}");
        let msg = goal_seek_bounds(None, Some(1.0)).expect_err("half-specified bounds must error");
        assert!(msg.contains("bounds_lo and bounds_hi"), "got: {msg}");
        assert_eq!(
            goal_seek_bounds(Some(0.0), Some(1.0)).expect("both bounds valid"),
            Some((0.0, 1.0))
        );
        assert_eq!(goal_seek_bounds(None, None).expect("no bounds valid"), None);
    }
}
