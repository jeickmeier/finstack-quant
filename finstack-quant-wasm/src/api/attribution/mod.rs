//! WASM bindings for P&L attribution across multiple methodologies.
//!
//! # Number safety
//!
//! All counts and metrics (`num_repricings`, residuals, factor P&Ls) cross the
//! wasm boundary *inside* JSON strings, not as raw `usize`/`f64` values. JS's
//! `JSON.parse` reads those numbers as IEEE-754 doubles, so integer counts
//! above `Number.MAX_SAFE_INTEGER` (2^53 − 1) would silently round in the
//! consumer. Today every count in the attribution surface is bounded by a
//! handful of factors (≤ 12) and a handful of repricings (≤ ~30), well under
//! the safe-integer ceiling.

use crate::utils::input::{js_opt_bool, js_string, json_text, opt_json_text};
use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Owned JSON fragments for P&L attribution via [`attribute_pnl`].
///
/// Holds the fields of Rust `AttributionJsonInputs`, which `attributePnl`
/// passes to `AttributionSpec::from_json_inputs`. JavaScript has no keyword
/// arguments, so the inputs are bundled in this class; Python passes the same
/// fields as keyword arguments of `attribute_pnl`.
#[wasm_bindgen(js_name = AttributionJsonInputs)]
#[derive(Default)]
pub struct JsAttributionJsonInputs {
    instrument_json: String,
    market_t0_json: String,
    market_t1_json: String,
    as_of_t0: String,
    as_of_t1: String,
    method_json: String,
    config_json: Option<String>,
    full_cross_attribution: Option<bool>,
    model_params_t0_json: Option<String>,
    credit_factor_model_json: Option<String>,
}

