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
#[wasm_bindgen]
#[derive(Default)]
pub struct AttributionParams {
    instrument_json: String,
    market_t0_json: String,
    market_t1_json: String,
    as_of_t0: String,
    as_of_t1: String,
    method_json: String,
    config_json: Option<String>,
    full_cross_attribution: Option<bool>,
}

#[wasm_bindgen]
impl AttributionParams {
    #[wasm_bindgen(constructor)]
    #[allow(clippy::too_many_arguments)]
    /// Bundle the attribution inputs (instrument / markets / dates / method
    /// JSON strings plus optional config and full-cross flag) for
    /// `attributePnl`.
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
        }
    }
}

/// Map a `finstack_core::Error` raised by attribution into a structured JS
/// error.
///
/// Mirrors the calibration binding's `envelope_error_to_js`: sets
/// `name = "AttributionError"`, attaches the variant name as `kind`, and the
/// full enum-serialized payload as `cause`. JS clients can pattern-match on
/// `err.kind` (e.g. `"Calibration"`, `"Validation"`, `"CurrencyMismatch"`,
/// `"Input"`) rather than parsing the human message.
///
/// JSON-parse errors during envelope deserialization fall back to a generic
/// `to_js_err` since they are not `finstack_core::Error` instances.
fn attribution_error_to_js(err: finstack_core::Error) -> JsValue {
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

/// Return the externally-tagged variant name for a `finstack_core::Error`.
/// Stable identifier suitable for JS clients to switch on (e.g.
/// `if (err.kind === "CurrencyMismatch") …`).
fn error_variant_name(err: &finstack_core::Error) -> &'static str {
    use finstack_core::Error as E;
    match err {
        E::Input(_) => "Input",
        E::InterpOutOfBounds => "InterpOutOfBounds",
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

/// Extract a human-readable message from a caught panic payload.
fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

/// Run an attribution `execute()` call, converting a Rust panic into a
/// catchable `AttributionError` `JsValue` instead of letting it unwind to the
/// wasm boundary. An uncaught unwind there `abort`s the whole module instance,
/// killing every subsequent call from the JS host.
fn catch_attribution_panic<T>(
    label: &str,
    f: impl FnOnce() -> Result<T, finstack_core::Error>,
) -> Result<T, JsValue> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(attribution_error_to_js(err)),
        Err(panic) => Err(attribution_error_to_js(finstack_core::Error::internal(
            format!(
                "attribution panicked in {label}: {}",
                panic_message(panic.as_ref())
            ),
        ))),
    }
}

/// Run P&L attribution for a single instrument.
///
/// Accepts an [`AttributionParams`] struct with the instrument JSON, two market
/// snapshots, dates, and a method descriptor. Returns the `PnlAttribution`
/// result as JSON. `config_json` may include `"execution_policy": "serial"`
/// for hosts that already parallelize attribution at a higher level.
#[wasm_bindgen(js_name = attributePnl)]
pub fn attribute_pnl(params: &AttributionParams) -> Result<String, JsValue> {
    // MI3 defense in depth: wrap input-parsing as well. `from_json_inputs`
    // funnels through serde + downstream constructors that should not panic,
    // but a deeply malformed payload could in principle. An uncaught unwind
    // at the wasm boundary aborts the whole module instance, killing every
    // subsequent call from the JS host.
    let mut spec = catch_attribution_panic("attributePnl/from_json_inputs", || {
        finstack_attribution::AttributionSpec::from_json_inputs(
            &params.instrument_json,
            &params.market_t0_json,
            &params.market_t1_json,
            &params.as_of_t0,
            &params.as_of_t1,
            &params.method_json,
            params.config_json.as_deref(),
        )
    })?;
    if let Some(val) = params.full_cross_attribution {
        spec.full_cross_attribution = val;
    }
    let result = catch_attribution_panic("attributePnl", || spec.execute())?;
    serde_json::to_string(&result.attribution).map_err(to_js_err)
}

/// Run attribution from a full JSON `AttributionEnvelope` and return JSON.
///
/// Power-user variant for full envelope round-trip workflows.
#[wasm_bindgen(js_name = attributePnlFromSpec)]
pub fn attribute_pnl_from_spec(spec_json: &str) -> Result<String, JsValue> {
    // MI3: wrap serde_json parse too. A JSON-parse panic would otherwise abort
    // the wasm module instance.
    let envelope = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        serde_json::from_str::<finstack_attribution::AttributionEnvelope>(spec_json)
    })) {
        Ok(Ok(envelope)) => envelope,
        Ok(Err(err)) => return Err(to_js_err(err)),
        Err(panic) => {
            return Err(attribution_error_to_js(finstack_core::Error::Validation(
                format!(
                    "attributePnlFromSpec panicked while parsing envelope JSON: {}",
                    panic_message(panic.as_ref())
                ),
            )));
        }
    };
    let result_envelope = catch_attribution_panic("attributePnlFromSpec", || envelope.execute())?;
    serde_json::to_string(&result_envelope).map_err(to_js_err)
}

/// Validate an attribution specification JSON.
///
/// Deserializes against the `AttributionEnvelope` schema, checks the
/// `schema` version tag (the same gate `execute` applies, so a payload that
/// validates here cannot later be rejected at execution), and returns the
/// canonical JSON.
#[wasm_bindgen(js_name = validateAttributionJson)]
pub fn validate_attribution_json(json: &str) -> Result<String, JsValue> {
    let envelope: finstack_attribution::AttributionEnvelope =
        serde_json::from_str(json).map_err(to_js_err)?;
    if envelope.schema != finstack_attribution::ATTRIBUTION_SCHEMA_V1 {
        return Err(JsValue::from_str(&format!(
            "unsupported attribution schema {:?}; expected {:?}",
            envelope.schema,
            finstack_attribution::ATTRIBUTION_SCHEMA_V1
        )));
    }
    serde_json::to_string(&envelope).map_err(to_js_err)
}

/// Return the default waterfall factor ordering as a JSON array.
#[wasm_bindgen(js_name = defaultWaterfallOrder)]
pub fn default_waterfall_order() -> Result<JsValue, JsValue> {
    let factors: Vec<String> = finstack_attribution::default_waterfall_order()
        .into_iter()
        .map(|f| f.to_string())
        .collect();
    serde_wasm_bindgen::to_value(&factors).map_err(to_js_err)
}

/// Return the default metric IDs used by metrics-based attribution.
#[wasm_bindgen(js_name = defaultAttributionMetrics)]
pub fn default_attribution_metrics() -> Result<JsValue, JsValue> {
    let metrics: Vec<String> = finstack_attribution::default_attribution_metrics()
        .into_iter()
        .map(|m| m.to_string())
        .collect();
    serde_wasm_bindgen::to_value(&metrics).map_err(to_js_err)
}
