#![allow(clippy::result_large_err)]

//! Calibration execution, validation, dry-run, hashing, and error conversion.

use crate::utils::input::{js_string, json_text};
#[cfg(target_arch = "wasm32")]
use crate::utils::structured_js_error;
use crate::utils::to_js_value;
use finstack_quant_calibration::api::engine::{self, ExecuteError};
use finstack_quant_calibration::api::schema::CalibrationResultEnvelope;
use finstack_quant_calibration::api::validate;
use wasm_bindgen::prelude::*;

/// Validate a calibration envelope and return it as a plain `CalibrationEnvelope` object.
///
/// Applies strict loading and fail-fast static validation.
/// @param envelope - `CalibrationEnvelope` (object or JSON) with schema marker `finstack_quant.calibration/1`.
/// @returns The validated `CalibrationEnvelope` in canonical form.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed, its
/// schema marker is missing or unsupported, or static validation fails
/// (fail-fast: first error; `dryRun` lists every static error). Throws a
/// `FinstackError` if a returned integer exceeds JavaScript’s safe integer range.
#[wasm_bindgen(js_name = validateCalibration, unchecked_return_type = "CalibrationEnvelope")]
pub fn validate_calibration(
    #[wasm_bindgen(unchecked_param_type = "CalibrationEnvelope | string")] envelope: JsValue,
) -> Result<JsValue, JsValue> {
    let envelope: &str = &json_text(&envelope, "envelope")?;
    let parsed =
        validate::parse_envelope(envelope).map_err(|err| execute_error_to_js(err.into()))?;
    validate::validate_calibration(&parsed).map_err(|err| execute_error_to_js(err.into()))?;
    to_js_value(&parsed)
}

/// Execute a calibration plan and return the full result envelope.
///
/// Accepts a serialized `CalibrationEnvelope` (plan + quote sets + optional
/// flat `market_data` / `prior_market` lists) and returns a plain JavaScript
/// `CalibrationResultEnvelope` object, the declared return type. Re-ingest a sub-document (e.g. `result.result.final_market`) with
/// `JSON.stringify`.
/// @param envelopeJson - `CalibrationEnvelope` as a plain object or its JSON string: the plan (steps, quote sets, settings) plus `market_data` and optional `prior_market`.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed or violates
/// the calibration schema or static plan contract (fail-fast: first static
/// error; `dryRun` lists every static error), solver configuration is invalid,
/// market context construction or a calibration step fails, a solver does
/// not converge, or the result envelope cannot be converted to a JavaScript value.
/// @returns Calibration result including the materialized market and per-step reports.
#[wasm_bindgen(js_name = calibrate, unchecked_return_type = "CalibrationResultEnvelope")]
pub fn calibrate(
    #[wasm_bindgen(
        js_name = "envelopeJson",
        unchecked_param_type = "CalibrationEnvelope | string"
    )]
    envelope_json: JsValue,
) -> Result<JsValue, JsValue> {
    let envelope_json: &str = &json_text(&envelope_json, "envelopeJson")?;
    let result = engine::calibrate_from_json(envelope_json).map_err(execute_error_to_js)?;
    to_js_value(&result)
}

/// Strict-load a calibration result envelope (default load limits).
pub(super) fn load_result(json: &str) -> Result<CalibrationResultEnvelope, ExecuteError> {
    CalibrationResultEnvelope::from_slice_strict(
        json.as_bytes(),
        &finstack_quant_core::contract::LoadLimits::default(),
    )
    .map(|(result, _report)| result)
    .map_err(|error| {
        ExecuteError::from(
            finstack_quant_calibration::api::errors::EnvelopeError::strict_load(&error),
        )
    })
}

/// Canonical content hash of a calibration envelope.
///
/// Free-function twin of Python `CalibrationEnvelope.content_hash` (Rust
/// `CalibrationEnvelope::content_hash`): the envelope is strict-loaded through
/// the same Rust parser `calibrate` uses, then hashed over its canonical JSON,
/// so the hash does not depend on key order or number spelling.
/// @param envelopeJson - `CalibrationEnvelope` (object or JSON).
/// @returns `"sha256:<hex>"` content hash.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed, its
/// schema marker is missing or unsupported, or it contains a non-finite
/// number.
#[wasm_bindgen(js_name = calibrationEnvelopeContentHash)]
pub fn calibration_envelope_content_hash(
    #[wasm_bindgen(
        js_name = "envelopeJson",
        unchecked_param_type = "CalibrationEnvelope | string"
    )]
    envelope_json: JsValue,
) -> Result<String, JsValue> {
    let envelope_json: &str = &json_text(&envelope_json, "envelopeJson")?;
    let envelope =
        validate::parse_envelope(envelope_json).map_err(|err| execute_error_to_js(err.into()))?;
    envelope
        .content_hash()
        .map_err(|err| execute_error_to_js(err.into()))
}

