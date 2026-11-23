//! Attribution execution, bridge, batch, and return-contribution WASM entry points.

//! inside JSON strings, where `JSON.parse` would silently round a larger
//! integer. Today every count in the attribution surface is bounded by a
//! handful of factors (≤ 12) and a handful of repricings (≤ ~30), well under
//! the safe-integer ceiling.

use crate::utils::input::{js_string, json_text};
use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Deserialize the canonical single-instrument spec and execute it once.
fn run_attribute_pnl(
    spec: &JsValue,
) -> Result<finstack_quant_attribution::AttributionResult, JsValue> {
    let spec: finstack_quant_attribution::AttributionSpec =
        crate::utils::input::from_js_json(spec, "spec")?;
    spec.execute().map_err(to_js_err)
}

/// Attribute one instrument from its canonical Rust specification.
///
/// Accepts an `AttributionSpec` plain object or its JSON text. All Money fields
/// retain decimal-string amounts; dates are ISO calendar dates. The spec carries
/// markets, method, model snapshots, credit detail options and execution config.
///
/// # Arguments
///
/// * `spec` - Canonical `AttributionSpec` object or JSON text with the instrument
///   payload, market states, ISO calendar dates, method and optional config.
///
/// # Returns
///
/// The native-currency or configured reporting-currency `PnlAttribution` object.
///
/// # Errors
///
/// Throws a classified `FinstackError` for malformed or unknown spec fields,
/// invalid dates, unavailable market data, pricing, currency, configuration or
/// method failures, or a contained Rust panic. Invalid host types throw `TypeError`.
/// @param spec - Canonical `AttributionSpec` object or JSON text with the instrument payload, market states, ISO calendar dates, method and optional config.
/// @returns The native-currency or configured reporting-currency `PnlAttribution` object.
#[wasm_bindgen(js_name = attributePnl, unchecked_return_type = "PnlAttribution")]
pub fn attribute_pnl(
    #[wasm_bindgen(unchecked_param_type = "AttributionSpec | string")] spec: JsValue,
) -> Result<JsValue, JsValue> {
    let result = run_attribute_pnl(&spec)?;
    crate::utils::to_js_value(&result.attribution)
}

/// Attribute one instrument and return exact Rust wire JSON.
///
/// # Arguments
///
/// * `spec` - Canonical `AttributionSpec` object or JSON text with the instrument
///   payload, market states, ISO calendar dates, method and optional config.
///
/// # Returns
///
/// Compact canonical `PnlAttribution` JSON, preserving decimal amounts and map order.
///
/// # Errors
///
/// Throws the same classified errors as `attributePnl`, or a serialization error.
/// @param spec - Canonical `AttributionSpec` object or JSON text with the instrument payload, market states, ISO calendar dates, method and optional config.
/// @returns Compact canonical `PnlAttribution` JSON, preserving decimal amounts and map order.
#[wasm_bindgen(js_name = attributePnlJson)]
pub fn attribute_pnl_json(
    #[wasm_bindgen(unchecked_param_type = "AttributionSpec | string")] spec: JsValue,
) -> Result<String, JsValue> {
    let result = run_attribute_pnl(&spec)?;
    serde_json::to_string(&result.attribution).map_err(to_js_err)
}

/// Parse and execute one `AttributionEnvelope`.
///
/// Shared by `attributePnlEnvelope` and `attributePnlEnvelopeJson`.
fn run_attribute_pnl_envelope(
    spec_json: &JsValue,
) -> Result<finstack_quant_attribution::AttributionResultEnvelope, JsValue> {
    let spec_json: &str = &json_text(spec_json, "specJson")?;
    let envelope =
        finstack_quant_attribution::AttributionEnvelope::from_json(spec_json).map_err(to_js_err)?;
    envelope.execute().map_err(to_js_err)
}

