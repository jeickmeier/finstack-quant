//! WASM bindings for the calibration engine.
//!
//! Mirrors the Python `calibrate` / `validate_calibration_json` surface plus
//! diagnostics (`dryRun` and its `dryRunJson` wire twin).
//!
//! # Number safety
//!
//! `calibrate` and `dryRun` return structured objects built by
//! `crate::utils::to_js_value`, so counts (`iterations`, `residual_evals`,
//! `lm_jacobian_evals`) arrive as JS numbers. That serializer refuses to emit
//! an integer above `Number.MAX_SAFE_INTEGER` (2^53 − 1) instead of rounding
//! it, so there is no silent loss on this path. Only the JSON-string surfaces
//! (`validateCalibrationJson`, `dryRunJson`) hand the caller text whose
//! integers `JSON.parse` would round above that limit; iteration counts stay
//! under ~10⁴ for any non-pathological calibration.
//!
//! On error, the host functions throw a JS `Error` with `name =
//! "CalibrationEnvelopeError"`. The error exposes `kind` (the execution
//! category), `stage`, `step_id`, `solver_diagnostics` (an object), the
//! strict-load `diagnostics` array (empty unless ingestion rejected the
//! document), JSON-string `details`, and a structured `cause` object. Absent
//! optional properties are `undefined`.
//!
//! # Submodules
//!
//! - `quotes`, `steps`: free-function twins of the Python quote, rate-bound
//!   and step constructors, returning the plain serde objects (`fields` holds
//!   their shared argument marshalling).
//! - `hull_white`: direct Hull-White one-factor calibrators on curve handles.
//! - `result`: free-function twins of the Python `CalibrationResult` methods.

// `EnvelopeError` / `ExecuteError` are intentionally large structured errors
// (rich diagnostic payloads); boxing them would change their public API.
// The upstream `calibration::api::{engine, validate}` modules make the same
// allowance — keep the binding layer consistent.
#![allow(clippy::result_large_err)]

mod fields;
mod hull_white;
mod quotes;
mod result;
mod steps;
mod validation;

use crate::utils::input::{js_string, json_text};
#[cfg(target_arch = "wasm32")]
use crate::utils::structured_js_error;
use crate::utils::to_js_value;
use finstack_quant_calibration::api::engine::{self, ExecuteError};
use finstack_quant_calibration::api::schema::CalibrationResultEnvelope;
use finstack_quant_calibration::api::validate;
use wasm_bindgen::prelude::*;

/// Validate a calibration plan JSON and return the canonical (pretty-printed) form.
/// @param json - `CalibrationEnvelope` as a plain object or its JSON string.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed, its calibration
/// schema marker is missing, malformed, or unsupported, static envelope
/// validation fails (fail-fast: first error; `dryRun` lists every static
/// error), or the canonical envelope cannot be serialized.
#[wasm_bindgen(js_name = validateCalibrationJson)]
pub fn validate_calibration_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    validate::validate_calibration_json(json).map_err(|err| execute_error_to_js(err.into()))
}

/// Validate a calibration envelope and return it as a plain `CalibrationEnvelope` object.
///
/// Typed twin of [`validate_calibration_json`]: the same strict load and
/// fail-fast static validation, returning the envelope object instead of its
/// pretty-printed JSON.
/// @param envelope - `CalibrationEnvelope` (object or JSON) with schema marker `finstack_quant.calibration/1`.
/// @returns The validated `CalibrationEnvelope` in canonical form.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed, its
/// schema marker is missing or unsupported, or static validation fails
/// (fail-fast: first error; `dryRun` lists every static error).
#[wasm_bindgen(js_name = validateCalibration)]
pub fn validate_calibration(envelope: JsValue) -> Result<JsValue, JsValue> {
    let envelope: &str = &json_text(&envelope, "envelope")?;
    let parsed =
        validate::parse_envelope(envelope).map_err(|err| execute_error_to_js(err.into()))?;
    validate::validate_fail_fast(&parsed).map_err(|err| execute_error_to_js(err.into()))?;
    to_js_value(&parsed)
}

/// Execute a calibration plan and return the full result envelope.
///
/// Accepts a serialized `CalibrationEnvelope` (plan + quote sets + optional
/// flat `market_data` / `prior_market` lists) and returns a plain JavaScript
/// `CalibrationResultEnvelope` object — the shape `index.d.ts` has always
/// declared. Re-ingest a sub-document (e.g. `result.result.final_market`) with
/// `JSON.stringify`.
/// @param envelope_json - `CalibrationEnvelope` as a plain object or its JSON string: the plan (steps, quote sets, settings) plus `market_data` and optional `prior_market`.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed or violates
/// the calibration schema or static plan contract (fail-fast: first static
/// error; `dryRun` lists every static error), solver configuration is invalid,
/// market context construction or a calibration step fails, a solver does
/// not converge, or the result envelope cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = calibrate)]
pub fn calibrate(envelope_json: JsValue) -> Result<JsValue, JsValue> {
    let envelope_json: &str = &json_text(&envelope_json, "envelopeJson")?;
    let result = engine::calibrate_from_json(envelope_json).map_err(execute_error_to_js)?;
    to_js_value(&result)
}

