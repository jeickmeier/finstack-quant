//! Free-function twins of the Python analysis entry points and of the
//! methods on plain-object analysis types.

use crate::utils::input::{
    from_js_json, js_f64, js_opt_f64, js_opt_string, js_string, js_string_seq, js_uint, json_text,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_statements_analytics::analysis as fa;
use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
use wasm_bindgen::prelude::*;

fn present(value: Option<JsValue>) -> Option<JsValue> {
    value.filter(|v| !v.is_null() && !v.is_undefined())
}

fn parse_model(model: &JsValue) -> Result<FinancialModelSpec, JsValue> {
    FinancialModelSpec::from_json(&json_text(model, "model")?).map_err(to_js_err)
}

fn parse_periods(periods: &JsValue) -> Result<Vec<PeriodId>, JsValue> {
    js_string_seq(periods, "periods")?
        .iter()
        .map(|period| period.parse().map_err(to_js_err))
        .collect()
}

/// Value a company with a discounted cash flow of its unlevered free cash flow.
///
/// Twin of Python `evaluate_dcf` (Rust `evaluate_dcf_with_market`). The model
/// is evaluated, the UFCF series is discounted at `wacc`, the terminal value
/// is added, and the equity bridge turns enterprise value into equity value.
/// @param model - `FinancialModelSpec` (object or JSON); its metadata must carry a `currency`.
/// @param wacc - Weighted average cost of capital as a decimal (`0.10` = 10%).
/// @param terminal_value - `TerminalValueSpec`, e.g. from `terminalValueSpecGordonGrowth` (object or JSON).
/// @param ufcf_node - Node holding unlevered free cash flow; omitted uses the Rust default `"ufcf"`.
/// @param net_debt_override - Flat net debt in model currency used instead of the model-derived equity bridge.
/// @param options - Optional Rust `DcfOptions`; every field is optional and a missing one takes its Rust default (`mid_year_convention`, `equity_bridge`, `shares_outstanding`, `valuation_discounts`, `exit_multiple_metric_node`, ...). Unknown keys are rejected (object or JSON).
/// @param market - Optional `MarketContext` state used for statement evaluation, not for WACC discounting (object or JSON).
/// @param as_of - Optional ISO 8601 valuation date; required when `market` is supplied.
/// @returns `CorporateValuationResult`: enterprise value, terminal-value PV, net debt and equity value as `Money` wire objects, plus per-share value when shares are supplied.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, the model
/// currency is missing, the WACC and terminal-value assumptions are
/// inconsistent, or the valuation fails, and kind `not_found` if the UFCF or
/// exit-multiple metric node is missing.
#[wasm_bindgen(js_name = evaluateDcf)]
#[allow(clippy::too_many_arguments)]
pub fn evaluate_dcf(
    model: JsValue,
    wacc: JsValue,
    terminal_value: JsValue,
    ufcf_node: Option<JsValue>,
    net_debt_override: Option<JsValue>,
    options: Option<JsValue>,
    market: Option<JsValue>,
    as_of: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let model = parse_model(&model)?;
    let wacc = js_f64(&wacc, "wacc")?;
    let terminal_value: TerminalValueSpec = from_js_json(&terminal_value, "terminalValue")?;
    let ufcf_node = js_opt_string(ufcf_node.as_ref(), "ufcfNode")?;
    let net_debt_override = js_opt_f64(net_debt_override.as_ref(), "netDebtOverride")?;
    let options: fa::DcfOptions = match present(options) {
        Some(options) => from_js_json(&options, "options")?,
        None => fa::DcfOptions::default(),
    };
    let market: Option<finstack_quant_core::market_data::context::MarketContext> = present(market)
        .map(|market| from_js_json(&market, "market"))
        .transpose()?;
    let as_of = js_opt_string(as_of.as_ref(), "asOf")?
        .map(|date| crate::utils::parse_iso_date(&date))
        .transpose()?;
    let result = fa::evaluate_dcf_with_market(
        &model,
        wacc,
        terminal_value,
        ufcf_node.as_deref().unwrap_or(fa::DEFAULT_UFCF_NODE),
        net_debt_override,
        &options,
        market.as_ref(),
        as_of,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Run the corporate analysis pipeline: statements, optional DCF equity value and credit metrics.
///
/// Twin of Python `run_corporate_analysis` (Rust `run_corporate_analysis`).
/// The Python keyword arguments are the fields of `options` here.
/// @param model - `FinancialModelSpec` (object or JSON); its metadata must carry a `currency` when a DCF is requested.
/// @param options - Optional Rust `CorporateAnalysisOptions`; every field is optional: `wacc` (decimal; enables the DCF and requires `terminal_value`), `terminal_value`, `net_debt_override` (model currency), `cfads_node`, `interest_coverage_node` (default `"ebitda"`), `check_suite` (`CheckSuiteSpec`; DCF and capital-structure analyses need one including the `non_finite` check), `as_of` (ISO date) and `ltv_value_node`. Unknown keys are rejected (object or JSON).
/// @param market - Optional `MarketContext` state for statement evaluation and instrument pricing; requires `options.as_of` (object or JSON).
/// @returns `CorporateAnalysis`: the `statement` result, the `equity` valuation (or `null`), per-instrument `credit` metrics and `ev_suppressed_non_positive`.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `wacc` is set
/// without `terminal_value`, `market` is supplied without `as_of`, a required
/// check suite is missing, or the pipeline fails, and kind `not_found` if a
/// referenced node is missing.
#[wasm_bindgen(js_name = runCorporateAnalysis)]
pub fn run_corporate_analysis(
    model: JsValue,
    options: Option<JsValue>,
    market: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let model = parse_model(&model)?;
    let options: fa::CorporateAnalysisOptions = match present(options) {
        Some(options) => from_js_json(&options, "options")?,
        None => fa::CorporateAnalysisOptions::default(),
    };
    let market: Option<finstack_quant_core::market_data::context::MarketContext> = present(market)
        .map(|market| from_js_json(&market, "market"))
        .transpose()?;
    to_js_value(&fa::run_corporate_analysis(model, options, market).map_err(to_js_err)?)
}

/// Gordon-growth terminal value: a perpetuity growing at a stable rate.
///
/// Free-function twin of Python `TerminalValueSpec.gordon_growth` (Rust
/// `TerminalValueSpec::GordonGrowth`).
/// @param stable_growth_rate - Perpetual growth rate as a decimal (`0.02` = 2%); must be below the WACC when valued.
/// @returns Plain `TerminalValueSpec` object (`{type: "gordon_growth", stable_growth_rate}`).
///
/// # Errors
///
/// Throws with kind `invalid_type` if `stableGrowthRate` is not a number.
#[wasm_bindgen(js_name = terminalValueSpecGordonGrowth)]
pub fn terminal_value_spec_gordon_growth(stable_growth_rate: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&TerminalValueSpec::GordonGrowth {
        stable_growth_rate: js_f64(&stable_growth_rate, "stableGrowthRate")?,
    })
}

/// Exit-multiple terminal value: a terminal metric times a multiple.
///
/// Free-function twin of Python `TerminalValueSpec.exit_multiple` (Rust
/// `TerminalValueSpec::ExitMultiple`).
/// @param multiple - Exit multiple in turns (`8.5` = 8.5x the terminal metric).
/// @param terminal_metric - Terminal-year metric (for example EBITDA) in model currency; pass `0` when `DcfOptions.exit_multiple_metric_node` supplies the metric from the model.
/// @returns Plain `TerminalValueSpec` object (`{type: "exit_multiple", terminal_metric, multiple}`).
///
/// # Errors
///
/// Throws with kind `invalid_type` if an argument is not a number.
#[wasm_bindgen(js_name = terminalValueSpecExitMultiple)]
pub fn terminal_value_spec_exit_multiple(
    multiple: JsValue,
    terminal_metric: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&TerminalValueSpec::ExitMultiple {
        terminal_metric: js_f64(&terminal_metric, "terminalMetric")?,
        multiple: js_f64(&multiple, "multiple")?,
    })
}

/// H-model terminal value: growth fades linearly from a high to a stable rate.
///
/// Free-function twin of Python `TerminalValueSpec.h_model` (Rust
/// `TerminalValueSpec::HModel`).
/// @param high_growth_rate - Initial growth rate as a decimal (`0.08` = 8%).
/// @param stable_growth_rate - Long-run growth rate as a decimal; must be below the WACC when valued.
/// @param half_life_years - Half-life of the growth fade in years (`5` = growth is halfway to stable after five years).
/// @returns Plain `TerminalValueSpec` object (`{type: "h_model", ...}`).
///
/// # Errors
///
/// Throws with kind `invalid_type` if an argument is not a number.
#[wasm_bindgen(js_name = terminalValueSpecHModel)]
pub fn terminal_value_spec_h_model(
    high_growth_rate: JsValue,
    stable_growth_rate: JsValue,
    half_life_years: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&TerminalValueSpec::HModel {
        high_growth_rate: js_f64(&high_growth_rate, "highGrowthRate")?,
        stable_growth_rate: js_f64(&stable_growth_rate, "stableGrowthRate")?,
        half_life_years: js_f64(&half_life_years, "halfLifeYears")?,
    })
}

