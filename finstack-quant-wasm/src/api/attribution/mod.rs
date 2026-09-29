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
//! the safe-integer ceiling. The [`crate::utils::check_js_safe_count`] guard
//! is therefore not wired in here; if a future getter exposes a raw `usize`
//! across the boundary, route it through that guard first.

use crate::utils::{structured_js_error, to_js_err};
use wasm_bindgen::prelude::*;

/// Parameters for P&L attribution via [`attribute_pnl`].
#[wasm_bindgen(js_name = AttributionParams)]
#[derive(Default)]
pub struct JsAttributionParams {
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

#[wasm_bindgen(js_class = AttributionParams)]
impl JsAttributionParams {
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
        instrument_json: String,
        market_t0_json: String,
        market_t1_json: String,
        as_of_t0: String,
        as_of_t1: String,
        method_json: String,
        config_json: Option<String>,
        full_cross_attribution: Option<bool>,
    ) -> Self {
        Self {
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
        }
    }

    /// Optional serialized opening `ModelParamsSnapshot`.
    ///
    /// # Arguments
    ///
    /// * `value` - JSON snapshot of T₀ model parameters, or omitted.
    ///
    /// @param value - Optional serialized opening ModelParamsSnapshot JSON.
    #[wasm_bindgen(setter, js_name = modelParamsT0Json)]
    pub fn set_model_params_t0_json(&mut self, value: Option<String>) {
        self.model_params_t0_json = value;
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
    pub fn set_credit_factor_model_json(&mut self, value: Option<String>) {
        self.credit_factor_model_json = value;
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

/// Map a `finstack_quant_core::Error` raised by attribution into a structured JS
/// error.
///
/// Mirrors the calibration binding's `envelope_error_to_js`: sets
/// `name = "AttributionError"`, attaches the variant name as `kind`, and the
/// full enum-serialized payload as `cause`. JS clients can pattern-match on
/// `err.kind` (e.g. `"Calibration"`, `"Validation"`, `"CurrencyMismatch"`,
/// `"Input"`) rather than parsing the human message.
///
/// JSON-parse errors during envelope deserialization fall back to a generic
/// `to_js_err` since they are not `finstack_quant_core::Error` instances.
fn attribution_error_to_js(err: finstack_quant_core::Error) -> JsValue {
    let message = err.to_string();
    let kind = error_variant_name(&err);
    let cause_json = serde_json::to_string(&err).ok();
    structured_js_error(
        "AttributionError",
        &message,
        Some(kind),
        cause_json.as_deref(),
    )
}

/// Return the externally-tagged variant name for a `finstack_quant_core::Error`.
/// Stable identifier suitable for JS clients to switch on (e.g.
/// `if (err.kind === "CurrencyMismatch") …`).
fn error_variant_name(err: &finstack_quant_core::Error) -> &'static str {
    use finstack_quant_core::Error as E;
    match err {
        E::Input(_) => "Input",
        E::CurrencyMismatch { .. } => "CurrencyMismatch",
        E::Calibration { .. } => "Calibration",
        E::Validation(_) => "Validation",
        E::UnknownMetric { .. } => "UnknownMetric",
        E::MetricNotApplicable { .. } => "MetricNotApplicable",
        E::MetricCalculationFailed { .. } => "MetricCalculationFailed",
        E::CircularDependency { .. } => "CircularDependency",
        E::Internal(_) => "Internal",
        // The Error enum is `#[non_exhaustive]`; future variants land here
        // until they are added above. The fallback keeps the binding
        // forward-compatible.
        _ => "Other",
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
    params: &JsAttributionParams,
) -> Result<finstack_quant_attribution::AttributionResult, JsValue> {
    let spec = finstack_quant_attribution::AttributionSpec::from_json_inputs(
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
    .map_err(attribution_error_to_js)?;
    spec.execute_contained().map_err(attribution_error_to_js)
}

/// Run P&L attribution for a single instrument.
///
/// Accepts a [`JsAttributionParams`] struct with the instrument JSON, two market
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
/// Rejects malformed instrument, market, method, or configuration JSON;
/// invalid ISO attribution dates; instrument or market reconstruction,
/// pricing, FX, rounding, metric, or method-specific attribution failures; a
/// caught attribution panic; or failure to convert the result to a
/// JavaScript value.
/// @param params - Fully specified AttributionParams object containing instrument, markets, dates, and method.
#[wasm_bindgen(js_name = attributePnl)]
pub fn attribute_pnl(params: &JsAttributionParams) -> Result<JsValue, JsValue> {
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
/// @param params - Fully specified AttributionParams object containing instrument, markets, dates, and method.
#[wasm_bindgen(js_name = attributePnlJson)]
pub fn attribute_pnl_json(params: &JsAttributionParams) -> Result<String, JsValue> {
    let result = run_attribute_pnl(params)?;
    serde_json::to_string(&result.attribution).map_err(to_js_err)
}

/// Run attribution from a full JSON `AttributionEnvelope` and return JSON.
///
/// Power-user variant for full envelope round-trip workflows.
///
/// # Errors
///
/// Rejects malformed, schema-incompatible, or unsupported-version `spec_json`;
/// instrument or market reconstruction, pricing, FX, rounding, metric, or
/// method-specific attribution failures; a caught execution panic; or
/// failure to serialize the result envelope.
/// @param spec_json - JSON-serialized AttributionParams specification to validate and execute.
#[wasm_bindgen(js_name = attributePnlEnvelopeJson)]
pub fn attribute_pnl_envelope_json(spec_json: &str) -> Result<String, JsValue> {
    let envelope: finstack_quant_attribution::AttributionEnvelope =
        serde_json::from_str(spec_json).map_err(to_js_err)?;
    let result_envelope = envelope
        .execute_contained()
        .map_err(attribution_error_to_js)?;
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
pub fn validate_attribution_json(json: &str) -> Result<String, JsValue> {
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
