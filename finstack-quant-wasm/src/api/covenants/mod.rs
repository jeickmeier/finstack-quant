//! WASM bindings for the `finstack-quant-covenants` crate.
//!
//! This module holds the JSON wire surface (validators, `evaluateEngine`, the
//! `*Json` templates). The typed surface lives in the submodules: the
//! `CovenantEngine` handle, forecasting, and free functions over wire-shaped
//! covenant definitions.

pub mod engine;
pub mod forecast;
pub mod spec;

use crate::utils::input::{js_string, json_text};
use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Validate and canonicalize a covenant spec JSON string.
/// @param specJson - JSON-serialized covenant specification to validate.
///
/// # Errors
///
/// Throws a JavaScript exception if `specJson` is malformed, does not match the
/// covenant-spec schema, violates covenant threshold or frequency invariants, or
/// cannot be serialized to canonical JSON.
/// @returns Canonical covenant-spec JSON after schema validation.
#[wasm_bindgen(js_name = validateCovenantSpecJson)]
pub fn validate_covenant_spec_json(
    #[wasm_bindgen(js_name = "specJson", unchecked_param_type = "JsonInput")] spec_json: JsValue,
) -> Result<String, JsValue> {
    let spec_json: &str = &json_text(&spec_json, "specJson")?;
    finstack_quant_covenants::validate_covenant_spec_json(spec_json).map_err(to_js_err)
}

/// Validate and canonicalize a covenant report JSON string.
/// @param reportJson - JSON-serialized covenant evaluation report to validate.
///
/// # Errors
///
/// Throws a JavaScript exception if `reportJson` is malformed, does not match the
/// covenant-report schema, or cannot be serialized to canonical JSON.
/// @returns Canonical covenant-report JSON after schema validation.
#[wasm_bindgen(js_name = validateCovenantReportJson)]
pub fn validate_covenant_report_json(
    #[wasm_bindgen(js_name = "reportJson", unchecked_param_type = "JsonInput")]
    report_json: JsValue,
) -> Result<String, JsValue> {
    let report_json: &str = &json_text(&report_json, "reportJson")?;
    finstack_quant_covenants::validate_covenant_report_json(report_json).map_err(to_js_err)
}

/// Validate and canonicalize a covenant engine JSON string.
/// @param engineJson - JSON-serialized covenant engine and its covenant definitions.
///
/// # Errors
///
/// Throws a JavaScript exception if `engineJson` is malformed, does not match the
/// covenant-engine schema, contains an invalid covenant package, violates engine
/// invariants, or cannot be serialized to canonical JSON.
/// @returns Canonical covenant-engine JSON after schema validation.
#[wasm_bindgen(js_name = validateCovenantEngineJson)]
pub fn validate_covenant_engine_json(
    #[wasm_bindgen(js_name = "engineJson", unchecked_param_type = "JsonInput")]
    engine_json: JsValue,
) -> Result<String, JsValue> {
    let engine_json: &str = &json_text(&engine_json, "engineJson")?;
    finstack_quant_covenants::validate_covenant_engine_json(engine_json).map_err(to_js_err)
}

/// Evaluate a covenant engine JSON string against a JSON metric map.
///
/// Returns a plain JavaScript object keyed by the engine's stable covenant
/// instance key, each value a covenant report carrying `covenant_type`,
/// `covenant_id`, `passed`, `actual_value`, `threshold`, `details`,
/// `headroom`, and `meta`. This mirrors the Python binding, which returns
/// `dict[str, CovenantReport]` from the same Rust entry point.
///
/// @param engineJson - JSON-serialized covenant engine and its covenant definitions.
/// @param metricsJson - JSON object of financial metrics referenced by the covenant engine.
/// @param asOf - ISO-8601 covenant test date; selects the active covenant window, applicable waivers, threshold-schedule step and cure-period state.
///
/// # Errors
///
/// Throws a JavaScript exception if either JSON input is malformed or has the
/// wrong schema, a metric is non-numeric, `asOf` is not a valid ISO date, the
/// engine or required metrics fail validation, or the reports cannot be
/// serialized to JavaScript.
/// @returns A plain object keyed by covenant instance key, each value a `CovenantReport`.
#[wasm_bindgen(js_name = evaluateEngine, unchecked_return_type = "Record<string, CovenantReport>")]
pub fn evaluate_engine(
    #[wasm_bindgen(js_name = "engineJson", unchecked_param_type = "JsonInput")]
    engine_json: JsValue,
    #[wasm_bindgen(js_name = "metricsJson", unchecked_param_type = "JsonInput")]
    metrics_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
) -> Result<JsValue, JsValue> {
    let engine_json: &str = &json_text(&engine_json, "engineJson")?;
    let metrics_json: &str = &json_text(&metrics_json, "metricsJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let reports = finstack_quant_covenants::evaluate_engine(engine_json, metrics_json, as_of)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&reports)
}
