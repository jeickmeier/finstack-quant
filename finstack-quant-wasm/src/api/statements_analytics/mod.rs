//! WASM bindings for the `finstack-quant-statements-analytics` crate.
//!
//! Exposes financial statement analysis functions. Inputs are JSON strings
//! or plain objects; results are structured JavaScript objects, except the
//! `*Text` functions, which return formatted text. The stateful pieces are
//! classes: `DependencyTracer`, `CorkscrewExtension` and
//! `CreditScorecardExtension`.

mod capital_structure;
mod comps;
mod covenants;
mod ecl;
mod handles;
mod members;
mod templates;

pub use handles::{JsCorkscrewExtension, JsCreditScorecardExtension, JsDependencyTracer};

pub use capital_structure::capital_structure_summary;
pub use covenants::{evaluate_covenants, forecast_breaches};

pub use comps::{
    compute_multiple, peer_stats, percentile_rank, regression_fair_value, relative_value_report,
    score_relative_value, z_score,
};

use crate::utils::input::{
    from_js_json, js_f64, js_f64_seq, js_opt_f64, js_opt_string, js_string, js_string_seq,
    json_text, opt_json_text,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements::types::FinancialModelSpec;
use wasm_bindgen::prelude::*;

/// Parse and semantically validate a `FinancialModelSpec` argument (object or JSON).
pub(super) fn parse_model(model: &JsValue) -> Result<FinancialModelSpec, JsValue> {
    FinancialModelSpec::from_json(&json_text(model, "model")?).map_err(to_js_err)
}

/// Run a sensitivity analysis on a financial model.
///
/// Accepts JSON strings for the model spec and sensitivity configuration,
/// evaluates all perturbation scenarios, and returns the `SensitivityResult`
/// as a structured JavaScript object. `generateTornadoEntries` still takes the
/// result as JSON, so pass `JSON.stringify(result)` when chaining the two.
///
/// Each perturbation replaces its node's value in its one forecast period
/// (Value > Forecast > Formula). It does not re-anchor the node's own
/// forecast: later periods of a growth, seasonal or stochastic forecast keep
/// their unshocked base. Formula nodes that read the shocked cell move, in its
/// period and in later periods reached through `lag`, `ttm` or `cumsum`. Add
/// one parameter per period to shock a whole forecast path.
///
/// # Errors
///
/// Rejects malformed model or configuration JSON, invalid sensitivity modes or
/// parameter perturbations, duplicate parameters, non-finite values, requests
/// over 128 parameters, 10,000 scenarios or 10 million model node-period
/// evaluation cells, missing model nodes or periods, model-evaluation failures,
/// or failure to serialize the sensitivity result to JavaScript.
/// @param modelJson - Financial-model specification JSON.
/// @param configJson - `SensitivityConfig` object or JSON text: `mode` (`diagonal`, `full_grid` or `tornado`), `parameters` (each `node_id`, `period_id`, `base_value` and exactly one of absolute `perturbations` or `pct_range` percentage moves of `base_value`, `10` meaning +10%) and `target_metrics`.
/// @returns Sensitivity results for each perturbation scenario.
#[wasm_bindgen(js_name = runSensitivity, unchecked_return_type = "SensitivityResult")]
pub fn run_sensitivity(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "configJson", unchecked_param_type = "JsonInput")]
    config_json: JsValue,
) -> Result<JsValue, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let config_json: &str = &json_text(&config_json, "configJson")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;

    let config: finstack_quant_statements_analytics::analysis::SensitivityConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;

    let result = finstack_quant_statements_analytics::analysis::run_sensitivity(&model, &config)
        .map_err(to_js_err)?;

    to_js_value(&result)
}