/// Run attribution from a full `AttributionEnvelope` and return the result envelope.
///
/// Returns the Rust `AttributionResultEnvelope` as a plain object:
/// `{ schema: "finstack_quant.attribution/1", result: { attribution, results_meta } }`.
/// Use `attributePnlEnvelopeJson` for the JSON wire string.
///
/// # Errors
///
/// Rejects malformed, schema-incompatible, or unsupported-version `spec_json`;
/// instrument or market reconstruction, pricing, FX, rounding, metric, or
/// method-specific attribution failures; a caught execution panic; or
/// failure to convert the result envelope to a JavaScript value.
/// @param specJson - JSON-serialized AttributionEnvelope (schema `finstack_quant.attribution/1`) to validate and execute.
/// @returns The `AttributionResultEnvelope` as a plain object.
#[wasm_bindgen(js_name = attributePnlEnvelope, unchecked_return_type = "AttributionResultEnvelope")]
pub fn attribute_pnl_envelope(
    #[wasm_bindgen(js_name = "specJson", unchecked_param_type = "JsonInput")] spec_json: JsValue,
) -> Result<JsValue, JsValue> {
    let result_envelope = run_attribute_pnl_envelope(&spec_json)?;
    crate::utils::to_js_value(&result_envelope)
}

/// Run attribution from a full JSON `AttributionEnvelope` and return JSON.
///
/// Wire twin of `attributePnlEnvelope` for full envelope round-trip
/// workflows.
///
/// # Errors
///
/// Rejects the same conditions as `attributePnlEnvelope`, plus failure to
/// serialize the result envelope.
/// @param specJson - JSON-serialized AttributionEnvelope (schema `finstack_quant.attribution/1`) to validate and execute.
/// @returns JSON attribution result envelope for the supplied spec.
#[wasm_bindgen(js_name = attributePnlEnvelopeJson)]
pub fn attribute_pnl_envelope_json(
    #[wasm_bindgen(js_name = "specJson", unchecked_param_type = "JsonInput")] spec_json: JsValue,
) -> Result<String, JsValue> {
    let result_envelope = run_attribute_pnl_envelope(&spec_json)?;
    serde_json::to_string(&result_envelope).map_err(to_js_err)
}

/// Validate an attribution specification JSON.
///
/// Deserializes against the `AttributionEnvelope` schema, checks the
/// `schema` version tag (the same gate `execute` applies, so a payload that
/// validates here cannot later be rejected at execution), and returns the
/// canonical JSON.
///
/// # Errors
///
/// Rejects malformed, schema-incompatible, or unsupported-version `json`, or
/// failure to serialize the canonical attribution envelope.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
/// @returns Canonical attribution-envelope JSON after schema validation.
#[wasm_bindgen(js_name = validateAttributionJson)]
pub fn validate_attribution_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    finstack_quant_attribution::validate_attribution_json(json).map_err(to_js_err)
}

/// Return the default waterfall factor ordering as canonical snake-case values.
///
/// # Errors
///
/// Rejects if the default factor identifiers cannot be serialized to
/// JavaScript.
/// @returns Default waterfall factor names in execution order.
#[wasm_bindgen(js_name = defaultWaterfallOrder, unchecked_return_type = "string[]")]
pub fn default_waterfall_order() -> Result<JsValue, JsValue> {
    let factors: Vec<String> = finstack_quant_attribution::default_waterfall_order()
        .into_iter()
        .map(|factor| factor.as_str().to_owned())
        .collect();
    crate::utils::to_js_value(&factors)
}

/// Return the default metric IDs used by metrics-based attribution.
///
/// # Errors
///
/// Rejects if the default metric identifiers cannot be serialized to
/// JavaScript.
/// @returns Default metric identifiers used by metrics-based attribution.
#[wasm_bindgen(js_name = defaultAttributionMetrics, unchecked_return_type = "string[]")]
pub fn default_attribution_metrics() -> Result<JsValue, JsValue> {
    let metrics: Vec<String> = finstack_quant_attribution::default_attribution_metrics()
        .into_iter()
        .map(|m| m.to_string())
        .collect();
    crate::utils::to_js_value(&metrics)
}

