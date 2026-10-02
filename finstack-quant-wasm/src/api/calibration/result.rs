//! Free-function twins of the Python `CalibrationResult` methods.
//!
//! WASM calibration results are plain `CalibrationResultEnvelope` objects, so
//! the Rust `CalibrationResult::step_report` and `CalibrationReport::quote_rows`
//! methods are exposed as functions taking that object. Each call strict-loads
//! the result (default load limits), exactly as `calibrationResultContentHash`
//! does.

use super::{execute_error_to_js, load_result};
use crate::utils::input::{js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Report of one calibration step, read from a calibration result.
///
/// Free-function twin of Python `CalibrationResult.step_report` (Rust
/// `CalibrationResult::step_report`).
/// @param result_json - `CalibrationResultEnvelope` returned by `calibrate` (object or JSON).
/// @param step_id - Identifier of the calibration step, as given in the plan.
/// @returns The step's `CalibrationReport`, with raw residuals keyed by quote id.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the result is malformed or exceeds
/// the default load limits, and a `FinstackError` with `kind: "not_found"`
/// naming the available step ids if no step has the given `stepId`.
#[wasm_bindgen(js_name = calibrationResultStepReport)]
pub fn calibration_result_step_report(
    result_json: JsValue,
    step_id: JsValue,
) -> Result<JsValue, JsValue> {
    let result_json: &str = &json_text(&result_json, "resultJson")?;
    let step_id: &str = &js_string(&step_id, "stepId")?;
    let result = load_result(result_json).map_err(execute_error_to_js)?;
    let report = result.result.step_report(step_id).map_err(to_js_err)?;
    to_js_value(report)
}

/// Report of one calibration step as a compact JSON string.
///
/// JSON wire twin of [`calibration_result_step_report`] and free-function twin
/// of Python `CalibrationResult.step_report_json`.
/// @param result_json - `CalibrationResultEnvelope` returned by `calibrate` (object or JSON).
/// @param step_id - Identifier of the calibration step, as given in the plan.
/// @returns Compact `CalibrationReport` JSON.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the result is malformed or exceeds
/// the default load limits, and a `FinstackError` with `kind: "not_found"` if
/// no step has the given `stepId` or with `kind: "validation"` if the report
/// cannot be serialized.
#[wasm_bindgen(js_name = calibrationResultStepReportJson)]
pub fn calibration_result_step_report_json(
    result_json: JsValue,
    step_id: JsValue,
) -> Result<String, JsValue> {
    let result_json: &str = &json_text(&result_json, "resultJson")?;
    let step_id: &str = &js_string(&step_id, "stepId")?;
    let result = load_result(result_json).map_err(execute_error_to_js)?;
    let report = result.result.step_report(step_id).map_err(to_js_err)?;
    serde_json::to_string(report).map_err(to_js_err)
}

/// Per-quote residual rows of one calibration step.
///
/// Free-function twin of Python `CalibrationResult.residuals`, which returns
/// the same rows (Rust `CalibrationReport::quote_rows`) as a pandas
/// `DataFrame`. `target_value`, `fitted_value` and `sensitivity` are `NaN`
/// unless `CalibrationConfig.compute_diagnostics` was enabled.
/// @param result_json - `CalibrationResultEnvelope` returned by `calibrate` (object or JSON).
/// @param step_id - Identifier of the calibration step, as given in the plan.
/// @returns One `QuoteQuality` row per quote: `quote_label`, `target_value`, `fitted_value`, `residual` (fitted minus target, in the quote's native units) and `sensitivity`.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the result is malformed or exceeds
/// the default load limits, and a `FinstackError` with `kind: "not_found"`
/// naming the available step ids if no step has the given `stepId`.
#[wasm_bindgen(js_name = calibrationResultResiduals)]
pub fn calibration_result_residuals(
    result_json: JsValue,
    step_id: JsValue,
) -> Result<JsValue, JsValue> {
    let result_json: &str = &json_text(&result_json, "resultJson")?;
    let step_id: &str = &js_string(&step_id, "stepId")?;
    let result = load_result(result_json).map_err(execute_error_to_js)?;
    let rows = result
        .result
        .step_report(step_id)
        .map_err(to_js_err)?
        .quote_rows();
    to_js_value(&rows)
}