/// Run a variance analysis comparing two evaluated statement results.
///
/// Returns the variance report as a structured JavaScript object.
///
/// # Errors
///
/// Rejects malformed result or configuration JSON, empty metric or period
/// selections, mismatched metric types or currencies, a requested value
/// missing from either result, or failure to serialize the variance report
/// to JavaScript.
/// @param baseJson - Base statement-result JSON.
/// @param comparisonJson - Comparison statement-result JSON.
/// @param configJson - Configuration JSON for this call.
/// @returns Variance report comparing the two evaluated statement results.
#[wasm_bindgen(js_name = runVariance, unchecked_return_type = "VarianceReport")]
pub fn run_variance(
    #[wasm_bindgen(js_name = "baseJson", unchecked_param_type = "JsonInput")] base_json: JsValue,
    #[wasm_bindgen(js_name = "comparisonJson", unchecked_param_type = "JsonInput")] comparison_json: JsValue,
    #[wasm_bindgen(js_name = "configJson", unchecked_param_type = "JsonInput")]
    config_json: JsValue,
) -> Result<JsValue, JsValue> {
    let base_json: &str = &json_text(&base_json, "baseJson")?;
    let comparison_json: &str = &json_text(&comparison_json, "comparisonJson")?;
    let config_json: &str = &json_text(&config_json, "configJson")?;
    let base: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(base_json).map_err(to_js_err)?;

    let comparison: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(comparison_json).map_err(to_js_err)?;

    let config: finstack_quant_statements_analytics::analysis::VarianceConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;

    let report =
        finstack_quant_statements_analytics::analysis::run_variance(&base, &comparison, &config)
            .map_err(to_js_err)?;

    to_js_value(&report)
}

/// Evaluate all scenarios in a scenario set against a base model.
///
/// Returns a structured JavaScript object mapping scenario names to their
/// statement results. Like `Evaluator.evaluate`, a capital structure is
/// evaluated from an empty market, which serves fixed-rate instruments; use
/// `evaluateScenarioSetWithMarket` for instruments that need market data.
/// A scenario's `forecast_overrides` replace whole node forecast specs.
///
/// # Errors
///
/// Rejects malformed model or scenario-set JSON, an empty scenario set,
/// invalid parent chains, overrides of missing nodes, invalid forecast
/// overrides, a capital-structure instrument needing market data (kind
/// `not_found`), failure to evaluate any scenario, or failure to serialize
/// the result map to JavaScript.
/// @param modelJson - Financial-model specification JSON.
/// @param scenarioSetJson - Scenario-set JSON keyed by scenario name.
/// @returns Statement results keyed by scenario name.
#[wasm_bindgen(js_name = evaluateScenarioSet, unchecked_return_type = "ScenarioResults")]
pub fn evaluate_scenario_set(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "scenarioSetJson", unchecked_param_type = "JsonInput")]
    scenario_set_json: JsValue,
) -> Result<JsValue, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let scenario_set_json: &str = &json_text(&scenario_set_json, "scenarioSetJson")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;

    let scenario_set: finstack_quant_statements_analytics::analysis::ScenarioSet =
        serde_json::from_str(scenario_set_json).map_err(to_js_err)?;

    let results = scenario_set.evaluate_all(&model).map_err(to_js_err)?;
    // `ScenarioResults` is `#[serde(transparent)]`: its wire form is the
    // `{scenario_name: StatementResult}` map.
    to_js_value(&results)
}

/// Evaluate all scenarios in a scenario set against a market as of a date.
///
/// Twin of Python `evaluate_scenario_set_with_market` (Rust
/// `ScenarioSet::evaluate_all_with_market`). Each scenario is evaluated like
/// `Evaluator.evaluateWithMarket`: capital-structure instruments are priced
/// from `market` as of `asOf` (floating coupons, swaps, caps), so every result
/// carries `cs_cashflows`, and explicit actuals not yet available at `asOf`
/// are hidden. `evaluateScenarioSet` evaluates the capital structure from an
/// empty market, which serves fixed-rate instruments only.
///
/// # Errors
///
/// Rejects malformed model, scenario-set or market input, an `asOf` that is
/// not an ISO date, an empty scenario set, invalid parent chains, overrides
/// of missing or incompatible nodes, invalid forecast overrides, an
/// instrument needing market data `market` lacks (kind `not_found`), or a
/// failure to evaluate any scenario.
/// @param modelJson - Financial-model specification JSON.
/// @param scenarioSetJson - Scenario-set JSON keyed by scenario name.
/// @param market - `MarketContext` state supplying curves, fixings and FX (object or JSON), shared by every scenario.
/// @param asOf - ISO 8601 date (`"2025-01-15"`): the pricing date for capital-structure instruments and the cutoff for explicit-value visibility.
/// @returns Map of scenario name to `StatementResult` plain object.
#[wasm_bindgen(js_name = evaluateScenarioSetWithMarket, unchecked_return_type = "ScenarioResults")]
pub fn evaluate_scenario_set_with_market(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "scenarioSetJson", unchecked_param_type = "JsonInput")]
    scenario_set_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] market: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
) -> Result<JsValue, JsValue> {
    let model =
        FinancialModelSpec::from_json(&json_text(&model_json, "modelJson")?).map_err(to_js_err)?;
    let scenario_set: finstack_quant_statements_analytics::analysis::ScenarioSet =
        serde_json::from_str(&json_text(&scenario_set_json, "scenarioSetJson")?)
            .map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        crate::utils::input::from_js_json(&market, "market")?;
    let as_of = crate::utils::wire::js_date(&as_of, "asOf")?;
    let results = scenario_set
        .evaluate_all_with_market(&model, &market, as_of)
        .map_err(to_js_err)?;
    to_js_value(&results)
}

