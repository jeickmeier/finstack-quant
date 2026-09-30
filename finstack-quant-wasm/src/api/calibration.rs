//! WASM bindings for the calibration engine.
//!
//! Mirrors the Python `calibrate` / `validate_calibration_json` surface plus
//! diagnostics (`dryRun` and its `dryRunJson` wire twin).
//!
//! # Number safety
//!
//! Counts (`iterations`, `residual_evals`, `lm_jacobian_evals`) are embedded
//! inside the JSON result envelope rather than crossed as raw `usize`. JS's
//! `JSON.parse` reads them as IEEE-754 doubles; values above
//! `Number.MAX_SAFE_INTEGER` (2^53 − 1) would round silently; in practice
//! iteration counts stay under ~10⁴ for any non-pathological calibration.
//!
//! On error, the host functions throw a JS `Error` with `name =
//! "CalibrationEnvelopeError"`. The error exposes `kind` (the execution
//! category), `stage`, `step_id`, `solver_diagnostics` (an object), the
//! strict-load `diagnostics` array (empty unless ingestion rejected the
//! document), JSON-string `details`, and a structured `cause` object. Absent
//! optional properties are `undefined`.
//!
//! # Native (non-wasm32) builds
//!
//! `JsValue` is opaque on native targets: every non-`const` constructor
//! (`JsValue::from_str`, `js_sys::Error::new`, ...) is a `wasm-bindgen` stub
//! that aborts the process. So the `#[wasm_bindgen]` wrappers below are kept
//! *thin* and the diagnostic-bearing logic lives in `*_inner` helpers that
//! return the structured `EnvelopeError` / `ExecuteError` directly. Native
//! tests exercise those helpers and assert on the real diagnostic — the
//! `JsValue` boundary (where the structured error would otherwise collapse to
//! an opaque value) is crossed only at the `#[wasm_bindgen]` edge.

// `EnvelopeError` / `ExecuteError` are intentionally large structured errors
// (rich diagnostic payloads); boxing them would change their public API.
// The upstream `calibration::api::{engine, validate}` modules make the same
// allowance — keep the binding layer consistent.
#![allow(clippy::result_large_err)]

use crate::utils::input::{js_string, json_text};
#[cfg(target_arch = "wasm32")]
use crate::utils::structured_js_error;
use crate::utils::to_js_value;
use finstack_quant_calibration::api::engine::{self, ExecuteError};
#[cfg(test)]
use finstack_quant_calibration::api::schema::CalibrationEnvelope;
use finstack_quant_calibration::api::schema::CalibrationResultEnvelope;
use finstack_quant_calibration::api::validate;
use wasm_bindgen::prelude::*;

/// Native-testable core of [`validate_calibration_json`].
///
/// Parses the envelope and returns its canonical (pretty-printed) form. A parse
/// failure surfaces a structured
/// [`EnvelopeError`](finstack_quant_calibration::api::errors::EnvelopeError)
/// preserving the full parse diagnostic. The Rust calibration API owns the
/// canonical serialization path, keeping the WASM layer to error mapping only.
fn validate_calibration_json_inner(json: &str) -> Result<String, ExecuteError> {
    validate::validate_calibration_json(json).map_err(ExecuteError::from)
}

/// Validate a calibration plan JSON and return the canonical (pretty-printed) form.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
///
/// # Errors
///
/// Throws a JavaScript exception if `json` is malformed, its calibration
/// schema marker is missing, malformed, or unsupported, static envelope
/// validation fails (fail-fast: first error; `dryRun` lists every static
/// error), or the canonical envelope cannot be serialized.
#[wasm_bindgen(js_name = validateCalibrationJson)]
pub fn validate_calibration_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    validate_calibration_json_inner(json).map_err(execute_error_to_js)
}

/// Native-testable core of [`calibrate`].
///
/// Returns the `CalibrationResultEnvelope`, or an [`ExecuteError`] (which
/// carries the structured `EnvelopeError` payload when the failure is
/// envelope-related).
fn calibrate_inner(envelope_json: &str) -> Result<CalibrationResultEnvelope, ExecuteError> {
    engine::calibrate_from_json(envelope_json)
}

/// Execute a calibration plan and return the full result envelope.
///
/// Accepts a serialized `CalibrationEnvelope` (plan + quote sets + optional
/// flat `market_data` / `prior_market` lists) and returns a plain JavaScript
/// `CalibrationResultEnvelope` object — the shape `index.d.ts` has always
/// declared. Re-ingest a sub-document (e.g. `result.result.final_market`) with
/// `JSON.stringify`.
/// @param envelope_json - CalibrationEnvelope JSON containing targets, parameters, bounds, and dependencies.
///
/// # Errors
///
/// Throws a JavaScript exception if `envelopeJson` is malformed or violates
/// the calibration schema or static plan contract (fail-fast: first static
/// error; `dryRun` lists every static error), market context construction
/// or a calibration step fails, a solver does not converge, or the result
/// envelope cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = calibrate)]
pub fn calibrate(envelope_json: JsValue) -> Result<JsValue, JsValue> {
    let envelope_json: &str = &json_text(&envelope_json, "envelopeJson")?;
    let result = calibrate_inner(envelope_json).map_err(execute_error_to_js)?;
    to_js_value(&result)
}

