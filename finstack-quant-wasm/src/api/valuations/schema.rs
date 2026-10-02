//! Compiled-in JSON Schemas for the valuations wire format.
//!
//! The instrument envelope, the per-type instrument schemas and the valuation
//! result schema are embedded in the WASM module by the Rust accessors in
//! `finstack_quant_valuations::schema`, so they always describe the wire
//! format this build accepts. Each accessor returns the schema document as a
//! plain object; the Python `finstack_quant.valuations.schema` accessors
//! return the same document as JSON text.

use crate::utils::input::{from_js_json, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::schema as canonical;
use wasm_bindgen::prelude::*;

/// JSON Schema of the canonical instrument envelope.
///
/// The envelope is the `finstack_quant.instrument/1` wrapper carrying a
/// `type` discriminator and the matching `spec` payload. The document is
/// large (about one megabyte); read it once and cache the parsed result.
/// @returns JSON Schema document of the instrument envelope.
/// @throws Error - Throws with kind `validation` if the embedded schema is malformed (does not occur for a released build).
#[wasm_bindgen(js_name = instrumentEnvelopeSchema)]
pub fn instrument_envelope_schema() -> Result<JsValue, JsValue> {
    to_js_value(canonical::instrument_envelope_schema().map_err(to_js_err)?)
}

/// Canonical instrument type discriminators, in registry order.
/// @returns Every `type` tag accepted by the instrument envelope, e.g. `"bond"`, `"interest_rate_swap"`.
/// @throws Error - Throws if the instrument registry cannot be read (does not occur for a released build).
#[wasm_bindgen(js_name = instrumentTypes)]
pub fn instrument_types() -> Result<Vec<String>, JsValue> {
    canonical::instrument_types().map_err(to_js_err)
}

/// JSON Schema of one instrument type.
/// @param instrument_type - Canonical instrument type discriminator from `instrumentTypes()`, e.g. `"bond"`.
/// @returns JSON Schema document of that instrument type's envelope.
/// @throws Error - Throws with kind `not_found` if `instrumentType` is not a registered instrument type, and kind `invalid_type` if it is not a string.
#[wasm_bindgen(js_name = instrumentSchema)]
pub fn instrument_schema(instrument_type: JsValue) -> Result<JsValue, JsValue> {
    let instrument_type = js_string(&instrument_type, "instrumentType")?;
    to_js_value(&canonical::instrument_schema(&instrument_type).map_err(to_js_err)?)
}

/// JSON Schema of the `ValuationResult` envelope returned by the pricing entry points.
/// @returns JSON Schema document of the valuation result.
/// @throws Error - Throws with kind `validation` if the embedded schema is malformed (does not occur for a released build).
#[wasm_bindgen(js_name = valuationResultSchema)]
pub fn valuation_result_schema() -> Result<JsValue, JsValue> {
    to_js_value(canonical::valuation_result_schema().map_err(to_js_err)?)
}

/// Canonical compact JSON of a validated instrument payload.
fn canonical_json(instance: &serde_json::Value) -> Result<String, JsValue> {
    serde_json::to_string(instance).map_err(to_js_err)
}

/// Validate an instrument envelope against the canonical schemas.
///
/// Both the envelope schema and the schema selected by `instrument.type` are
/// applied, as the Rust loader does when it decodes tagged instrument JSON.
/// @param instrument_json - A `finstack_quant.instrument/1` envelope, as JSON text or a plain object.
/// @returns Canonical compact JSON of the validated payload.
/// @throws Error - Throws with kind `validation` if `instrumentJson` is JSON text that does not parse, or if it violates the envelope schema or its instrument type's schema (the message lists every violation); and kind `invalid_type` if it is neither JSON text nor a plain object.
#[wasm_bindgen(js_name = validateInstrumentEnvelopeJson)]
pub fn validate_instrument_envelope_json(instrument_json: JsValue) -> Result<String, JsValue> {
    let instance: serde_json::Value = from_js_json(&instrument_json, "instrumentJson")?;
    canonical::validate_instrument_envelope_json(&instance).map_err(to_js_err)?;
    canonical_json(&instance)
}

/// Validate a payload against one instrument type's schema.
/// @param instrument_type - Canonical instrument type discriminator from `instrumentTypes()`, e.g. `"bond"`.
/// @param instrument_json - Payload to check against that type's schema, as JSON text or a plain object.
/// @returns Canonical compact JSON of the validated payload.
/// @throws Error - Throws with kind `not_found` if `instrumentType` is not a registered instrument type; kind `validation` if `instrumentJson` is JSON text that does not parse or violates the type schema (the message lists every violation); and kind `invalid_type` if `instrumentType` is not a string or `instrumentJson` is neither JSON text nor a plain object.
#[wasm_bindgen(js_name = validateInstrumentTypeJson)]
pub fn validate_instrument_type_json(
    instrument_type: JsValue,
    instrument_json: JsValue,
) -> Result<String, JsValue> {
    let instrument_type = js_string(&instrument_type, "instrumentType")?;
    let instance: serde_json::Value = from_js_json(&instrument_json, "instrumentJson")?;
    canonical::validate_instrument_type_json(&instrument_type, &instance).map_err(to_js_err)?;
    canonical_json(&instance)
}