/// Scenario comparison table: one row per `(period, metric)` and one column
/// per scenario, plus a `<scenario>_vs_<baseline>_frac` change column for
/// every non-baseline scenario.
///
/// Free-function twin of Python `ScenarioResults.to_comparison_table` (Rust
/// `ScenarioResults::to_comparison_table`). The baseline is the scenario named
/// `"base"` when present, otherwise the first scenario; the fractional change
/// is `(scenario - baseline) / baseline`, `0.0` when the baseline is
/// effectively zero.
/// @param scenarioResultsJson - The scenario map returned by `evaluateScenarioSet` (object or JSON).
/// @param metrics - Node identifiers to include, in row order within each period.
/// @returns `TableEnvelope` with `period` and `metric` columns, one value column per scenario and the `_frac` change columns.
///
/// # Errors
///
/// Throws with kind `validation` if the results input is malformed, the
/// result set or metric list is empty, or table construction fails.
#[wasm_bindgen(js_name = scenarioComparisonTable, unchecked_return_type = "TableEnvelope")]
pub fn scenario_comparison_table(
    #[wasm_bindgen(
        js_name = "scenarioResultsJson",
        unchecked_param_type = "ScenarioResults | string"
    )]
    scenario_results_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string[]")] metrics: JsValue,
) -> Result<JsValue, JsValue> {
    let results: finstack_quant_statements_analytics::analysis::ScenarioResults =
        serde_json::from_str(&json_text(&scenario_results_json, "scenarioResultsJson")?)
            .map_err(to_js_err)?;
    let metrics = js_string_seq(&metrics, "metrics")?;
    let metrics: Vec<&str> = metrics.iter().map(String::as_str).collect();
    crate::utils::to_js_table_numeric(&results.to_comparison_table(&metrics).map_err(to_js_err)?)
}

/// Compute forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).
///
/// Takes two float arrays (actual, forecast) and returns the serde form of
/// the Rust `ForecastMetrics` (`mae`, `mape`, `mape_effective_n`, `smape`,
/// `rmse`, `n`). Unavailable MAPE/sMAPE values are `null`, including both
/// percentage metrics for a perfect forecast of an all-zero series.
///
/// # Errors
///
/// Rejects inputs that cannot be decoded as numeric JavaScript arrays, arrays
/// with unequal lengths, empty arrays, non-finite observations, arithmetic
/// overflow, or metrics that cannot be serialized to JavaScript.
/// @param actual - Finite actual realized values in consistent units aligned one-for-one with the forecast series; negative values are accepted.
/// @param forecast - Finite forecast values in the same units aligned one-for-one with the actual realized series.
/// @returns Backtest forecast accuracy metrics for the selected series.
#[wasm_bindgen(js_name = backtestForecast, unchecked_return_type = "ForecastMetrics")]
pub fn backtest_forecast(
    #[wasm_bindgen(unchecked_param_type = "number[]")] actual: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number[]")] forecast: JsValue,
) -> Result<JsValue, JsValue> {
    let actual_vec = js_f64_seq(&actual, "actual")?;
    let forecast_vec = js_f64_seq(&forecast, "forecast")?;

    let metrics = finstack_quant_statements_analytics::analysis::backtest_forecast(
        &actual_vec,
        &forecast_vec,
    )
    .map_err(to_js_err)?;
    crate::utils::to_js_value_numeric(&metrics)
}