/// Strict-load a calibration result envelope (default load limits).
fn load_result(json: &str) -> Result<CalibrationResultEnvelope, ExecuteError> {
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
/// @param envelope_json - `CalibrationEnvelope` (object or JSON).
/// @returns `"sha256:<hex>"` content hash.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed, its
/// schema marker is missing or unsupported, or it contains a non-finite
/// number.
#[wasm_bindgen(js_name = calibrationEnvelopeContentHash)]
pub fn calibration_envelope_content_hash(envelope_json: JsValue) -> Result<String, JsValue> {
    let envelope_json: &str = &json_text(&envelope_json, "envelopeJson")?;
    let envelope =
        validate::parse_envelope(envelope_json).map_err(|err| execute_error_to_js(err.into()))?;
    envelope
        .content_hash()
        .map_err(|err| execute_error_to_js(err.into()))
}

/// Canonical content hash of a calibration result envelope.
///
/// Free-function twin of Python `CalibrationResult.content_hash` (Rust
/// `CalibrationResultEnvelope::content_hash`): the result is strict-loaded
/// (default load limits), then hashed over its canonical JSON.
/// @param result_json - `CalibrationResultEnvelope` returned by `calibrate` (object or JSON).
/// @returns `"sha256:<hex>"` content hash.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the result is malformed, exceeds the
/// default load limits, its schema marker is missing or unsupported, or it
/// contains a non-finite number.
#[wasm_bindgen(js_name = calibrationResultContentHash)]
pub fn calibration_result_content_hash(result_json: JsValue) -> Result<String, JsValue> {
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
/// (`dependency_graph`). Microseconds. Use [`dry_run_json`] for the JSON
/// wire string.
/// @param envelope_json - `CalibrationEnvelope` as a plain object or its JSON string: the plan (steps, quote sets, settings) plus `market_data` and optional `prior_market`.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed, its schema
/// marker is missing, malformed, or unsupported, the envelope structure is
/// invalid, or the validation report cannot be converted to a JavaScript
/// value. Semantic findings are returned in the report rather than thrown.
#[wasm_bindgen(js_name = dryRun)]
pub fn dry_run(envelope_json: JsValue) -> Result<JsValue, JsValue> {
    let envelope_json: &str = &json_text(&envelope_json, "envelopeJson")?;
    let report =
        validate::dry_run(envelope_json).map_err(|error| execute_error_to_js(error.into()))?;
    to_js_value(&report)
}

/// JSON wire twin of [`dry_run`]: the validation report as pretty-printed JSON.
/// @param envelope_json - `CalibrationEnvelope` as a plain object or its JSON string: the plan (steps, quote sets, settings) plus `market_data` and optional `prior_market`.
///
/// # Errors
///
/// Throws a `CalibrationEnvelopeError` if the envelope is malformed, its schema
/// marker is missing, malformed, or unsupported, the envelope structure is
/// invalid, or the validation report cannot be serialized. Semantic findings
/// are returned in the report rather than thrown.
#[wasm_bindgen(js_name = dryRunJson)]
pub fn dry_run_json(envelope_json: JsValue) -> Result<String, JsValue> {
    let envelope_json: &str = &json_text(&envelope_json, "envelopeJson")?;
    validate::dry_run_json(envelope_json).map_err(|error| execute_error_to_js(error.into()))
}

/// Calibrate the explicit Bermudan LMM loading scale from the market surface.
///
/// Callers must place the returned value in `modelConfig.lmmBaseVol` before
/// pricing; the Bermudan pricer never reads or fits the surface itself.
/// @param instrument_json - Canonical Bermudan swaption instrument envelope, as a plain object or its JSON string.
/// @param market - Reusable market handle containing discount and swaption-volatility inputs.
/// @param as_of - ISO-8601 valuation date.
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
    instrument_json: JsValue,
    market: &crate::api::core::market_context::JsMarketContext,
    as_of: JsValue,
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
fn execute_error_to_js(err: ExecuteError) -> JsValue {
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

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_calibration::api::schema::{
        CalibrationEnvelope, CalibrationPlan, CalibrationSchema,
    };

    fn empty_envelope_json() -> String {
        let plan = CalibrationPlan {
            id: "empty".to_string(),
            description: None,
            quote_sets: Default::default(),
            steps: Vec::new(),
            settings: Default::default(),
        };
        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: CalibrationSchema::CURRENT,
            plan,
            market_data: Vec::new(),
            prior_market: Vec::new(),
        };
        serde_json::to_string(&envelope).expect("serialize")
    }

    #[test]
    fn validate_calibration_json_accepts_empty_plan() {
        let json = empty_envelope_json();
        let canonical = validate::validate_calibration_json(&json).expect("validate");
        assert!(!canonical.is_empty());
    }

    #[test]
    fn validation_and_execution_reject_semantically_invalid_requests() {
        let mut value: serde_json::Value =
            serde_json::from_str(&empty_envelope_json()).expect("empty envelope parses");
        value["plan"]["steps"] = serde_json::json!([{
            "id": "discount_step",
            "quote_set": "missing_quotes",
            "kind": "discount",
            "curve_id": "USD-OIS",
            "currency": "USD",
            "base_date": "2026-05-08"
        }]);
        let json = value.to_string();

        for error in [
            ExecuteError::from(
                validate::validate_calibration_json(&json)
                    .expect_err("validation must reject undefined quote sets"),
            ),
            engine::calibrate_from_json(&json)
                .expect_err("execution must reject undefined quote sets"),
        ] {
            let details = error.details();
            assert_eq!(details.stage.as_str(), "ingestion");
            assert_eq!(details.category, "undefined_quote_set");
            assert!(details.cause.contains("missing_quotes"));
        }
    }

    #[test]
    fn calibrate_empty_plan_succeeds() {
        let json = empty_envelope_json();
        let result = engine::calibrate_from_json(&json).expect("execute");
        let parsed = serde_json::to_value(&result).expect("json");
        assert!(parsed.is_object());
        assert!(parsed.get("result").is_some());
    }

    #[test]
    fn validate_calibration_returns_the_typed_envelope() {
        let envelope = validate::parse_envelope(&empty_envelope_json()).expect("parse");
        validate::validate_fail_fast(&envelope).expect("validate");
        assert_eq!(envelope.plan.id, "empty");
        assert!(validate::parse_envelope("{ not json").is_err());
    }

    #[test]
    fn dry_run_accepts_empty_plan() {
        let json = empty_envelope_json();
        let report = validate::dry_run(&json).expect("dry_run");
        assert!(report.errors.is_empty());
        let report_json = validate::dry_run_json(&json).expect("dry_run_json");
        let parsed: serde_json::Value = serde_json::from_str(&report_json).expect("json");
        assert!(parsed.get("errors").is_some());
        assert!(parsed.get("dependency_graph").is_some());
    }

    #[test]
    fn dry_run_rejects_malformed_json() {
        assert!(validate::dry_run("not json").is_err());
        assert!(validate::dry_run_json("not json").is_err());
    }

    #[test]
    fn validate_calibration_json_preserves_parse_diagnostic() {
        let error = ExecuteError::from(
            validate::validate_calibration_json("{ not valid json")
                .expect_err("malformed JSON must error"),
        );
        let details = error.details();
        assert_eq!(details.stage.as_str(), "ingestion");
        assert_eq!(details.category, "strict_load");
        assert!(!details.cause.is_empty());
        assert!(error.to_json().contains("\"stage\":\"ingestion\""));
    }

    #[test]
    fn validate_calibration_json_never_returns_empty_object_fallback() {
        let json = empty_envelope_json();
        let canonical = validate::validate_calibration_json(&json).expect("validate");
        assert_ne!(
            canonical, "{}",
            "canonical envelope JSON must not collapse to an empty object"
        );
        let (reparsed, _report) = CalibrationEnvelope::from_slice_strict(
            canonical.as_bytes(),
            &finstack_quant_core::contract::LoadLimits::default(),
        )
        .expect("canonical JSON must round-trip strictly");
        assert_eq!(reparsed.schema, CalibrationSchema::Calibration);
    }

    #[test]
    fn content_hashes_match_the_rust_typed_hashes() {
        let json = empty_envelope_json();
        let (envelope, _report) = CalibrationEnvelope::from_slice_strict(
            json.as_bytes(),
            &finstack_quant_core::contract::LoadLimits::default(),
        )
        .expect("envelope");
        assert_eq!(
            validate::parse_envelope(&json)
                .expect("parse")
                .content_hash()
                .expect("hash"),
            envelope.content_hash().expect("typed hash"),
        );
        assert!(validate::parse_envelope("{ not json").is_err());
        assert!(load_result("{ not json").is_err());
    }

    #[test]
    fn calibrate_preserves_malformed_envelope_diagnostic() {
        let error = engine::calibrate_from_json("{ this is not valid json")
            .expect_err("malformed envelope must error");
        let details = error.details();
        let json = error.to_json();
        assert_eq!(details.stage.as_str(), "ingestion");
        assert_eq!(details.category, "strict_load");
        assert!(
            json.contains("strict_load") && json.contains("cause"),
            "diagnostic JSON should carry the structured load error, got: {json}"
        );
        assert!(
            !details.cause.is_empty(),
            "diagnostic cause must not be empty"
        );
    }
}