#[wasm_bindgen(js_class = AttributionJsonInputs)]
impl JsAttributionJsonInputs {
    /// Bundle the attribution inputs (instrument / markets / dates / method
    /// JSON strings plus optional config and full-cross flag) for
    /// `attributePnl`. Attach a T₀ model-parameter snapshot or credit-factor
    /// model with the optional setters after construction.
    ///
    /// # Arguments
    ///
    /// * `instrument_json` - Canonical v1 instrument envelope JSON.
    /// * `market_t0_json` - Canonical MarketContext JSON at T₀.
    /// * `market_t1_json` - Canonical MarketContext JSON at T₁.
    /// * `as_of_t0` - ISO-8601 valuation date for the start snapshot.
    /// * `as_of_t1` - ISO-8601 valuation date for the end snapshot.
    /// * `method_json` - Snake-case serialized attribution method.
    /// * `config_json` - Optional complete attribution configuration JSON.
    /// * `full_cross_attribution` - When `Some(true)`, evaluate every pairwise
    ///   cross-factor term.
    ///
    /// @param instrument_json - Canonical instrument envelope JSON in the Finstack v1 schema.
    /// @param market_t0_json - Canonical MarketContext JSON at the attribution start date.
    /// @param market_t1_json - Canonical MarketContext JSON at the attribution end date.
    /// @param as_of_t0 - ISO-8601 valuation date for the start market snapshot.
    /// @param as_of_t1 - ISO-8601 valuation date for the end market snapshot.
    /// @param method_json - Attribution-method configuration JSON selecting the P-and-L decomposition.
    /// @param config_json - Optional attribution configuration JSON controlling calculation settings.
    /// @param full_cross_attribution - Whether to calculate all pairwise cross-factor attribution terms.
    #[wasm_bindgen(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument_json: JsValue,
        market_t0_json: JsValue,
        market_t1_json: JsValue,
        as_of_t0: JsValue,
        as_of_t1: JsValue,
        method_json: JsValue,
        config_json: Option<JsValue>,
        full_cross_attribution: Option<JsValue>,
    ) -> Result<Self, JsValue> {
        let instrument_json = json_text(&instrument_json, "instrumentJson")?;
        let market_t0_json = json_text(&market_t0_json, "marketT0Json")?;
        let market_t1_json = json_text(&market_t1_json, "marketT1Json")?;
        let as_of_t0 = js_string(&as_of_t0, "asOfT0")?;
        let as_of_t1 = js_string(&as_of_t1, "asOfT1")?;
        let method_json = json_text(&method_json, "methodJson")?;
        let config_json = opt_json_text(config_json.as_ref(), "configJson")?;
        let full_cross_attribution =
            js_opt_bool(full_cross_attribution.as_ref(), "fullCrossAttribution")?;
        Ok(Self {
            instrument_json,
            market_t0_json,
            market_t1_json,
            as_of_t0,
            as_of_t1,
            method_json,
            config_json,
            full_cross_attribution,
            model_params_t0_json: None,
            credit_factor_model_json: None,
        })
    }

    /// Optional serialized opening `ModelParamsSnapshot`.
    ///
    /// # Arguments
    ///
    /// * `value` - JSON snapshot of T₀ model parameters, or omitted.
    ///
    /// @param value - Optional serialized opening ModelParamsSnapshot JSON.
    #[wasm_bindgen(setter, js_name = modelParamsT0Json)]
    pub fn set_model_params_t0_json(&mut self, value: Option<JsValue>) -> Result<(), JsValue> {
        self.model_params_t0_json = opt_json_text(value.as_ref(), "modelParamsT0Json")?;
        Ok(())
    }

    /// Optional serialized opening `ModelParamsSnapshot`.
    ///
    /// # Returns
    ///
    /// The JSON snapshot attached after construction, or omitted.
    ///
    /// @returns The JSON snapshot attached after construction, or omitted.
    #[wasm_bindgen(getter, js_name = modelParamsT0Json)]
    pub fn model_params_t0_json(&self) -> Option<String> {
        self.model_params_t0_json.clone()
    }

    /// Optional serialized `CreditFactorModel`.
    ///
    /// # Arguments
    ///
    /// * `value` - JSON credit-factor model, or omitted.
    ///
    /// @param value - Optional serialized CreditFactorModel JSON.
    #[wasm_bindgen(setter, js_name = creditFactorModelJson)]
    pub fn set_credit_factor_model_json(&mut self, value: Option<JsValue>) -> Result<(), JsValue> {
        self.credit_factor_model_json = opt_json_text(value.as_ref(), "creditFactorModelJson")?;
        Ok(())
    }

    /// Optional serialized `CreditFactorModel`.
    ///
    /// # Returns
    ///
    /// The JSON credit-factor model attached after construction, or omitted.
    ///
    /// @returns The JSON credit-factor model attached after construction, or omitted.
    #[wasm_bindgen(getter, js_name = creditFactorModelJson)]
    pub fn credit_factor_model_json(&self) -> Option<String> {
        self.credit_factor_model_json.clone()
    }
}

/// Parse and execute one attribution request.
///
/// Shared by [`attribute_pnl`] and [`attribute_pnl_json`], which differ only
/// in how they hand the `PnlAttribution` back across the boundary. Execution
/// goes through `execute_contained`, which turns a Rust panic into
/// `Error::Internal`: an uncaught unwind at the wasm boundary would abort the
/// module instance and kill every subsequent call from the JS host.
fn run_attribute_pnl(
    params: &JsAttributionJsonInputs,
) -> Result<finstack_quant_attribution::AttributionResult, JsValue> {
    attribution_spec(params)?
        .execute_contained()
        .map_err(to_js_err)
}

/// Build the Rust `AttributionSpec` from the bundled JSON inputs.
fn attribution_spec(
    params: &JsAttributionJsonInputs,
) -> Result<finstack_quant_attribution::AttributionSpec, JsValue> {
    finstack_quant_attribution::AttributionSpec::from_json_inputs(
        finstack_quant_attribution::AttributionJsonInputs {
            instrument_json: &params.instrument_json,
            market_t0_json: &params.market_t0_json,
            market_t1_json: &params.market_t1_json,
            as_of_t0: &params.as_of_t0,
            as_of_t1: &params.as_of_t1,
            method_json: &params.method_json,
            config_json: params.config_json.as_deref(),
            model_params_t0_json: params.model_params_t0_json.as_deref(),
            credit_factor_model_json: params.credit_factor_model_json.as_deref(),
            full_cross_attribution: params.full_cross_attribution.unwrap_or(false),
        },
    )
    .map_err(to_js_err)
}