/// Canonical content hash of a calibration result envelope.
///
/// Free-function twin of Python `CalibrationResultEnvelope.content_hash` (Rust
/// `CalibrationResultEnvelope::content_hash`): the result is strict-loaded
/// (default load limits), then hashed over its canonical JSON.
/// @param resultJson - `CalibrationResultEnvelope` returned by `calibrate` (object or JSON).
/// @returns `"sha256:<hex>"` content hash.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the result is malformed, exceeds the
/// default load limits, its schema marker is missing or unsupported, or it
/// contains a non-finite number.
#[wasm_bindgen(js_name = calibrationResultContentHash)]
pub fn calibration_result_content_hash(
    #[wasm_bindgen(
        js_name = "resultJson",
        unchecked_param_type = "CalibrationResultEnvelope | string"
    )]
    result_json: JsValue,
) -> Result<String, JsValue> {
    let result_json: &str = &json_text(&result_json, "resultJson")?;
    let result = load_result(result_json).map_err(execute_error_to_js)?;
    result
        .content_hash()
        .map_err(|err| execute_error_to_js(err.into()))
}

/// Pre-flight envelope validation without invoking the solver.
///
/// Returns the `CalibrationValidationReport` as a plain object listing every
/// static error found (`errors`) plus the dependency graph
/// (`dependency_graph`). Serialize the returned object when JSON text is needed.
/// @param envelopeJson - `CalibrationEnvelope` as a plain object or its JSON string: the plan (steps, quote sets, settings) plus `market_data` and optional `prior_market`.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed, its schema
/// marker is missing, malformed, or unsupported, the envelope structure is
/// invalid, or the validation report cannot be converted to a JavaScript
/// value. Semantic findings are returned in the report rather than thrown.
/// @returns `CalibrationValidationReport` object containing every static error and the dependency graph.
#[wasm_bindgen(js_name = dryRun, unchecked_return_type = "CalibrationValidationReport")]
pub fn dry_run(
    #[wasm_bindgen(
        js_name = "envelopeJson",
        unchecked_param_type = "CalibrationEnvelope | string"
    )]
    envelope_json: JsValue,
) -> Result<JsValue, JsValue> {
    let envelope_json: &str = &json_text(&envelope_json, "envelopeJson")?;
    let report =
        validate::dry_run(envelope_json).map_err(|error| execute_error_to_js(error.into()))?;
    to_js_value(&report)
}

/// Calibrate the explicit Bermudan LMM loading scale from the market surface.
///
/// Callers must place the returned value in `modelConfig.lmmBaseVol` before
/// pricing; the Bermudan pricer never reads or fits the surface itself.
/// @param instrumentJson - Canonical Bermudan swaption instrument envelope, as a plain object or its JSON string.
/// @param market - Reusable market handle containing discount and swaption-volatility inputs.
/// @param asOf - ISO-8601 valuation date.
/// @returns Positive finite LMM base volatility.
///
/// # Errors
///
/// The JavaScript facade rejects object inputs containing non-finite numbers
/// with `TypeError` before serialization. Explicit JSON strings are parsed in Rust.
///
/// Throws if the envelope is not a Bermudan swaption, the date or market
/// inputs are invalid, or the Rebonato calibration cannot be completed.
#[wasm_bindgen(js_name = calibrateBermudanLmmBaseVol)]
pub fn calibrate_bermudan_lmm_base_vol(
    #[wasm_bindgen(
        js_name = "instrumentJson",
        unchecked_param_type = "Record<string, unknown> | string"
    )]
    instrument_json: JsValue,
    market: &crate::api::core::market_context::JsMarketContext,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
) -> Result<f64, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let as_of = crate::utils::parse_iso_date(as_of)?;
    finstack_quant_calibration::calibrate_bermudan_lmm_base_vol_from_json(
        instrument_json,
        market.inner(),
        as_of,
    )
    .map_err(crate::utils::to_js_err)
}

/// Map every execution stage to the same structured JavaScript error contract:
/// a `CalibrationEnvelopeError` carrying the structured Rust error payload.
pub(super) fn execute_error_to_js(err: ExecuteError) -> JsValue {
    let details = err.details();
    #[cfg(target_arch = "wasm32")]
    {
        let details_json = details.to_json();
        let error = structured_js_error(
            "CalibrationEnvelopeError",
            &details.cause,
            Some(&details.category),
            Some(&details_json),
        );
        let solver_diagnostics = match details.solver_diagnostics.as_ref() {
            Some(diagnostics) => match to_js_value(diagnostics) {
                Ok(value) => value,
                Err(serialization_error) => return serialization_error,
            },
            None => JsValue::UNDEFINED,
        };
        let diagnostics = match to_js_value(&details.diagnostics) {
            Ok(value) => value,
            Err(serialization_error) => return serialization_error,
        };
        let step_id = details
            .step_id
            .as_deref()
            .map_or(JsValue::UNDEFINED, JsValue::from);
        for (name, value) in [
            ("stage", JsValue::from(details.stage.as_str())),
            ("step_id", step_id),
            ("solver_diagnostics", solver_diagnostics),
            ("details", JsValue::from(&details_json)),
            ("diagnostics", diagnostics),
        ] {
            let _ = js_sys::Reflect::set(&error, &JsValue::from(name), &value);
        }
        error
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = details;
        JsValue::UNDEFINED
    }
}