/// Headline P&L bridge: `value(T₁) − value(T₀)` in one currency.
///
/// Mirrors Python `pnl_bridge` (Rust `pnl_bridge`): two repricings and no
/// factor loop. The T₀ value converts into `targetCurrency` with the T₀
/// market's FX and the T₁ value with the T₁ market's FX. Use `attributePnl`
/// when the factor decomposition matters.
/// @param instrumentJson - Canonical `finstack_quant.instrument/1` envelope (object or JSON).
/// @param marketT0Json - Canonical MarketContext at the opening date.
/// @param marketT1Json - Canonical MarketContext at the closing date.
/// @param asOfT0 - Opening valuation date as an ISO-8601 string.
/// @param asOfT1 - Closing valuation date as an ISO-8601 string.
/// @param targetCurrency - ISO-4217 currency of the returned P&L.
/// @returns The P&L as a `Money` handle in `targetCurrency`.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, kind `not_found`
/// if a curve, market item or FX leg is missing, and kind `computation` if
/// pricing fails.
#[wasm_bindgen(js_name = pnlBridge)]
pub fn pnl_bridge(
    #[wasm_bindgen(js_name = "instrumentJson", unchecked_param_type = "JsonInput")] instrument_json: JsValue,
    #[wasm_bindgen(js_name = "marketT0Json", unchecked_param_type = "JsonInput")]
    market_t0_json: JsValue,
    #[wasm_bindgen(js_name = "marketT1Json", unchecked_param_type = "JsonInput")]
    market_t1_json: JsValue,
    #[wasm_bindgen(js_name = "asOfT0", unchecked_param_type = "string")] as_of_t0: JsValue,
    #[wasm_bindgen(js_name = "asOfT1", unchecked_param_type = "string")] as_of_t1: JsValue,
    #[wasm_bindgen(js_name = "targetCurrency", unchecked_param_type = "string")]
    target_currency: JsValue,
) -> Result<crate::api::core::money::JsMoney, JsValue> {
    let instrument = finstack_quant_valuations::pricer::parse_instrument_from_json(&json_text(
        &instrument_json,
        "instrumentJson",
    )?)
    .map_err(to_js_err)?;
    let instrument: std::sync::Arc<dyn finstack_quant_valuations::instruments::Instrument> =
        std::sync::Arc::from(instrument.into_boxed().map_err(to_js_err)?);
    let market_t0: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(&json_text(&market_t0_json, "marketT0Json")?).map_err(to_js_err)?;
    let market_t1: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(&json_text(&market_t1_json, "marketT1Json")?).map_err(to_js_err)?;
    let target_currency: finstack_quant_core::currency::Currency =
        js_string(&target_currency, "targetCurrency")?
            .parse()
            .map_err(to_js_err)?;
    let inner = finstack_quant_attribution::pnl_bridge(
        &instrument,
        &market_t0,
        &market_t1,
        crate::utils::parse_iso_date(&js_string(&as_of_t0, "asOfT0")?)?,
        crate::utils::parse_iso_date(&js_string(&as_of_t1, "asOfT1")?)?,
        target_currency,
    )
    .map_err(to_js_err)?;
    Ok(crate::api::core::money::JsMoney { inner })
}