/// Variance between two evaluated scenarios of a scenario set.
///
/// Twin of Python `scenario_diff` (Rust `ScenarioSet::diff`).
/// @param scenario_set - `ScenarioSet` that produced the results (object or JSON).
/// @param results - Scenario map returned by `evaluateScenarioSet` (object or JSON).
/// @param baseline - Name of the baseline scenario.
/// @param comparison - Name of the comparison scenario.
/// @param metrics - Node identifiers to compare; at least one.
/// @param periods - Period identifiers to compare, e.g. `["2025Q1"]`; at least one.
/// @returns `ScenarioDiff`: the two scenario names and a `variance` report with one row per metric and period (absolute and fractional variance).
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `metrics` or
/// `periods` is empty, a scenario name is unknown, or a period does not
/// parse, and kind `not_found` if a metric is missing at a period in either
/// scenario.
#[wasm_bindgen(js_name = scenarioDiff)]
pub fn scenario_diff(
    scenario_set: JsValue,
    results: JsValue,
    baseline: JsValue,
    comparison: JsValue,
    metrics: JsValue,
    periods: JsValue,
) -> Result<JsValue, JsValue> {
    let scenario_set: fa::ScenarioSet = from_js_json(&scenario_set, "scenarioSet")?;
    let results: fa::ScenarioResults = from_js_json(&results, "results")?;
    let diff = scenario_set
        .diff(
            &results,
            &js_string(&baseline, "baseline")?,
            &js_string(&comparison, "comparison")?,
            &js_string_seq(&metrics, "metrics")?,
            &parse_periods(&periods)?,
        )
        .map_err(to_js_err)?;
    to_js_value(&diff)
}