/// Pre-flight envelope validation without invoking the solver.
///
/// Returns the `CalibrationValidationReport` as a plain object listing every
/// static error found (`errors`) plus the dependency graph
/// (`dependency_graph`). Microseconds. Use [`dry_run_json`] for the JSON
/// wire string.
/// @param envelope_json - CalibrationEnvelope JSON containing targets, parameters, bounds, and dependencies.
///
/// # Errors
///
/// Throws a JavaScript exception if `envelopeJson` is malformed, its schema
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
/// @param envelope_json - CalibrationEnvelope JSON containing targets, parameters, bounds, and dependencies.
///
/// # Errors
///
/// Throws a JavaScript exception if `envelopeJson` is malformed, its schema
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
/// @param instrument_json - Canonical Bermudan swaption instrument envelope JSON.
/// @param market - Reusable market handle containing discount and swaption-volatility inputs.
/// @param as_of - ISO-8601 valuation date.
/// @returns Positive finite LMM base volatility.
///
/// # Errors
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
/// a `CalibrationEnvelopeError` carrying the Rust-owned host payload.
fn execute_error_to_js(err: ExecuteError) -> JsValue {
    let host = err.host_error();
    #[cfg(target_arch = "wasm32")]
    {
        let error = structured_js_error(
            "CalibrationEnvelopeError",
            &host.message,
            Some(&host.kind),
            Some(&host.details),
        );
        let solver_diagnostics = match host.solver_diagnostics.as_ref() {
            Some(diagnostics) => match to_js_value(diagnostics) {
                Ok(value) => value,
                Err(serialization_error) => return serialization_error,
            },
            None => JsValue::UNDEFINED,
        };
        let diagnostics = match to_js_value(&host.diagnostics) {
            Ok(value) => value,
            Err(serialization_error) => return serialization_error,
        };
        let step_id = host
            .step_id
            .as_deref()
            .map_or(JsValue::UNDEFINED, JsValue::from);
        for (name, value) in [
            ("stage", JsValue::from(host.stage.as_str())),
            ("step_id", step_id),
            ("solver_diagnostics", solver_diagnostics),
            ("details", JsValue::from(&host.details)),
            ("diagnostics", diagnostics),
        ] {
            let _ = js_sys::Reflect::set(&error, &JsValue::from(name), &value);
        }
        error
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = host;
        JsValue::UNDEFINED
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_calibration::api::schema::{CalibrationPlan, CalibrationSchema};

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
            schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
            plan,
            market_data: Vec::new(),
            prior_market: Vec::new(),
        };
        serde_json::to_string(&envelope).expect("serialize")
    }

    #[test]
    fn validate_calibration_json_accepts_empty_plan() {
        let json = empty_envelope_json();
        let canonical = validate_calibration_json_inner(&json).expect("validate");
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
            validate_calibration_json_inner(&json)
                .expect_err("validation must reject undefined quote sets"),
            calibrate_inner(&json).expect_err("execution must reject undefined quote sets"),
        ] {
            let details = error.details();
            assert_eq!(details.stage.as_str(), "ingestion");
            assert_eq!(details.category, "undefined_quote_set");
            assert!(details.cause.contains("missing_quotes"));
        }
    }

    #[test]
    fn calibrate_empty_plan_succeeds() {
        // Drives the native-testable core: the `#[wasm_bindgen]` wrapper only
        // adds the `JsValue` conversion, which aborts on non-wasm32 targets.
        let json = empty_envelope_json();
        let result = calibrate_inner(&json).expect("execute");
        let parsed = serde_json::to_value(&result).expect("json");
        assert!(parsed.is_object());
        assert!(parsed.get("result").is_some());
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
    fn validate_calibration_json_inner_preserves_parse_diagnostic() {
        let error = validate_calibration_json_inner("{ not valid json")
            .expect_err("malformed JSON must error");
        let details = error.details();
        assert_eq!(details.stage.as_str(), "ingestion");
        assert_eq!(details.category, "strict_load");
        assert!(!details.cause.is_empty());
        assert!(error.to_json().contains("\"stage\":\"ingestion\""));
    }

    #[test]
    fn validate_calibration_json_inner_never_returns_empty_object_fallback() {
        // A successfully validated envelope must round-trip to a non-trivial
        // canonical JSON — never the literal `"{}"` that the old silent
        // `unwrap_or_else` fallback would have produced on a (hypothetical)
        // re-serialization failure. This guards the regression: the success
        // path returns the real canonical form, and the error path (covered
        // by `validate_calibration_json_inner_preserves_parse_diagnostic`)
        // returns a structured error rather than a fake `"{}"` success.
        let json = empty_envelope_json();
        let canonical = validate_calibration_json_inner(&json).expect("validate");
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
    fn calibrate_inner_preserves_malformed_envelope_diagnostic() {
        let error =
            calibrate_inner("{ this is not valid json").expect_err("malformed envelope must error");
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
