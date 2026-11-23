//! Sensitivity, variance, scenario-set, backtest, and tornado WASM entry points.

use crate::utils::input::{js_f64_seq, js_string, js_string_seq, json_text};
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

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
    let model = crate::api::statements::load_model_text(model_json)?;

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
    let model = crate::api::statements::load_model_text(model_json)?;

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
    let model = crate::api::statements::load_model(&model_json, "modelJson")?;
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
