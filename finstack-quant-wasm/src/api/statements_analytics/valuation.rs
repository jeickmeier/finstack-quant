//! DCF, WACC, goal-seek, formula explanation, and report WASM entry points.

use crate::utils::input::{
    from_js_json, js_f64, js_opt_f64, js_opt_string, js_string, js_string_seq, json_text,
    opt_json_text,
};
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

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

    let model = crate::api::statements::load_model_text(model_json)?;
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

/// Compute weighted-average cost of capital (WACC) using exact capital weights.
///
/// The costs of equity and debt are annual effective rates above -100%. The
/// debt cost is multiplied by `1 - tax_rate` before weighting.
/// @param inputs - Capital weights as exact decimal strings summing to one, annual effective equity and pre-tax debt costs as decimal numbers, and a marginal tax rate in [0, 1].
/// @returns The finite blended annual effective rate as a decimal.
/// @throws TypeError if the object contains a non-JSON value. FinstackError for malformed structured inputs, inexact capital allocation, invalid effective rates or tax domain, or unrepresentable output.
#[wasm_bindgen(js_name = wacc)]
pub fn wacc(
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.WaccInputs | string")]
    inputs: JsValue,
) -> Result<f64, JsValue> {
    let inputs: finstack_quant_statements_analytics::analysis::WaccInputs =
        from_js_json(&inputs, "inputs")?;
    finstack_quant_statements_analytics::analysis::wacc(&inputs).map_err(to_js_err)
}

/// Validate the elected dates, target units and bounded root-search controls.
/// @param spec - Complete goal-seek assumptions, including the driver, target and both absolute tolerances.
/// @returns The canonical request with the dates and numerical search controls checked.
/// @throws Validation error if the request fields or bounds are invalid.
#[wasm_bindgen(js_name = goalSeekSpec, unchecked_return_type = "generated.statements_analytics.GoalSeekSpec")]
pub fn goal_seek_spec(
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.GoalSeekSpec | string")]
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let spec: finstack_quant_statements_analytics::analysis::GoalSeekSpec =
        from_js_json(&spec, "spec")?;
    spec.validate().map_err(to_js_err)?;
    to_js_value(&spec)
}

/// Find and verify a target at a fixed publication cutoff and market pricing date.
/// @param model - Original statement model with the elected editable forecast assumption.
/// @param spec - Explicit driver and target periods, typed amount, tolerances and search policy.
/// @param market - Held market data for linked financing. Omit for an empty market.
/// @returns Typed solved driver and target amounts, absolute tolerance, verified run
///   evidence, and a verified model copy when requested.
/// @throws If the selected observations, units, dates or root search are inadmissible,
///   or if the verified objective misses the elected target tolerance.
#[wasm_bindgen(js_name = goalSeek, unchecked_return_type = "GoalSeekResult")]
pub fn goal_seek(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] model: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.GoalSeekSpec | string")]
    spec: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "JsonInput | null")] market: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let model = crate::api::statements::load_model(&model, "model")?;
    let spec = from_js_json(&spec, "spec")?;
    let market = crate::utils::wire::js_opt_wire(market.as_ref(), "market")?.unwrap_or_default();
    let result = finstack_quant_statements_analytics::analysis::goal_seek(&model, &spec, &market)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Explain a formula for a specific node and period (JSON in, structured
/// object out).
///
/// Returns the Rust `Explanation` as a structured object: `source` names the
/// value source selected for the cell (`value`, `forecast` or `formula`; absent
/// for a masked or missing cell), `forecast` carries the node's forecast spec for a
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
    let model = crate::api::statements::load_model_text(model_json)?;
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
    let model = crate::api::statements::load_model_text(model_json)?;
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
