//! WASM bindings for P&L attribution across multiple methodologies.
//!
//! # Number safety
//!
//! The structured entry points (`attributePnl`, `attributePnlEnvelope`,
//! `attributePnlMany`) return objects built by `crate::utils::to_js_value`, so
//! counts and metrics (`num_repricings`, residuals, factor P&Ls) arrive as JS
//! numbers; that serializer rejects an integer above `Number.MAX_SAFE_INTEGER`
//! (2^53 − 1) instead of rounding it. Only the `*Json` wire twins carry them
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
    spec.execute_contained().map_err(to_js_err)
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
#[wasm_bindgen(js_name = attributePnl)]
pub fn attribute_pnl(spec: JsValue) -> Result<JsValue, JsValue> {
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
#[wasm_bindgen(js_name = attributePnlJson)]
pub fn attribute_pnl_json(spec: JsValue) -> Result<String, JsValue> {
    let result = run_attribute_pnl(&spec)?;
    serde_json::to_string(&result.attribution).map_err(to_js_err)
}

/// Parse and execute one `AttributionEnvelope` with panic containment.
///
/// Shared by [`attribute_pnl_envelope`] and [`attribute_pnl_envelope_json`].
fn run_attribute_pnl_envelope(
    spec_json: &JsValue,
) -> Result<finstack_quant_attribution::AttributionResultEnvelope, JsValue> {
    let spec_json: &str = &json_text(spec_json, "specJson")?;
    let envelope =
        finstack_quant_attribution::AttributionEnvelope::from_json(spec_json).map_err(to_js_err)?;
    envelope.execute_contained().map_err(to_js_err)
}

/// Run attribution from a full `AttributionEnvelope` and return the result envelope.
///
/// Returns the Rust `AttributionResultEnvelope` as a plain object:
/// `{ schema: "finstack_quant.attribution/1", result: { attribution, results_meta } }`.
/// Use [`attribute_pnl_envelope_json`] for the JSON wire string.
///
/// # Errors
///
/// Rejects malformed, schema-incompatible, or unsupported-version `spec_json`;
/// instrument or market reconstruction, pricing, FX, rounding, metric, or
/// method-specific attribution failures; a caught execution panic; or
/// failure to convert the result envelope to a JavaScript value.
/// @param spec_json - JSON-serialized AttributionEnvelope (schema `finstack_quant.attribution/1`) to validate and execute.
#[wasm_bindgen(js_name = attributePnlEnvelope)]
pub fn attribute_pnl_envelope(spec_json: JsValue) -> Result<JsValue, JsValue> {
    let result_envelope = run_attribute_pnl_envelope(&spec_json)?;
    crate::utils::to_js_value(&result_envelope)
}

/// Run attribution from a full JSON `AttributionEnvelope` and return JSON.
///
/// Wire twin of [`attribute_pnl_envelope`] for full envelope round-trip
/// workflows.
///
/// # Errors
///
/// Rejects the same conditions as `attributePnlEnvelope`, plus failure to
/// serialize the result envelope.
/// @param spec_json - JSON-serialized AttributionEnvelope (schema `finstack_quant.attribution/1`) to validate and execute.
#[wasm_bindgen(js_name = attributePnlEnvelopeJson)]
pub fn attribute_pnl_envelope_json(spec_json: JsValue) -> Result<String, JsValue> {
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
#[wasm_bindgen(js_name = validateAttributionJson)]
pub fn validate_attribution_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    finstack_quant_attribution::validate_attribution_json(json).map_err(to_js_err)
}

/// Return the default waterfall factor ordering as canonical snake-case values.
///
/// # Errors
///
/// Rejects if the default factor identifiers cannot be serialized to
/// JavaScript.
#[wasm_bindgen(js_name = defaultWaterfallOrder)]
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
#[wasm_bindgen(js_name = defaultAttributionMetrics)]
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
/// @param instrument_json - Canonical `finstack_quant.instrument/1` envelope (object or JSON).
/// @param market_t0_json - Canonical MarketContext at the opening date.
/// @param market_t1_json - Canonical MarketContext at the closing date.
/// @param as_of_t0 - Opening valuation date as an ISO-8601 string.
/// @param as_of_t1 - Closing valuation date as an ISO-8601 string.
/// @param target_currency - ISO-4217 currency of the returned P&L.
/// @returns The P&L as a `Money` handle in `targetCurrency`.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, kind `not_found`
/// if a curve, market item or FX leg is missing, and kind `computation` if
/// pricing fails.
#[wasm_bindgen(js_name = pnlBridge)]
pub fn pnl_bridge(
    instrument_json: JsValue,
    market_t0_json: JsValue,
    market_t1_json: JsValue,
    as_of_t0: JsValue,
    as_of_t1: JsValue,
    target_currency: JsValue,
) -> Result<crate::api::core::money::JsMoney, JsValue> {
    let instrument = parse_instrument(&instrument_json, "instrumentJson")?;
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

/// Parse a canonical instrument envelope (object or JSON) into its payload.
fn parse_instrument(
    value: &JsValue,
    label: &str,
) -> Result<finstack_quant_valuations::instruments::InstrumentJson, JsValue> {
    let envelope: finstack_quant_valuations::instruments::InstrumentEnvelope =
        serde_json::from_str(&json_text(value, label)?).map_err(to_js_err)?;
    Ok(envelope.instrument)
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
#[wasm_bindgen(js_name = attributePnlMany)]
pub fn attribute_pnl_many(inputs: JsValue, instruments: JsValue) -> Result<JsValue, JsValue> {
    let inputs: finstack_quant_attribution::AttributionInputs =
        crate::utils::input::from_js_json(&inputs, "inputs")?;
    let instruments: Vec<serde_json::Value> =
        crate::utils::input::from_js_json(&instruments, "instruments")?;
    let instruments = instruments
        .into_iter()
        .map(|value| {
            let envelope: finstack_quant_valuations::instruments::InstrumentEnvelope =
                match value {
                    serde_json::Value::String(json) => serde_json::from_str(&json),
                    object => serde_json::from_value(object),
                }
                .map_err(to_js_err)?;
            Ok(envelope.instrument)
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
#[wasm_bindgen(js_name = attributeReturnContribution)]
pub fn attribute_return_contribution(spec: JsValue) -> Result<JsValue, JsValue> {
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
/// @param spec_json - `ReturnContributionSpec` (object or JSON).
/// @returns Canonical `ReturnContributionResult` JSON text.
///
/// # Errors
///
/// Throws with kind `validation` if the spec is malformed or violates the
/// weighting/benchmark invariants.
#[wasm_bindgen(js_name = attributeReturnContributionJson)]
pub fn attribute_return_contribution_json(spec_json: JsValue) -> Result<String, JsValue> {
    finstack_quant_attribution::attribute_return_contribution_json(&json_text(
        &spec_json, "specJson",
    )?)
    .map_err(to_js_err)
}

/// Validate a return-contribution specification and return its canonical JSON.
///
/// Mirrors Python `validate_return_contribution_json`: the spec is parsed and
/// executed, so a spec that validates here also computes.
/// @param spec_json - `ReturnContributionSpec` (object or JSON).
/// @returns Canonical compact spec JSON.
///
/// # Errors
///
/// Throws with kind `validation` if the spec is malformed or violates the
/// weighting/benchmark invariants.
#[wasm_bindgen(js_name = validateReturnContributionJson)]
pub fn validate_return_contribution_json(spec_json: JsValue) -> Result<String, JsValue> {
    finstack_quant_attribution::validate_return_contribution_json(&json_text(
        &spec_json, "specJson",
    )?)
    .map_err(to_js_err)
}

fn parse_pnl(value: &JsValue) -> Result<finstack_quant_attribution::PnlAttribution, JsValue> {
    crate::utils::input::from_js_json(value, "pnl")
}

/// Human-readable tree explanation of an attribution (non-zero factors only).
///
/// Free-function twin of Python `PnlAttribution.explain` (Rust
/// `PnlAttribution::explain`).
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @returns Multi-line tree text.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`.
#[wasm_bindgen(js_name = pnlAttributionExplainText)]
pub fn pnl_attribution_explain(pnl: JsValue) -> Result<String, JsValue> {
    Ok(parse_pnl(&pnl)?.explain())
}

/// Verbose tree explanation of an attribution, including zero-valued factors.
///
/// Free-function twin of Python `PnlAttribution.explain_verbose` (Rust
/// `PnlAttribution::explain_verbose`).
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @returns Multi-line tree text.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`.
#[wasm_bindgen(js_name = pnlAttributionExplainVerboseText)]
pub fn pnl_attribution_explain_verbose(pnl: JsValue) -> Result<String, JsValue> {
    Ok(parse_pnl(&pnl)?.explain_verbose())
}

/// Share of total P&L, in percent, that an amount represents.
///
/// Free-function twin of Python `PnlAttribution.pct_of_total` (Rust
/// `PnlAttribution::pct_of_total`): `amount / total_pnl × 100`, with the
/// zero test on `total_pnl` taken from the run's rounding context.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @param amount - P&L amount in `total_pnl` currency units, normally one factor field (`carry`, `rates_curves_pnl`, `residual`, ...) or `total_pnl` itself.
/// @returns Signed percentage points (`25` = 25% of total P&L), or `undefined` when `total_pnl` is effectively zero and the share is undefined.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`, and kind
/// `invalid_type` if `amount` is not a number.
#[wasm_bindgen(js_name = pnlAttributionPctOfTotal)]
pub fn pnl_attribution_pct_of_total(pnl: JsValue, amount: JsValue) -> Result<Option<f64>, JsValue> {
    let pnl = parse_pnl(&pnl)?;
    Ok(pnl.pct_of_total(crate::utils::input::js_f64(&amount, "amount")?))
}

/// Whether the attribution residual is within tolerance.
///
/// Free-function twin of Python `PnlAttribution.residual_within_tolerance`
/// (Rust `PnlAttribution::residual_within_tolerance`): the tolerance is the
/// larger of `pctTolerance`% of |total P&L| and `absTolerance`; an
/// attribution flagged `result_invalid` is never within tolerance.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @param pct_tolerance - Optional percentage tolerance (`0.1` = 0.1%); omitted uses the run's `meta.tolerance_pct`.
/// @param abs_tolerance - Optional absolute tolerance in `total_pnl` currency units; omitted uses `meta.tolerance_abs`.
/// @returns `true` when the residual is within tolerance.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`, and kind
/// `invalid_type` if a tolerance is not a number.
#[wasm_bindgen(js_name = pnlAttributionResidualWithinTolerance)]
pub fn pnl_attribution_residual_within_tolerance(
    pnl: JsValue,
    pct_tolerance: Option<JsValue>,
    abs_tolerance: Option<JsValue>,
) -> Result<bool, JsValue> {
    let pnl = parse_pnl(&pnl)?;
    Ok(pnl.residual_within_tolerance(
        crate::utils::input::js_opt_f64(pct_tolerance.as_ref(), "pctTolerance")?,
        crate::utils::input::js_opt_f64(abs_tolerance.as_ref(), "absTolerance")?,
    ))
}

/// Check that every factor's currency matches the total P&L currency.
///
/// Free-function twin of Python `PnlAttribution.validate_currencies` (Rust
/// `PnlAttribution::validate_currencies`); run it before summing factors.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution` or a
/// factor is denominated in another currency.
#[wasm_bindgen(js_name = pnlAttributionValidateCurrencies)]
pub fn pnl_attribution_validate_currencies(pnl: JsValue) -> Result<(), JsValue> {
    parse_pnl(&pnl)?.validate_currencies().map_err(to_js_err)
}

/// Metric identifiers the attribution's method needs pre-computed.
///
/// Free-function twin of Python `PnlAttribution.required_metrics` (Rust
/// `AttributionMethod::required_metrics` of `meta.method`): the metrics-based
/// method lists its sensitivities; repricing methods return an empty array.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @returns Canonical metric identifiers.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`.
#[wasm_bindgen(js_name = pnlAttributionRequiredMetrics)]
pub fn pnl_attribution_required_metrics(pnl: JsValue) -> Result<Vec<String>, JsValue> {
    Ok(parse_pnl(&pnl)?
        .meta
        .method
        .required_metrics()
        .iter()
        .map(ToString::to_string)
        .collect())
}