/// Generate tornado chart entries for a sensitivity result.
///
/// # Errors
///
/// Rejects malformed `result_json`, an invalid `period` identifier, or a
/// one-at-a-time run whose scenario does not identify exactly one perturbed
/// parameter, an absent unperturbed baseline, a missing target metric or period,
/// non-finite metric values or impacts, or failure to convert the entries to
/// JavaScript.
/// @returns Structured tornado entries sorted by descending absolute swing, with parameter IDs preserving the perturbed `node@period` key; `null` for a full-grid run, whose scenarios combine parameters and cannot be ranked.
/// @param resultJson - Result JSON produced by a prior call.
/// @param metricNode - Statement metric node identifier selected for the requested analysis.
/// @param period - Required model period, such as `"2025Q4"`, at which the metric is read in the baseline and every scenario; a shock replaces one forecast cell, so the metric moves only in the shocked period and in later periods whose formulas read it.
#[wasm_bindgen(js_name = generateTornadoEntries, unchecked_return_type = "TornadoEntry[] | null")]
pub fn generate_tornado_entries(
    #[wasm_bindgen(js_name = "resultJson", unchecked_param_type = "JsonInput")]
    result_json: JsValue,
    #[wasm_bindgen(js_name = "metricNode", unchecked_param_type = "string")] metric_node: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] period: JsValue,
) -> Result<JsValue, JsValue> {
    let result_json: &str = &json_text(&result_json, "resultJson")?;
    let metric_node: &str = &js_string(&metric_node, "metricNode")?;
    let period: finstack_quant_core::dates::PeriodId =
        js_string(&period, "period")?.parse().map_err(to_js_err)?;
    let result: finstack_quant_statements_analytics::analysis::SensitivityResult =
        serde_json::from_str(result_json).map_err(to_js_err)?;
    let entries = finstack_quant_statements_analytics::analysis::generate_tornado_entries(
        &result,
        metric_node,
        period,
    )
    .map_err(to_js_err)?;
    to_js_value(&entries)
}

/// Rank the headline DCF assumptions by enterprise-value impact.
///
/// The statement model is evaluated once; each shocked point re-runs only the
/// DCF. Returns a structured JavaScript object with the baseline enterprise
/// value, tornado entries as deltas versus that baseline sorted by descending
/// absolute swing, and the effective (possibly clamped) shock levels.
///
/// # Errors
///
/// Rejects malformed model, terminal-value or options JSON (an
/// unknown options key included), model-evaluation failures, a missing UFCF
/// series or model currency, inconsistent WACC or terminal-value assumptions,
/// a missing or incomplete exit-multiple metric node, missing bridge inputs,
/// valuation failures, or failure to serialize the sensitivity result.
/// @param modelJson - Financial-model specification JSON.
/// @param wacc - Baseline weighted average cost of capital in decimal form (0.10 = 10%).
/// @param terminalValueJson - Terminal-value spec JSON selecting whether growth or the exit multiple is shocked.
/// @param ufcfNode - Node identifier holding monetary unlevered free cash flow in model currency for the forecast periods; omitted uses the canonical "ufcf" node. Gordon Growth / H-Model terminal cash flow requires a complete contiguous calendar year at the forecast end.
/// @param netDebtOverride - Optional net debt in model currency; otherwise requires debt and cash in that currency from a period ending on or before valuation.
/// @param optionsJson - Optional Rust `DcfOptions` JSON; every field is optional and a missing one takes its default: `mid_year_convention` (false), `wacc_sensitivity_bump` (0.01 = +/-100 bp), `wacc_denominator_epsilon` (0.005), `max_stable_growth_rate` (0.05), `exit_multiple_bump` (`{"absolute": 1.0}` turns or `{"relative": 0.10}`), `exit_multiple_metric_node` (flow node whose complete trailing year supplies the exit-multiple metric), `equity_bridge`, `shares_outstanding`, `valuation_discounts`. Unknown keys are rejected.
/// @returns Ranked DCF-assumption impacts on enterprise value.
#[wasm_bindgen(js_name = dcfSensitivity, unchecked_return_type = "DcfSensitivityResult")]
pub fn dcf_sensitivity(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] wacc: JsValue,
    #[wasm_bindgen(js_name = "terminalValueJson", unchecked_param_type = "JsonInput")]
    terminal_value_json: JsValue,
    #[wasm_bindgen(js_name = "ufcfNode", unchecked_optional_param_type = "string | null")]
    ufcf_node: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "netDebtOverride",
        unchecked_optional_param_type = "number | null"
    )]
    net_debt_override: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "optionsJson",
        unchecked_optional_param_type = "JsonInput | null"
    )]
    options_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let wacc = js_f64(&wacc, "wacc")?;
    let net_debt_override = js_opt_f64(net_debt_override.as_ref(), "netDebtOverride")?;
    use finstack_quant_statements_analytics::analysis::{DEFAULT_UFCF_NODE, DcfOptions};

    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let terminal_value_json: &str = &json_text(&terminal_value_json, "terminalValueJson")?;
    let ufcf_node = js_opt_string(ufcf_node.as_ref(), "ufcfNode")?;
    let options_json = opt_json_text(options_json.as_ref(), "optionsJson")?;

    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
    let terminal_value: finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec =
        serde_json::from_str(terminal_value_json).map_err(to_js_err)?;
    let options: DcfOptions = options_json
        .map(|json| serde_json::from_str(&json).map_err(to_js_err))
        .transpose()?
        .unwrap_or_default();

    let result = finstack_quant_statements_analytics::analysis::dcf_sensitivity(
        &model,
        wacc,
        terminal_value,
        ufcf_node.as_deref().unwrap_or(DEFAULT_UFCF_NODE),
        net_debt_override,
        &options,
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
/// period. When the config carries `check_mappings`, the Rust LBO check suite
/// runs against the same evaluation and fills `checks`; otherwise `checks` is
/// `null`. IRR is out of scope: pair the returned `exit_equity_proceeds` with
/// the equity outflow at close and call `portfolio.mwrXirr`.
///
/// # Errors
///
/// Rejects malformed model or config JSON (unknown config or mapping keys
/// included), an invalid `exit_period`, model evaluation or lookup failures, a
/// missing model currency or period, non-finite transaction inputs or model
/// values, negative tranche amounts, a non-positive sponsor equity check,
/// check-suite failures, or failure to serialize the result to JavaScript.
/// The result is a structured JavaScript object.
/// @param modelJson - Financial-model specification JSON.
/// @param configJson - Rust `LboConfig` JSON: `entry_multiple` (8.5 = 8.5x), `entry_metric_node`, `transaction_fees` (model currency), `sources` (`[{"name", "amount"}]` funded at close, model currency), `exit_multiple`, `exit_metric_node`, `exit_net_debt_node`, `exit_period` (e.g. "2029"), and optional `check_mappings` (`{"three_statement", "credit"}`). Every field except `check_mappings` is required.
/// @returns Leveraged-buyout evaluation result against the statement model.
#[wasm_bindgen(js_name = evaluateLbo, unchecked_return_type = "LboResult")]
pub fn evaluate_lbo(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "configJson", unchecked_param_type = "JsonInput")]
    config_json: JsValue,
) -> Result<JsValue, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let config_json: &str = &json_text(&config_json, "configJson")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
    let config: finstack_quant_statements_analytics::analysis::LboConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;

    let result = finstack_quant_statements_analytics::analysis::evaluate_lbo(&model, &config)
        .map_err(to_js_err)?;

    // Serde form of the Rust `LboResult`; monetary fields are `Money` wire
    // objects (`{amount, currency}`).
    to_js_value(&result)
}

