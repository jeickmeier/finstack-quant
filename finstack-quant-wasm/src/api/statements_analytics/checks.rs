//! Financial statement check execution and report rendering WASM entry points.

use crate::utils::input::{json_text, opt_json_text};
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

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
/// @param resultsJson - Evaluated statement-result JSON, or a rollup of it (`statementResultRollup`), which is checked at its target periods.
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
    let model = crate::api::statements::load_model_text(model_json)?;
    let spec: finstack_quant_statements::checks::CheckSuiteSpec =
        serde_json::from_str(suite_spec_json).map_err(to_js_err)?;
    let suite = spec.resolve().map_err(to_js_err)?;
    let results = parse_optional_results(results_json)?;
    let report = suite
        .run_model(
            &model,
            &finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
            results.as_ref(),
        )
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
    let model = crate::api::statements::load_model_text(model_json)?;
    let mapping: finstack_quant_statements_analytics::analysis::ThreeStatementMapping =
        serde_json::from_str(mapping_json).map_err(to_js_err)?;
    let suite = finstack_quant_statements_analytics::analysis::three_statement_checks(mapping)
        .map_err(to_js_err)?;
    let results = parse_optional_results(results_json)?;
    let report = suite
        .run_model(
            &model,
            &finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
            results.as_ref(),
        )
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
    let model = crate::api::statements::load_model_text(model_json)?;
    let mapping: finstack_quant_statements_analytics::analysis::CreditMapping =
        serde_json::from_str(mapping_json).map_err(to_js_err)?;
    let suite = finstack_quant_statements_analytics::analysis::credit_underwriting_checks(mapping)
        .map_err(to_js_err)?;
    let results = parse_optional_results(results_json)?;
    let report = suite
        .run_model(
            &model,
            &finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
            results.as_ref(),
        )
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