/// Decompose a metric's variance between two results across named drivers.
///
/// Twin of Python `variance_bridge` (Rust
/// `VarianceAnalyzer::bridge_decomposition`). Driver contributions are raw
/// deltas in driver units, not sensitivities of the target, so they need not
/// sum to the target variance; the gap is `unexplained`.
/// @param base - Baseline evaluated `StatementResult` (object or JSON).
/// @param comparison - Comparison evaluated `StatementResult` (object or JSON).
/// @param target_metric - Node whose variance is explained.
/// @param period - Period identifier, e.g. `"2025Q4"`.
/// @param drivers - Node identifiers treated as explanatory drivers.
/// @param baseline_label - Display label of the baseline column.
/// @param comparison_label - Display label of the comparison column.
/// @returns `BridgeChart`: baseline and comparison values, ordered driver `steps` and the `unexplained` residual.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed or `period` does
/// not parse, and kind `not_found` if the target or a driver is missing from
/// either result at `period`.
#[wasm_bindgen(js_name = varianceBridge)]
#[allow(clippy::too_many_arguments)]
pub fn variance_bridge(
    base: JsValue,
    comparison: JsValue,
    target_metric: JsValue,
    period: JsValue,
    drivers: JsValue,
    baseline_label: JsValue,
    comparison_label: JsValue,
) -> Result<JsValue, JsValue> {
    let base: StatementResult = from_js_json(&base, "base")?;
    let comparison: StatementResult = from_js_json(&comparison, "comparison")?;
    let period: PeriodId = js_string(&period, "period")?.parse().map_err(to_js_err)?;
    let drivers = js_string_seq(&drivers, "drivers")?;
    let driver_refs: Vec<&str> = drivers.iter().map(String::as_str).collect();
    let chart = fa::VarianceAnalyzer::new(&base, &comparison)
        .bridge_decomposition(
            &js_string(&target_metric, "targetMetric")?,
            period,
            &driver_refs,
            &js_string(&baseline_label, "baselineLabel")?,
            &js_string(&comparison_label, "comparisonLabel")?,
        )
        .map_err(to_js_err)?;
    to_js_value(&chart)
}