/// Weighted-average cost of capital (WACC).
///
/// `WACC = w_E * r_E + w_D * r_D * (1 - T)`.
///
/// # Errors
///
/// Rejects malformed inputs, any non-finite value, negative capital weights,
/// weights that do not sum to one within tolerance, or a `tax_rate` outside
/// `[0, 1]`.
/// @param inputs - `WaccInputs` object or JSON text: `equity_weight` and `debt_weight` (decimal fractions of total capital summing to 1.0), `cost_of_equity` and pre-tax `cost_of_debt` (decimals, `0.06` = 6%) and `tax_rate` (decimal in [0, 1]).
/// @returns The blended discount rate as a decimal fraction.
#[wasm_bindgen(js_name = wacc)]
pub fn wacc(
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.WaccInputs | string")]
    inputs: JsValue,
) -> Result<f64, JsValue> {
    let inputs: finstack_quant_statements_analytics::analysis::WaccInputs =
        from_js_json(&inputs, "inputs")?;
    finstack_quant_statements_analytics::analysis::wacc(&inputs).map_err(to_js_err)
}

/// Find the driver value that makes a target node reach a target value.
///
/// Twin of Python `goal_seek` (Rust `goal_seek`). Returns the serde form of
/// the Rust `GoalSeekResult`: `solved_value`, the solve diagnostics
/// (`residual` and `tolerance` in the target node's units, `evaluations`,
/// `converged`) and `model`, the input model with the solved driver written in
/// when `update_model` is `true` and `null` otherwise. The input model is never
/// modified; pass `model` straight back as the `modelJson` of another call.
///
/// # Errors
///
/// Rejects a malformed `modelJson` or `spec` (a missing or unknown key
/// included), invalid target or driver period identifiers, missing target or
/// driver nodes or periods, non-finite or unordered bounds, model-evaluation or
/// solver-convergence failures, or failure to serialize the result.
/// @param modelJson - Financial-model specification (object or JSON).
/// @param spec - Rust `GoalSeekSpec` (object or JSON); an unknown key is rejected: `target_node` (node driven toward the target), `target_period` (period label of the target, e.g. `"2025Q4"`), `target_value` (finite target in the target node's units), `driver_node` (node the solve adjusts), `driver_period` (period label of the driver), `update_model` (whether the result carries the model with the solved driver applied) and the optional `bounds` (`[lower, upper]` search bracket in the driver node's units; omitted searches outward from the driver's current value). Every key except `bounds` is required.
/// @returns Solved driver value and the updated model (or `null`).
#[wasm_bindgen(js_name = goalSeek, unchecked_return_type = "GoalSeekResult")]
pub fn goal_seek(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.GoalSeekSpec | string")]
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
    let spec: finstack_quant_statements_analytics::analysis::GoalSeekSpec =
        from_js_json(&spec, "spec")?;
    let result = finstack_quant_statements_analytics::analysis::goal_seek(&model, &spec)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Explain a formula for a specific node and period (JSON in, structured
/// object out).
///
/// Returns the Rust `Explanation` as a structured object: `source` names the
/// layer that produced the value (`value`, `forecast`, `formula` or
/// `where_masked`), `forecast` carries the node's forecast spec for a
/// forecast cell, and `breakdown` lists the formula's components (including
/// `cs.*` references) when the formula produced the value. A non-finite
/// `final_value` or breakdown `value` (for example a `lag` node at the first
/// period) is the JavaScript number `NaN` or `±Infinity`. The Rust JSON form
/// writes it as the string `"nan"`, `"inf"` or `"-inf"`.
///
/// # Errors
///
/// Rejects malformed model or result JSON, an invalid `period` identifier, a
/// missing model node or node-period result, an invalid formula used to build
/// the breakdown, or failure to serialize the explanation to JavaScript.
/// @param modelJson - Financial-model specification JSON.
/// @param resultsJson - Evaluated statement-result JSON.
/// @param nodeId - Stable node identifier used to select the required domain object.
/// @param period - Model period label for the requested statement value or calculation.
/// @returns Structured formula breakdown for the selected node and period.
#[wasm_bindgen(js_name = explainFormula, unchecked_return_type = "Explanation")]
pub fn explain_formula(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "resultsJson", unchecked_param_type = "JsonInput")]
    results_json: JsValue,
    #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] period: JsValue,
) -> Result<JsValue, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let results_json: &str = &json_text(&results_json, "resultsJson")?;
    let node_id: &str = &js_string(&node_id, "nodeId")?;
    let period: &str = &js_string(&period, "period")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let pid: finstack_quant_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let explanation = finstack_quant_statements_analytics::analysis::explain_formula(
        &model, &results, node_id, &pid,
    )
    .map_err(to_js_err)?;
    let js = crate::utils::to_js_value_numeric(&explanation)?;
    let breakdown = js_sys::Reflect::get(&js, &JsValue::from("breakdown"))?;
    crate::utils::restore_non_finite_rows::<
        finstack_quant_statements_analytics::analysis::ExplanationStep,
    >(&breakdown)?;
    Ok(js)
}