/// Run P&L attribution for a single instrument.
///
/// Accepts a [`JsAttributionJsonInputs`] object with the instrument JSON, two market
/// snapshots, dates, and a method descriptor. Returns the `PnlAttribution`
/// result as a structured JavaScript object whose fields carry the canonical
/// Rust serde names (`total_pnl.amount`, `carry`, `meta`, ...); use
/// [`attribute_pnl_json`] for the JSON wire string. `config_json` may include
/// `"execution_policy": "parallel"` to opt into inner Rayon when the host
/// is not already parallelizing attribution at a higher level. Serial is
/// the default.
///
/// # Errors
///
/// Throws a `FinstackError` whose `kind` is the Rust classification
/// (`not_found` for missing market data, `computation` for a caught panic or
/// solver failure, otherwise `validation`). Rejects malformed instrument,
/// market, method, or configuration JSON;
/// invalid ISO attribution dates; instrument or market reconstruction,
/// pricing, FX, rounding, metric, or method-specific attribution failures; a
/// caught attribution panic; or failure to convert the result to a
/// JavaScript value.
/// @param params - Fully specified AttributionJsonInputs object containing instrument, markets, dates, and method.
#[wasm_bindgen(js_name = attributePnl)]
pub fn attribute_pnl(params: &JsAttributionJsonInputs) -> Result<JsValue, JsValue> {
    let result = run_attribute_pnl(params)?;
    crate::utils::to_js_value(&result.attribution)
}

/// Run P&L attribution for a single instrument and return wire JSON.
///
/// Wire twin of [`attribute_pnl`]: same inputs, validation, and panic
/// containment, returning the `PnlAttribution` as a JSON string instead of a
/// structured object.
///
/// # Errors
///
/// Rejects the same conditions as [`attribute_pnl`], plus failure to
/// serialize the result to JSON.
/// @param params - Fully specified AttributionJsonInputs object containing instrument, markets, dates, and method.
#[wasm_bindgen(js_name = attributePnlJson)]
pub fn attribute_pnl_json(params: &JsAttributionJsonInputs) -> Result<String, JsValue> {
    let result = run_attribute_pnl(params)?;
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
/// Rejects the same conditions as [`attribute_pnl_envelope`], plus failure to
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

/// Run one attribution configuration against many instruments.
///
/// Mirrors Python `attribute_pnl_many` (Rust `attribute_pnl_many`): every
/// instrument is attributed with the markets, dates, method and configuration
/// in `params` (whose own `instrumentJson` is replaced by each entry of
/// `instruments`). Results come back in input order; the first failing
/// instrument aborts the batch. Python returns the same attributions as a
/// wide DataFrame.
/// @param params - AttributionJsonInputs carrying the shared markets, dates, method and configuration.
/// @param instruments - Array of canonical instrument envelopes (objects or JSON), in output order.
/// @returns One `PnlAttribution` object per instrument, in input order.
///
/// # Errors
///
/// Throws a `FinstackError` with the Rust classification for the first
/// failing instrument (see `attributePnl`), or kind `validation` if an
/// instrument envelope is malformed.
#[wasm_bindgen(js_name = attributePnlMany)]
pub fn attribute_pnl_many(
    params: &JsAttributionJsonInputs,
    instruments: JsValue,
) -> Result<JsValue, JsValue> {
    let instruments: Vec<serde_json::Value> =
        crate::utils::input::from_js_json(&instruments, "instruments")?;
    let instruments = instruments
        .into_iter()
        .map(|value| {
            serde_json::from_value::<finstack_quant_valuations::instruments::InstrumentEnvelope>(
                value,
            )
            .map(|envelope| envelope.instrument)
            .map_err(to_js_err)
        })
        .collect::<Result<Vec<_>, JsValue>>()?;
    let template = attribution_spec(params)?;
    let attributions = finstack_quant_attribution::attribute_pnl_many(&template, instruments)
        .map_err(to_js_err)?;
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