/// Attribute a batch using canonical shared inputs and instrument envelopes.
///
/// # Arguments
///
/// * `inputs` - `AttributionInputs` object or JSON containing markets, dates,
///   method and execution options; no placeholder instrument is required.
/// * `instruments` - Ordered array of canonical instrument envelopes, each an
///   object or JSON string. The array itself may also be serialized JSON.
///
/// # Returns
///
/// One `PnlAttribution` object per instrument, in input order.
///
/// # Errors
///
/// Throws a classified `FinstackError` for malformed inputs or envelopes,
/// or the first instrument's pricing, market-data, currency or validation error.
/// @param inputs - `AttributionInputs` object or JSON containing markets, dates, method and execution options; no placeholder instrument is required.
/// @param instruments - Ordered array of canonical instrument envelopes, each an object or JSON string. The array itself may also be serialized JSON.
/// @returns One `PnlAttribution` object per instrument, in input order.
#[wasm_bindgen(js_name = attributePnlMany, unchecked_return_type = "PnlAttribution[]")]
pub fn attribute_pnl_many(
    #[wasm_bindgen(unchecked_param_type = "AttributionInputs | string")] inputs: JsValue,
    #[wasm_bindgen(unchecked_param_type = "JsonInput[] | string")] instruments: JsValue,
) -> Result<JsValue, JsValue> {
    let inputs: finstack_quant_attribution::AttributionInputs =
        crate::utils::input::from_js_json(&inputs, "inputs")?;
    let instruments: Vec<serde_json::Value> =
        crate::utils::input::from_js_json(&instruments, "instruments")?;
    let instruments = instruments
        .into_iter()
        .map(|value| {
            let json = match value {
                serde_json::Value::String(json) => json,
                object => object.to_string(),
            };
            finstack_quant_valuations::pricer::parse_instrument_from_json(&json).map_err(to_js_err)
        })
        .collect::<Result<Vec<_>, JsValue>>()?;
    let attributions =
        finstack_quant_attribution::attribute_pnl_many(&inputs, instruments).map_err(to_js_err)?;
    crate::utils::to_js_value(&attributions)
}

/// Compute return-contribution attribution from a specification.
///
/// Mirrors Python `attribute_return_contribution` (Rust
/// `attribute_return_contribution`): per-position contributions, group and
/// factor roll-ups and, with a benchmark, Brinson-style allocation and
/// selection effects.
/// @param spec - `ReturnContributionSpec` (object or JSON): positions with weights and returns, weighting scheme and optional benchmark.
/// @returns Plain `ReturnContributionResult` object.
///
/// # Errors
///
/// Throws with kind `validation` if the spec is malformed or violates the
/// weighting/benchmark invariants (including a Brinson group with zero net
/// weight but nonzero contribution).
#[wasm_bindgen(js_name = attributeReturnContribution, unchecked_return_type = "ReturnContributionResult")]
pub fn attribute_return_contribution(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] spec: JsValue,
) -> Result<JsValue, JsValue> {
    let spec: finstack_quant_attribution::ReturnContributionSpec =
        crate::utils::input::from_js_json(&spec, "spec")?;
    let result =
        finstack_quant_attribution::attribute_return_contribution(&spec).map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Compute return-contribution attribution and return wire JSON.
///
/// Wire twin of `attributeReturnContribution` (Rust
/// `attribute_return_contribution_json`).
/// @param specJson - `ReturnContributionSpec` (object or JSON).
/// @returns Canonical `ReturnContributionResult` JSON text.
///
/// # Errors
///
/// Throws with kind `validation` if the spec is malformed or violates the
/// weighting/benchmark invariants.
#[wasm_bindgen(js_name = attributeReturnContributionJson)]
pub fn attribute_return_contribution_json(
    #[wasm_bindgen(js_name = "specJson", unchecked_param_type = "JsonInput")] spec_json: JsValue,
) -> Result<String, JsValue> {
    finstack_quant_attribution::attribute_return_contribution_json(&json_text(
        &spec_json, "specJson",
    )?)
    .map_err(to_js_err)
}

/// Validate a return-contribution specification and return its canonical JSON.
///
/// Mirrors Python `validate_return_contribution_json`: the spec is parsed and
/// executed, so a spec that validates here also computes.
/// @param specJson - `ReturnContributionSpec` (object or JSON).
/// @returns Canonical compact spec JSON.
///
/// # Errors
///
/// Throws with kind `validation` if the spec is malformed or violates the
/// weighting/benchmark invariants.
#[wasm_bindgen(js_name = validateReturnContributionJson)]
pub fn validate_return_contribution_json(
    #[wasm_bindgen(js_name = "specJson", unchecked_param_type = "JsonInput")] spec_json: JsValue,
) -> Result<String, JsValue> {
    finstack_quant_attribution::validate_return_contribution_json(&json_text(
        &spec_json, "specJson",
    )?)
    .map_err(to_js_err)
}