/// Explain a formula for a specific node and period as formatted text.
///
/// # Errors
///
/// Rejects malformed model or result JSON, an invalid `period` identifier, a
/// missing model node or node-period result, or an invalid formula used to
/// build the explanation breakdown.
/// @param modelJson - Financial-model specification JSON.
/// @param resultsJson - Evaluated statement-result JSON.
/// @param nodeId - Stable node identifier used to select the required domain object.
/// @param period - Model period label for the requested statement value or calculation.
/// @returns Formatted formula explanation for the selected node and period.
#[wasm_bindgen(js_name = explainFormulaText)]
pub fn explain_formula_text(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "resultsJson", unchecked_param_type = "JsonInput")]
    results_json: JsValue,
    #[wasm_bindgen(js_name = "nodeId", unchecked_param_type = "string")] node_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] period: JsValue,
) -> Result<String, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let results_json: &str = &json_text(&results_json, "resultsJson")?;
    let node_id: &str = &js_string(&node_id, "nodeId")?;
    let period: &str = &js_string(&period, "period")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let pid: finstack_quant_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let explanation = finstack_quant_statements_analytics::analysis::explain_formula(
        &model, &results, node_id, &pid,
    )
    .map_err(to_js_err)?;
    Ok(explanation.to_text())
}

/// Generate a P&L summary report as formatted text.
///
/// # Errors
///
/// Rejects malformed `results_json`, `line_items` or `periods` values that are
/// not JavaScript string arrays, or any period string that is not a valid
/// statement period identifier.
/// @param resultsJson - Evaluated statement-result JSON.
/// @param lineItems - Ordered statement line-item definitions included in the summary report.
/// @param periods - Ordered period labels or observations aligned with the supplied data.
/// @returns Returns a human-readable text report, not JSON.
#[wasm_bindgen(js_name = plSummaryReportText)]
pub fn pl_summary_report_text(
    #[wasm_bindgen(js_name = "resultsJson", unchecked_param_type = "JsonInput")]
    results_json: JsValue,
    #[wasm_bindgen(js_name = "lineItems", unchecked_param_type = "string[]")] line_items: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string[]")] periods: JsValue,
) -> Result<String, JsValue> {
    let results_json: &str = &json_text(&results_json, "resultsJson")?;
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let items = js_string_seq(&line_items, "lineItems")?;
    let period_strs = js_string_seq(&periods, "periods")?;
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
/// Trailing-year ratios step through the results' period plan and are
/// omitted when the twelve months are incomplete; a daily plan needs every
/// calendar day.
///
/// # Errors
///
/// Rejects malformed `results_json`, a `period` value that is not a valid
/// statement period identifier, results holding `ebitda` or
/// `interest_expense` without a period plan, a `period` outside that plan,
/// or a weekly plan.
/// @param resultsJson - Evaluated statement-result JSON.
/// @param period - Statement period identifier, such as `2025Q4` or `2025A`.
/// @returns Returns a human-readable text report, not JSON.
#[wasm_bindgen(js_name = creditAssessmentReportText)]
pub fn credit_assessment_report_text(
    #[wasm_bindgen(js_name = "resultsJson", unchecked_param_type = "JsonInput")]
    results_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] period: JsValue,
) -> Result<String, JsValue> {
    let results_json: &str = &json_text(&results_json, "resultsJson")?;
    let period: &str = &js_string(&period, "period")?;
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let period: finstack_quant_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let report = finstack_quant_statements_analytics::analysis::CreditAssessmentReport::new(
        &results, period,
    )
    .map_err(to_js_err)?;
    Ok(report.to_string())
}