/// P&L summary as a table: the requested line items across the requested periods.
///
/// Twin of Python `pl_summary_report(...).to_table()` (Rust
/// `PLSummaryReport::to_table`); `plSummaryReportText` is the formatted-text
/// twin of the same report.
/// @param results - Evaluated `StatementResult` (object or JSON).
/// @param line_items - Node identifiers to report, in row order.
/// @param periods - Period identifiers to report, in column order, e.g. `["2025Q1", "2025Q2"]`.
/// @returns `TableEnvelope` with one row per `(line item, period)` in the result's own units; a missing value is `null`.
///
/// # Errors
///
/// Throws with kind `validation` if `results` is malformed, a period does not
/// parse, or the table cannot be built.
#[wasm_bindgen(js_name = plSummaryReport)]
pub fn pl_summary_report(
    results: JsValue,
    line_items: JsValue,
    periods: JsValue,
) -> Result<JsValue, JsValue> {
    let results: StatementResult = from_js_json(&results, "results")?;
    let report = fa::PLSummaryReport::new(
        &results,
        js_string_seq(&line_items, "lineItems")?,
        parse_periods(&periods)?,
    );
    to_js_value(&report.to_table().map_err(to_js_err)?)
}

/// Look up one metric of a company by name.
///
/// Free-function twin of Python `CompanyMetrics.get` (Rust
/// `CompanyMetrics::get`): a canonical named field first, then a custom metric.
/// @param company_metrics - `CompanyMetrics` (object or JSON).
/// @param name - Canonical snake_case field (e.g. `"ebitda"`, `"leverage"`) or a key of `custom`.
/// @returns The metric value in its own units, or `undefined` when it is absent.
///
/// # Errors
///
/// Throws with kind `validation` if `companyMetrics` is malformed.
#[wasm_bindgen(js_name = companyMetricsGet)]
pub fn company_metrics_get(
    company_metrics: JsValue,
    name: JsValue,
) -> Result<Option<f64>, JsValue> {
    let metrics: fa::CompanyMetrics = from_js_json(&company_metrics, "companyMetrics")?;
    Ok(metrics.get(&js_string(&name, "name")?))
}

/// Whether a company passes a peer filter.
///
/// Free-function twin of Python `PeerFilter.accepts` (Rust
/// `PeerFilter::accepts`).
/// @param filter - `PeerFilter`: sector, industry, country, rating, tag and market-cap criteria; an empty criterion accepts every company (object or JSON).
/// @param company - `CompanyMetrics` of the candidate (object or JSON).
/// @returns `true` when the company satisfies every populated criterion.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = peerFilterAccepts)]
pub fn peer_filter_accepts(filter: JsValue, company: JsValue) -> Result<bool, JsValue> {
    let filter: fa::PeerFilter = from_js_json(&filter, "filter")?;
    let company: fa::CompanyMetrics = from_js_json(&company, "company")?;
    Ok(filter.accepts(&company))
}

