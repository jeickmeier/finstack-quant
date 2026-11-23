#![allow(clippy::result_large_err)]

//! Calibration envelope builders and quote-perturbation WASM entry points.

use crate::utils::input::{from_js_json, js_f64, js_string, opt_from_js_json};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_calibration::CalibrationConfig;
use finstack_quant_calibration::api::schema::{CalibrationEnvelope, StepWithQuotes};
use wasm_bindgen::prelude::*;

/// Build a calibration envelope from ordered steps with inline observations.
/// @param id - Nonempty plan identifier.
/// @param steps - Ordered step and quote-input pairs. An empty quote list can reference a previously defined quote set.
/// @param settings - Optional solver and validation configuration overrides.
/// @param priorMarket - Optional previously calibrated objects used as dependencies.
/// @returns Canonical calibration envelope ready for validation or execution.
/// @throws `TypeError` for a mistyped argument or `FinstackError` for invalid settings, malformed data, blank or duplicate identifiers, or duplicate quote-set definitions.
#[wasm_bindgen(js_name = calibrationEnvelopeFromSteps, unchecked_return_type = "CalibrationEnvelope")]
pub fn calibration_envelope_from_steps(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "readonly generated.calibration.StepWithQuotes[]")] steps: JsValue,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.calibration.CalibrationConfig | null"
    )]
    settings: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "priorMarket",
        unchecked_optional_param_type = "readonly generated.calibration.PriorMarketObject[] | null"
    )]
    prior_market: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let id = js_string(&id, "id")?;
    let steps: Vec<StepWithQuotes> = from_js_json(&steps, "steps")?;
    let settings = opt_from_js_json::<serde_json::Value>(settings.as_ref(), "settings")?
        .map(|overrides| CalibrationConfig::default().with_json_overrides(overrides))
        .transpose()
        .map_err(to_js_err)?
        .unwrap_or_default();
    let prior_market = opt_from_js_json(prior_market.as_ref(), "priorMarket")?.unwrap_or_default();
    let envelope =
        CalibrationEnvelope::from_steps(id, steps, settings, prior_market).map_err(to_js_err)?;
    to_js_value(&envelope)
}

/// Derive an envelope with one more step at the end of the execution order.
/// @param envelope - Envelope whose existing steps and settings are retained.
/// @param entry - Step and inline quotes. An empty list can reference an existing quote set.
/// @returns Independent calibration envelope with the appended step.
/// @throws `TypeError` for malformed argument shapes and `FinstackError` for invalid or duplicate identifiers or a redefined quote set.
#[wasm_bindgen(js_name = calibrationEnvelopeWithStep, unchecked_return_type = "CalibrationEnvelope")]
pub fn calibration_envelope_with_step(
    #[wasm_bindgen(unchecked_param_type = "CalibrationEnvelope")] envelope: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.calibration.StepWithQuotes")] entry: JsValue,
) -> Result<JsValue, JsValue> {
    let envelope: CalibrationEnvelope = from_js_json(&envelope, "envelope")?;
    let entry = from_js_json(&entry, "entry")?;
    to_js_value(&envelope.with_step(entry).map_err(to_js_err)?)
}

/// Derive an envelope without one quote or any references to it.
/// @param envelope - Calibration envelope containing the quoted observation.
/// @param quoteId - Exact stable ID of the quote.
/// @returns Independent envelope with matching references removed.
/// @throws `TypeError` for malformed arguments or `FinstackError` for an unknown or non-quote ID.
#[wasm_bindgen(js_name = calibrationEnvelopeWithoutQuote, unchecked_return_type = "CalibrationEnvelope")]
pub fn calibration_envelope_without_quote(
    #[wasm_bindgen(unchecked_param_type = "CalibrationEnvelope")] envelope: JsValue,
    #[wasm_bindgen(js_name = "quoteId", unchecked_param_type = "string")] quote_id: JsValue,
) -> Result<JsValue, JsValue> {
    let envelope: CalibrationEnvelope = from_js_json(&envelope, "envelope")?;
    let quote_id = js_string(&quote_id, "quoteId")?;
    to_js_value(&envelope.without_quote(&quote_id).map_err(to_js_err)?)
}

/// Apply a signed rate or spread shock to a derived calibration envelope.
/// @param envelope - Calibration envelope with the original market inputs.
/// @param quoteSet - Exact quote-set name or null to select every quote once.
/// @param bp - Finite signed basis points. One basis point adds 0.0001 to decimal rates, lowers rate-futures prices by 0.01, or adds one to spreads quoted in basis points.
/// @returns Independent envelope with the selected quotes shifted.
/// @throws `TypeError` for malformed arguments or `FinstackError` for nonfinite shocks, unknown sets or references, or quotes without a rate or spread bump basis.
#[wasm_bindgen(js_name = calibrationEnvelopeBumpQuotesBp, unchecked_return_type = "CalibrationEnvelope")]
pub fn calibration_envelope_bump_quotes_bp(
    #[wasm_bindgen(unchecked_param_type = "CalibrationEnvelope")] envelope: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_param_type = "string | null")]
    quote_set: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] bp: JsValue,
) -> Result<JsValue, JsValue> {
    let envelope: CalibrationEnvelope = from_js_json(&envelope, "envelope")?;
    let quote_set = if quote_set.is_null() {
        None
    } else {
        Some(js_string(&quote_set, "quoteSet")?)
    };
    let bp = js_f64(&bp, "bp")?;
    to_js_value(
        &envelope
            .bump_quotes_bp(quote_set.as_deref(), bp)
            .map_err(to_js_err)?,
    )
}