/// Compute a structured credit assessment (leverage, coverage, FCF).
///
/// Returns a structured JavaScript object whose `series` covers every period
/// of the results' `periods` up to `period`, in timeline order.
/// Ratios are `null` when their inputs have mixed scalar/money representations,
/// different currencies, or non-finite arithmetic.
/// Trailing-year ratios step through the results' period plan in its own
/// calendar: they need every annual, semiannual, quarterly or monthly period
/// of the twelve months, or every calendar day of a daily plan (a
/// business-day grid is incomplete, so its ratios are `null`). A weekly plan
/// is an error.
///
/// # Errors
///
/// Rejects malformed `results_json`, a `period` value that is not a valid
/// statement period identifier, a `period` that is not one of the results'
/// `periods` (kind `validation`), or failure to serialize the assessment to
/// JavaScript.
/// @param resultsJson - Evaluated statement-result JSON.
/// @param period - Statement period identifier, such as `2025Q4`, that is a period of the results in the model's own frequency: `2025` is the annual period, not the last quarter of a quarterly model.
/// @returns Credit-assessment result object from the statement results.
#[wasm_bindgen(js_name = creditAssessment, unchecked_return_type = "CreditAssessment")]
pub fn credit_assessment(
    #[wasm_bindgen(js_name = "resultsJson", unchecked_param_type = "JsonInput")]
    results_json: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] period: JsValue,
) -> Result<JsValue, JsValue> {
    let results_json: &str = &json_text(&results_json, "resultsJson")?;
    let period: &str = &js_string(&period, "period")?;
    let results: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let period: finstack_quant_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let assessment =
        finstack_quant_statements_analytics::analysis::CreditAssessment::compute(&results, period)
            .map_err(to_js_err)?;
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
/// @param modelJson - Financial-model specification JSON.
/// @param suiteSpecJson - Check-suite specification JSON.
/// @param resultsJson - Evaluated statement-result JSON.
#[wasm_bindgen(js_name = runChecks, unchecked_return_type = "CheckReport")]
pub fn run_checks(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "suiteSpecJson", unchecked_param_type = "JsonInput")] suite_spec_json: JsValue,
    #[wasm_bindgen(
        js_name = "resultsJson",
        unchecked_optional_param_type = "JsonInput | null"
    )]
    results_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let suite_spec_json: &str = &json_text(&suite_spec_json, "suiteSpecJson")?;
    let results_json = opt_json_text(results_json.as_ref(), "resultsJson")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
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
/// failures when results are omitted; incompatible data or invalid check
/// configuration; or failure to convert the report to JavaScript. A mapped node
/// that is missing from the model does not throw: its check is skipped or
/// reported as a finding.
/// @returns Structured three-statement check report with results and aggregate summary.
/// @param modelJson - Financial-model specification JSON.
/// @param mappingJson - `ThreeStatementMapping` (object or JSON) naming the model nodes that feed each check.
/// @param resultsJson - Evaluated statement-result JSON.
#[wasm_bindgen(js_name = runThreeStatementChecks, unchecked_return_type = "CheckReport")]
pub fn run_three_statement_checks(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "mappingJson", unchecked_param_type = "JsonInput")]
    mapping_json: JsValue,
    #[wasm_bindgen(
        js_name = "resultsJson",
        unchecked_optional_param_type = "JsonInput | null"
    )]
    results_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let mapping_json: &str = &json_text(&mapping_json, "mappingJson")?;
    let results_json = opt_json_text(results_json.as_ref(), "resultsJson")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
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
/// failures when results are omitted; incompatible data or invalid check
/// configuration; or failure to convert the report to JavaScript. A mapped node
/// that is missing from the model does not throw: its check is skipped or
/// reported as a finding.
/// @returns Structured credit-underwriting check report with results and aggregate summary.
/// @param modelJson - Financial-model specification JSON.
/// @param mappingJson - `CreditMapping` (object or JSON) naming the debt, EBITDA and interest nodes plus the optional warning bands (`leverage_warn` is a `(min, max)` band in turns).
/// @param resultsJson - Evaluated statement-result JSON.
#[wasm_bindgen(js_name = runCreditUnderwritingChecks, unchecked_return_type = "CheckReport")]
pub fn run_credit_underwriting_checks(
    #[wasm_bindgen(js_name = "modelJson", unchecked_param_type = "JsonInput")] model_json: JsValue,
    #[wasm_bindgen(js_name = "mappingJson", unchecked_param_type = "JsonInput")]
    mapping_json: JsValue,
    #[wasm_bindgen(
        js_name = "resultsJson",
        unchecked_optional_param_type = "JsonInput | null"
    )]
    results_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let mapping_json: &str = &json_text(&mapping_json, "mappingJson")?;
    let results_json = opt_json_text(results_json.as_ref(), "resultsJson")?;
    let model = FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
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
/// @param reportJson - Check-report JSON.
/// @returns Plain-text check report.
#[wasm_bindgen(js_name = renderCheckReportText)]
pub fn render_check_report_text(
    #[wasm_bindgen(js_name = "reportJson", unchecked_param_type = "JsonInput")]
    report_json: JsValue,
) -> Result<String, JsValue> {
    let report_json: &str = &json_text(&report_json, "reportJson")?;
    let report: finstack_quant_statements::checks::CheckReport =
        serde_json::from_str(report_json).map_err(to_js_err)?;
    Ok(finstack_quant_statements_analytics::analysis::render_check_report_text(&report))
}

/// Render a check report as HTML.
///
/// # Errors
///
/// Rejects `report_json` when it is malformed or incompatible with the check
/// report schema.
/// @param reportJson - Check-report JSON.
/// @returns HTML check report.
#[wasm_bindgen(js_name = renderCheckReportHtml)]
pub fn render_check_report_html(
    #[wasm_bindgen(js_name = "reportJson", unchecked_param_type = "JsonInput")]
    report_json: JsValue,
) -> Result<String, JsValue> {
    let report_json: &str = &json_text(&report_json, "reportJson")?;
    let report: finstack_quant_statements::checks::CheckReport =
        serde_json::from_str(report_json).map_err(to_js_err)?;
    Ok(finstack_quant_statements_analytics::analysis::render_check_report_html(&report))
}