/// Build a peer set by filtering a universe of companies.
///
/// Free-function twin of Python `PeerSet.from_universe` (Rust
/// `PeerSet::from_universe`). The subject is never included among the peers.
/// @param subject - `CompanyMetrics` of the company being evaluated (object or JSON).
/// @param universe - Array of candidate `CompanyMetrics` (array or JSON).
/// @param filter - `PeerFilter` applied to the universe (object or JSON).
/// @param period_basis - `"ltm"`, `"ntm"`, or `{custom: "<label>"}` for another basis such as `"FY2025E"`.
/// @returns `PeerSet` plain object: `subject`, the accepted `peers` and `period_basis`.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed or `periodBasis` is
/// not a period basis.
#[wasm_bindgen(js_name = peerSetFromUniverse)]
pub fn peer_set_from_universe(
    subject: JsValue,
    universe: JsValue,
    filter: JsValue,
    period_basis: JsValue,
) -> Result<JsValue, JsValue> {
    let subject: fa::CompanyMetrics = from_js_json(&subject, "subject")?;
    let universe: Vec<fa::CompanyMetrics> = from_js_json(&universe, "universe")?;
    let filter: fa::PeerFilter = from_js_json(&filter, "filter")?;
    let period_basis: fa::PeriodBasis = match period_basis.as_string() {
        Some(label) => finstack_quant_core::wire::serde_parse(&label).map_err(to_js_err)?,
        None => from_js_json(&period_basis, "periodBasis")?,
    };
    to_js_value(&fa::PeerSet::from_universe(
        subject,
        &universe,
        &filter,
        period_basis,
    ))
}

/// Statement result of one scenario.
///
/// Free-function twin of Python `ScenarioResults.get`.
/// @param results - Scenario map returned by `evaluateScenarioSet` (object or JSON).
/// @param name - Name of the scenario to read, as keyed in the scenario set (e.g. `"base"`).
/// @returns The scenario's `StatementResult` plain object, or `undefined` when no scenario has that name.
///
/// # Errors
///
/// Throws with kind `validation` if `results` is malformed.
#[wasm_bindgen(js_name = scenarioResultsGet)]
pub fn scenario_results_get(results: JsValue, name: JsValue) -> Result<JsValue, JsValue> {
    let results: fa::ScenarioResults = from_js_json(&results, "results")?;
    match results.scenarios.get(&js_string(&name, "name")?) {
        Some(result) => to_js_value(result),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Inheritance lineage of a scenario, from the root parent to the scenario itself.
///
/// Free-function twin of Python `ScenarioSet.trace` (Rust `ScenarioSet::trace`).
/// @param scenario_set - `ScenarioSet` (object or JSON).
/// @param scenario - Name of the scenario to trace.
/// @returns Scenario names from the outermost ancestor down to `scenario`.
///
/// # Errors
///
/// Throws with kind `validation` if `scenarioSet` is malformed, the scenario
/// or one of its parents is unknown, or the parent chain is cyclic.
#[wasm_bindgen(js_name = scenarioSetTrace)]
pub fn scenario_set_trace(scenario_set: JsValue, scenario: JsValue) -> Result<JsValue, JsValue> {
    let scenario_set: fa::ScenarioSet = from_js_json(&scenario_set, "scenarioSet")?;
    to_js_value(
        &scenario_set
            .trace(&js_string(&scenario, "scenario")?)
            .map_err(to_js_err)?,
    )
}

/// Copy of a sensitivity configuration with one more parameter.
///
/// Free-function twin of Python `SensitivityConfig.add_parameter` (Rust
/// `SensitivityConfig::add_parameter`). Python builds the `ParameterSpec`
/// from keyword arguments; here it is passed as an object, e.g. from
/// `parameterSpecWithPercentages`.
/// @param config - `SensitivityConfig` (object or JSON).
/// @param parameter - `ParameterSpec`: `node_id`, `period_id`, `base_value` and absolute `perturbations` (object or JSON).
/// @returns New plain `SensitivityConfig` object.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed.
#[wasm_bindgen(js_name = sensitivityConfigAddParameter)]
pub fn sensitivity_config_add_parameter(
    config: JsValue,
    parameter: JsValue,
) -> Result<JsValue, JsValue> {
    let mut config: fa::SensitivityConfig = from_js_json(&config, "config")?;
    config.add_parameter(from_js_json(&parameter, "parameter")?);
    to_js_value(&config)
}

/// Value a parameter took in one sensitivity scenario.
///
/// Free-function twin of Python `SensitivityResult.get_parameter_value` (Rust
/// `SensitivityResult::get_parameter_value`).
/// @param result - `SensitivityResult` returned by `runSensitivity` (object or JSON).
/// @param scenario_index - Zero-based position of the scenario in `scenarios`.
/// @param parameter - Parameter key as `node_id@period_id`, e.g. `"revenue@2025Q1"`.
/// @returns The perturbed value in the node's own units, or `undefined` when the scenario did not vary that parameter.
///
/// # Errors
///
/// Throws with kind `validation` if `result` is malformed or `scenarioIndex`
/// is out of range, and kind `invalid_type` if `scenarioIndex` is not a
/// non-negative integer.
#[wasm_bindgen(js_name = sensitivityResultGetParameterValue)]
pub fn sensitivity_result_get_parameter_value(
    result: JsValue,
    scenario_index: JsValue,
    parameter: JsValue,
) -> Result<Option<f64>, JsValue> {
    let result: fa::SensitivityResult = from_js_json(&result, "result")?;
    result
        .get_parameter_value(
            js_uint(&scenario_index, "scenarioIndex")?,
            &js_string(&parameter, "parameter")?,
        )
        .map_err(to_js_err)
}

/// Evaluated node value in one sensitivity scenario.
///
/// Free-function twin of Python `SensitivityResult.get_value` (Rust
/// `SensitivityResult::get_value`).
/// @param result - `SensitivityResult` returned by `runSensitivity` (object or JSON).
/// @param scenario_index - Zero-based position of the scenario in `scenarios`.
/// @param node_id - Node identifier to read.
/// @param period - Period identifier, e.g. `"2025Q1"`.
/// @returns The node value in its own units, or `undefined` when the node or period is absent from the scenario.
///
/// # Errors
///
/// Throws with kind `validation` if `result` is malformed, `period` does not
/// parse or `scenarioIndex` is out of range, and kind `invalid_type` if
/// `scenarioIndex` is not a non-negative integer.
#[wasm_bindgen(js_name = sensitivityResultGetValue)]
pub fn sensitivity_result_get_value(
    result: JsValue,
    scenario_index: JsValue,
    node_id: JsValue,
    period: JsValue,
) -> Result<Option<f64>, JsValue> {
    let result: fa::SensitivityResult = from_js_json(&result, "result")?;
    let period: PeriodId = js_string(&period, "period")?.parse().map_err(to_js_err)?;
    result
        .get_value(
            js_uint(&scenario_index, "scenarioIndex")?,
            &js_string(&node_id, "nodeId")?,
            &period,
        )
        .map_err(to_js_err)
}

/// Detailed text rendering of a formula explanation.
///
/// Free-function twin of Python `Explanation.to_text` (Rust
/// `Explanation::to_string_detailed`).
/// @param explanation - `Explanation` returned by `explainFormula`. Pass the JSON form (non-finite values as `"nan"`, `"inf"`, `"-inf"`) when a value is not finite; a plain object holding `NaN` is rejected at the boundary.
/// @returns Multi-line text: the node, period, final value, formula and each component with its value.
///
/// # Errors
///
/// Throws with kind `validation` if `explanation` is malformed.
#[wasm_bindgen(js_name = explanationToText)]
pub fn explanation_to_text(explanation: JsValue) -> Result<String, JsValue> {
    let explanation: fa::Explanation = from_js_json(&explanation, "explanation")?;
    Ok(explanation.to_string_detailed())
}

/// One-line summary of forecast accuracy metrics.
///
/// Free-function twin of Python `ForecastMetrics.summary` (Rust
/// `ForecastMetrics::summary`).
/// @param metrics - `ForecastMetrics` returned by `backtestForecast`. Pass the JSON form (non-finite values as `"nan"`) when MAPE is undefined; a plain object holding `NaN` is rejected at the boundary.
/// @returns Text such as `"MAE: 2.00, MAPE: 1.91% (eff_n=2), sMAPE: 1.91%, RMSE: 2.00 (n=2)"`.
///
/// # Errors
///
/// Throws with kind `validation` if `metrics` is malformed.
#[wasm_bindgen(js_name = forecastMetricsSummaryText)]
pub fn forecast_metrics_summary_text(metrics: JsValue) -> Result<String, JsValue> {
    let metrics: fa::ForecastMetrics = from_js_json(&metrics, "metrics")?;
    Ok(metrics.summary())
}
