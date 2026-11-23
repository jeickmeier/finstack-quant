//! Tagged-instrument JSON parsing, validation, and pricing dispatch.

use super::super::{ModelKey, STANDARD, parse_model_key};
use super::overrides::*;
use crate::instruments::Instrument;
use crate::instruments::envelope::{MAX_JSON_BYTES, instrument_load_error};
use crate::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
#[cfg(feature = "json-schema")]
use crate::instruments::pricing_overrides::{InstrumentPricingOverrides, MetricPricingOverrides};
use crate::metrics::MetricId;
use crate::metrics::risk::MarketHistory;
use crate::results::ValuationResult;
use finstack_quant_core::Error;
use finstack_quant_core::contract::{LoadLimits, parse_json_value};
use finstack_quant_core::market_data::context::MarketContext;
use serde_json::Value;
use std::sync::Arc;

/// Standard option Greek metric IDs exposed by host-language option wrappers.
pub const STANDARD_OPTION_GREEKS: &[&str] = &[
    "delta",
    "gamma",
    "vega",
    "theta",
    "rho",
    "foreign_rho",
    "vanna",
    "volga",
];

/// Instrument parsed and validated at a host-language request boundary.
///
/// This handle preserves proof that validation already ran before market
/// extraction. Parsed pricing cores accept it so registry dispatch can avoid
/// repeating validation while ordinary Rust pricing routes remain guarded.
pub struct ParsedInstrument {
    instrument: Box<dyn Instrument>,
}

impl ParsedInstrument {
    /// Prepare a typed instrument for host pricing, merging overrides before validation.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Owned instrument in its native Rust representation.
    /// * `metric_pricing_overrides` - Optional JSON object patch. Omitted fields
    ///   retain stored controls, nested bump fields merge, and explicit null
    ///   clears optional controls. Rates and bumps use the canonical Rust units.
    ///
    /// * `instrument_pricing_overrides` - Optional instrument-owned JSON patch.
    ///   Supplied market quotes and model controls replace stored values for this
    ///   request only, using canonical instrument units.
    ///
    /// # Errors
    ///
    /// Returns a validation error for malformed overrides, unsupported controls,
    /// or invalid instrument state after merging.
    pub fn from_instrument(
        mut instrument: Box<dyn Instrument>,
        metric_pricing_overrides: Option<&str>,
        instrument_pricing_overrides: Option<&Value>,
    ) -> finstack_quant_core::Result<Self> {
        if let Some(patch) = metric_pricing_overrides {
            apply_metric_pricing_overrides(instrument.as_mut(), patch)?;
        }
        if let Some(patch) = instrument_pricing_overrides {
            apply_instrument_pricing_overrides(instrument.as_mut(), patch)?;
        }
        instrument.validate_for_pricing()?;
        Ok(Self { instrument })
    }

    /// Prepare a canonical instrument payload for host pricing without a JSON
    /// round trip, merging overrides before validation.
    ///
    /// Equivalent to [`parse_boxed_instrument_from_json`] on the payload's
    /// envelope, for bindings that already hold the native instrument.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Owned tagged instrument payload, boxed without
    ///   validation so `metric_pricing_overrides` can repair stored controls.
    /// * `metric_pricing_overrides` - Optional JSON object patch with the same
    ///   merge semantics as [`Self::from_instrument`].
    ///
    /// * `instrument_pricing_overrides` - Optional instrument-owned JSON patch.
    ///   Supplied market quotes and model controls replace stored values for this
    ///   request only, using canonical instrument units.
    ///
    /// # Errors
    ///
    /// Returns a validation error for malformed overrides, unsupported controls,
    /// or invalid instrument state after merging.
    pub fn from_instrument_json(
        instrument: InstrumentJson,
        metric_pricing_overrides: Option<&str>,
        instrument_pricing_overrides: Option<&Value>,
    ) -> finstack_quant_core::Result<Self> {
        Self::from_instrument(
            instrument.into_boxed_unchecked(),
            metric_pricing_overrides,
            instrument_pricing_overrides,
        )
    }

    /// Borrow the validated instrument trait object.
    #[must_use]
    pub fn as_instrument(&self) -> &dyn Instrument {
        self.instrument.as_ref()
    }

    /// Consume this validation handle and return the boxed instrument.
    #[must_use]
    pub fn into_boxed(self) -> Box<dyn Instrument> {
        self.instrument
    }
}

/// Parse a canonical instrument envelope into the canonical Rust enum.
///
/// Input must use the canonical versioned [`InstrumentEnvelope`] form. Its size
/// is capped at [`MAX_JSON_BYTES`], and the concrete instrument is validated
/// before the enum is returned.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON containing the required v1 envelope and a recognized
///   instrument `type` tag.
///
/// # Errors
///
/// Returns `Error::Validation` when `json` exceeds the size cap, is malformed,
/// carries an unsupported envelope schema, does not match a supported tagged
/// instrument shape, or fails domain validation.
pub fn parse_instrument_from_json(json: &str) -> finstack_quant_core::Result<InstrumentJson> {
    let envelope = InstrumentEnvelope::decode_bytes(
        json.as_bytes(),
        &LoadLimits::default().with_max_bytes(MAX_JSON_BYTES),
    )
    .map_err(instrument_load_error)?;
    let instrument = envelope.instrument;
    instrument.validate_for_pricing()?;
    Ok(instrument)
}

/// Build and validate an instrument payload from a bare spec object.
///
/// # Arguments
///
/// * `type_tag` - Canonical instrument discriminator expected by the caller's
///   API route, such as `"fx_option"`.
/// * `spec` - Bare instrument spec object for `type_tag`. Tagged payloads and
///   envelopes are rejected.
///
/// # Returns
///
/// The tagged [`InstrumentJson`] payload after type-specific deserialization
/// and validation has succeeded. Convert it to the concrete Rust type with
/// `TryFrom`.
///
/// # Errors
///
/// Returns `Error::Validation` when `spec` is not a bare object, does not
/// deserialize as a `type_tag` instrument, or fails instrument validation.
pub fn instrument_from_spec(
    type_tag: &str,
    spec: Value,
) -> finstack_quant_core::Result<InstrumentJson> {
    let object = spec.as_object().ok_or_else(|| {
        Error::Validation("instrument constructor requires a bare spec object".to_string())
    })?;
    if (object.contains_key("type") && object.contains_key("spec"))
        || (object.contains_key("schema") && object.contains_key("instrument"))
    {
        return Err(Error::Validation(
            "instrument constructor requires a bare spec object, not a tagged payload or envelope"
                .to_string(),
        ));
    }

    let instrument: InstrumentJson = serde_json::from_value(serde_json::json!({
        "type": type_tag,
        "spec": spec,
    }))
    .map_err(|error| Error::Validation(format!("invalid {type_tag} instrument spec: {error}")))?;
    instrument.validate_for_pricing()?;
    Ok(instrument)
}

/// Build and validate a canonical instrument envelope from a bare spec object.
///
/// # Arguments
///
/// * `type_tag` - Canonical instrument discriminator expected by the caller's
///   API route.
/// * `spec` - Bare instrument spec object for `type_tag`. Tagged payloads and
///   envelopes are rejected.
///
/// # Returns
///
/// The canonical v1 [`InstrumentEnvelope`] after type-specific deserialization
/// and validation has succeeded. Serialize it with `serde_json` when the wire
/// text is needed.
///
/// # Errors
///
/// Returns `Error::Validation` when `spec` is not a bare object, does not
/// deserialize as a `type_tag` instrument, or fails instrument validation.
pub fn instrument_envelope_from_spec(
    type_tag: &str,
    spec: Value,
) -> finstack_quant_core::Result<InstrumentEnvelope> {
    instrument_from_spec(type_tag, spec).map(InstrumentEnvelope::new)
}

/// Parse a canonical envelope into one concrete instrument type.
///
/// This is the typed twin of [`validate_typed_instrument_json`]: the envelope
/// is parsed and validated by [`parse_instrument_from_json`], then converted
/// with the registry-generated `TryFrom<InstrumentJson>` impl of `T`.
///
/// # Arguments
///
/// * `json` - Required canonical v1 instrument envelope whose `type` must be
///   the discriminator of `T` (for example `"bond"` for
///   [`crate::instruments::fixed_income::bond::Bond`]).
///
/// # Errors
///
/// Returns `Error::Validation` when `json` is malformed, fails instrument
/// validation, or carries another instrument type
/// (``expected instrument type `bond`, got `term_loan` ``).
pub fn parse_typed_instrument_json<T>(json: &str) -> finstack_quant_core::Result<T>
where
    T: TryFrom<InstrumentJson, Error = Error>,
{
    T::try_from(parse_instrument_from_json(json)?)
}

/// Validate a canonical envelope for one exact instrument type.
///
/// # Arguments
///
/// * `type_tag` - Canonical instrument discriminator expected by the caller.
/// * `json` - Required canonical v1 instrument envelope.
///
/// # Errors
///
/// Returns `Error::Validation` when `json` is malformed, uses another
/// instrument type, fails validation, or cannot be canonically serialized.
pub fn validate_typed_instrument_json(
    type_tag: &str,
    json: &str,
) -> finstack_quant_core::Result<String> {
    let instrument = parse_instrument_from_json(json)?;
    let actual = instrument.type_tag();
    if actual != type_tag {
        return Err(crate::instruments::json_loader::instrument_type_mismatch(
            type_tag, actual,
        ));
    }
    serde_json::to_string(&InstrumentEnvelope::new(instrument))
        .map_err(|error| Error::Validation(format!("invalid instrument JSON: {error}")))
}

/// Validate a v1 instrument envelope against the pricing contract and return
/// its canonical JSON representation.
///
/// # Arguments
///
/// * `json` - Required canonical v1 instrument envelope.
/// * `metric_pricing_overrides` - Optional `MetricPricingOverrides` JSON merged
///   into the instrument’s stored metric controls by the
///   canonical pricing path before instrument validation; `None` retains the
///   envelope configuration. Only supplied fields replace stored values;
///   omitted fields, including individual `bump_config` fields, are retained.
///   Explicit `null` clears an optional field to its default behavior.
///
/// # Errors
///
/// Returns `Error::Validation` when parsing, metric-pricing override merging,
/// instrument validation, or canonical serialization fails.
#[tracing::instrument(
    name = "valuations.json.validate_instrument",
    level = "debug",
    skip(json, metric_pricing_overrides),
    fields(
        json_bytes = json.len(),
        has_metric_overrides = metric_pricing_overrides.is_some(),
    ),
    err,
)]
pub fn validate_instrument_json(
    json: &str,
    metric_pricing_overrides: Option<&str>,
) -> finstack_quant_core::Result<String> {
    let envelope = decode_envelope_with_metric_overrides(json, metric_pricing_overrides)?;
    envelope.instrument.validate_for_pricing()?;
    serde_json::to_string(&envelope)
        .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))
}

/// Parse a canonical instrument envelope, optionally merge metric pricing
/// overrides, and return a validated handle for pricing dispatch.
///
/// # Arguments
///
/// * `instrument_json` - Required canonical v1 instrument envelope.
/// * `metric_pricing_overrides` - Optional `MetricPricingOverrides` JSON merged
///   into the instrument’s stored metric controls before validation. Only
///   supplied fields replace stored values; omitted fields, including individual
///   `bump_config` fields, are retained. Explicit `null` clears optional fields.
///
/// * `instrument_pricing_overrides` - Optional instrument-owned JSON patch.
///   Supplied market quotes and model controls replace stored values for this
///   request only, using canonical instrument units.
///
/// # Errors
///
/// Returns `Error::Validation` when either JSON value is malformed, the
/// override cannot be merged, or the resulting instrument is invalid.
#[tracing::instrument(
    name = "valuations.json.parse_boxed_instrument",
    level = "debug",
    skip(instrument_json, metric_pricing_overrides, instrument_pricing_overrides),
    fields(
        json_bytes = instrument_json.len(),
        has_metric_overrides = metric_pricing_overrides.is_some(),
        has_instrument_overrides = instrument_pricing_overrides.is_some(),
    ),
    err,
)]
pub fn parse_boxed_instrument_from_json(
    instrument_json: &str,
    metric_pricing_overrides: Option<&str>,
    instrument_pricing_overrides: Option<&Value>,
) -> finstack_quant_core::Result<ParsedInstrument> {
    let envelope =
        decode_envelope_with_metric_overrides(instrument_json, metric_pricing_overrides)?;
    ParsedInstrument::from_instrument_json(envelope.instrument, None, instrument_pricing_overrides)
}

/// Parse a JSON array of canonical instrument envelopes into validated
/// instruments, in input order.
///
/// This is the inventory counterpart of [`parse_boxed_instrument_from_json`]:
/// the whole array is capped at [`MAX_JSON_BYTES`], parse failures carry the
/// same `invalid instrument envelope JSON` prefix, and every instrument is
/// validated before it is boxed.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON array whose elements are canonical v1 instrument
///   envelopes (`{"schema": ..., "instrument": {...}}`). An empty array yields
///   an empty inventory.
///
/// # Errors
///
/// Returns `Error::Validation` when `json` exceeds the size cap, is not an
/// array of well-formed envelopes, or any instrument fails domain validation.
#[tracing::instrument(
    name = "valuations.json.parse_boxed_instruments",
    level = "debug",
    skip(json),
    fields(json_bytes = json.len()),
    err,
)]
pub fn parse_boxed_instruments_from_json(
    json: &str,
) -> finstack_quant_core::Result<Vec<Box<dyn Instrument>>> {
    let limits = LoadLimits::default().with_max_bytes(MAX_JSON_BYTES);
    let value = parse_json_value(json.as_bytes(), &limits).map_err(instrument_load_error)?;
    let Value::Array(values) = value else {
        return Err(Error::Validation(
            "invalid instrument envelope JSON: expected array".into(),
        ));
    };
    values
        .into_iter()
        .map(|value| {
            InstrumentEnvelope::decode_value(value, &limits)
                .map_err(instrument_load_error)?
                .into_boxed()
        })
        .collect()
}

/// Pretty-print JSON for inspection-oriented binding APIs.
///
/// This reformats arbitrary valid JSON; it does not validate that the value is
/// an instrument payload. Use [`validate_instrument_json`] when the caller also
/// needs pricing-contract validation.
///
/// # Errors
///
/// Returns `Error::Validation` when `json` is malformed or cannot be rendered
/// as a JSON string.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON text to parse and reserialize with canonical
///   indentation; it need not be an instrument envelope.
pub fn pretty_instrument_json(json: &str) -> finstack_quant_core::Result<String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))?;
    serde_json::to_string_pretty(&value)
        .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))
}

pub(super) fn resolve_model_key(
    instrument: &dyn Instrument,
    model: &str,
) -> finstack_quant_core::Result<ModelKey> {
    if model == "default" {
        Ok(instrument.default_model())
    } else {
        parse_model_key(model)
    }
}

/// The rarer inputs of a host pricing call, read from one object.
///
/// Python `price_instrument(..., extras=...)`, WASM `priceInstrument(...,
/// extras)` and every typed instrument's `price` take this beside `model` and
/// `metrics`. The bindings read it once and route its fields to two places:
/// Both pricing override patches merge into the instrument when it is prepared
/// ([`ParsedInstrument::from_instrument`] / [`parse_boxed_instrument_from_json`]),
/// while `market_history` and `explain` apply at the [`price_instrument`] call.
/// The bindings map `explain` onto
/// [`PricingOptions::with_explain`](crate::instruments::PricingOptions::with_explain)
/// together with the calibration crate's recalibration provider.
///
/// Every field is optional; `PriceExtras::default()` (or an empty object)
/// prices with the instrument's stored overrides, no market history and no
/// step trace.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::pricer::PriceExtras;
///
/// let extras: PriceExtras = serde_json::from_str(
///     r#"{"metric_pricing_overrides": {"theta_period": {"count": 1, "unit": "weeks"}}, "explain": true}"#,
/// )?;
/// assert!(extras.explain);
/// assert!(extras.market_history.is_none());
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PriceExtras {
    /// JSON object patch merged into the instrument’s stored metric controls
    /// before validation, in the shape of [`MetricPricingOverrides`]. Only the
    /// keys present replace stored controls (an explicit `null` clears an
    /// optional control) and `bump_config` keys merge into the stored bump
    /// configuration. The field holds the raw patch rather than the typed
    /// struct because key presence decides what is replaced. `None` keeps
    /// the instrument's stored overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<MetricPricingOverrides>")
    )]
    pub metric_pricing_overrides: Option<Value>,
    /// Call-time JSON object patch for instrument-owned market quotes and model
    /// controls. Only supplied fields replace stored values. Setting a nullable
    /// leaf to null clears it, while omitted leaves keep their original values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<InstrumentPricingOverrides>")
    )]
    pub instrument_pricing_overrides: Option<Value>,
    /// Historical market scenarios for the historical risk metrics (`hvar`,
    /// `expected_shortfall`). `None` prices without history; requesting
    /// those metrics then fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_history: Option<MarketHistory>,
    /// Per-request configuration for metric settings and monetary conventions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<serde_json::Map<String, Value>>")
    )]
    pub config: Option<finstack_quant_core::config::FinstackConfig>,
    /// When `true`, attaches a step trace to the result as `explanation`.
    /// Defaults to `false` (no trace).
    #[serde(default)]
    pub explain: bool,
}

impl PriceExtras {
    /// Configure valuation and metric calculation for this request.
    ///
    /// # Arguments
    ///
    /// * `options` - Host pricing services, including any recalibration provider.
    ///   Supplied configuration, history and the explanation flag are applied
    ///   for this request.
    pub fn pricing_options(
        &self,
        mut options: crate::instruments::PricingOptions,
    ) -> crate::instruments::PricingOptions {
        if let Some(config) = &self.config {
            options = options.with_config(config);
        }
        if let Some(history) = &self.market_history {
            options = options.with_market_history(Arc::new(history.clone()));
        }
        if self.explain {
            options = options.with_explain(finstack_quant_core::explain::ExplainOpts::enabled());
        }
        options
    }

    /// The metric-pricing override patch as JSON text, for
    /// [`ParsedInstrument::from_instrument`] and [`parse_boxed_instrument_from_json`].
    ///
    /// Returns `None` when no patch is set.
    #[must_use]
    pub fn metric_pricing_overrides_json(&self) -> Option<String> {
        self.metric_pricing_overrides.as_ref().map(Value::to_string)
    }
}

/// Price an already parsed and validated instrument using the caller-selected
/// registry or the shared standard registry.
///
/// Host bindings call this after [`parse_boxed_instrument_from_json`] so
/// malformed instruments are reported before market extraction without
/// deserializing the instrument twice.
///
/// # Arguments
///
/// * `instrument` - Validated instrument to dispatch through the selected
///   pricer registry.
/// * `market` - Market context supplying curves, quotes, fixings, and FX data.
/// * `as_of` - ISO-8601 valuation date.
/// * `model` - Concrete model key or `"default"` to use the instrument's
///   native default model.
/// * `metrics` - Strict metric identifiers requested with the valuation.
/// * `market_history` - Optional historical market scenarios required by the
///   historical risk metrics (`hvar`, `expected_shortfall`); cloned into the
///   pricing options.
/// * `pricing_options` - Pricing services and configuration supplied by the
///   caller. When it contains a pricer registry, that registry drives both the
///   base value and every nested metric reprice; otherwise the shared standard
///   registry is used.
///
/// # Errors
///
/// Returns an error for an invalid date, model or metric identifier; missing
/// required market data; or a failure in the selected pricer
/// or metric calculation.
#[tracing::instrument(
    name = "valuations.json.price_instrument",
    level = "debug",
    skip(instrument, market, metrics, market_history, pricing_options),
    fields(
        inst_id = %instrument.as_instrument().id(),
        model = %model,
        as_of = %as_of,
        metric_count = metrics.len(),
        has_market_history = market_history.is_some(),
    ),
    err,
)]
pub fn price_instrument(
    instrument: &ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: &[String],
    market_history: Option<&MarketHistory>,
    pricing_options: crate::instruments::PricingOptions,
) -> finstack_quant_core::Result<ValuationResult> {
    let instrument = instrument.as_instrument();
    let as_of = finstack_quant_core::dates::parse_iso_date(as_of)?;
    let model = resolve_model_key(instrument, model)?;
    let registry = pricing_options
        .registry
        .as_ref()
        .map_or_else(|| STANDARD.shared_pricers(), Arc::clone);
    let shared_metric_registry = pricing_options
        .metric_registry
        .clone()
        .unwrap_or_else(|| STANDARD.shared_metrics());
    let metric_ids = shared_metric_registry.resolve_metric_ids(metrics)?;
    let pricing_options = pricing_options.with_metric_registry(shared_metric_registry);
    let pricing_options = match market_history {
        Some(history) => pricing_options.with_market_history(Arc::new(history.clone())),
        None => pricing_options,
    };
    registry
        .price_with_metrics(
            instrument,
            model,
            market,
            as_of,
            &metric_ids,
            pricing_options,
        )
        .map_err(Into::into)
}

/// Price a parsed instrument and return one requested scalar metric.
///
/// Fails when the selected model does not produce `metric`.
///
/// # Arguments
///
/// * `instrument` - Validated instrument already accepted by
///   [`price_instrument`].
/// * `market` - Market context supplying model-required curves, quotes, and FX
///   data.
/// * `as_of` - ISO-8601 valuation date passed to the pricing lifecycle.
/// * `model` - Canonical model key, or `"default"` to use the instrument's
///   registered default model.
/// * `metric` - Scalar metric name that must be produced by the selected model.
/// * `pricing_options` - Pricing services and configuration supplied by the
///   caller, including any host-attached recalibration provider.
///
/// # Errors
///
/// Propagates pricing and input-validation failures from [`price_instrument`],
/// and returns `Error::Validation` when the selected model does not produce
/// `metric`.
pub fn metric_value(
    instrument: &ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metric: &str,
    pricing_options: crate::instruments::PricingOptions,
) -> finstack_quant_core::Result<f64> {
    let metrics = [metric.to_string()];
    let result = price_instrument(
        instrument,
        market,
        as_of,
        model,
        &metrics,
        None,
        pricing_options,
    )?;
    result
        .metric_str(metric)
        .ok_or_else(|| Error::Validation(format!("metric `{metric}` was not returned")))
}

/// Price a parsed option instrument and return the standard sparse option
/// Greek set produced by the selected model.
///
/// The result is an ordered subset of [`STANDARD_OPTION_GREEKS`]. Models that
/// cannot produce a requested Greek omit it rather than fabricating a zero.
///
/// # Arguments
///
/// * `instrument` - Validated option instrument already accepted by
///   [`price_instrument`].
/// * `market` - Market context supplying model-required curves, volatilities,
///   quotes, and FX data.
/// * `as_of` - ISO-8601 valuation date passed to the pricing lifecycle.
/// * `model` - Canonical option model key, or `"default"` for the
///   instrument's registered default model.
/// * `pricing_options` - Pricing services and configuration supplied by the
///   caller, including any host-attached recalibration provider.
///
/// # Errors
///
/// Returns an error for the same input, market-data, or pricing failures as
/// [`price_instrument`], and a validation error if any computed greek is
/// non-finite (hosts would otherwise emit `null`).
pub fn present_standard_option_greeks(
    instrument: &ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    pricing_options: crate::instruments::PricingOptions,
) -> finstack_quant_core::Result<Vec<(&'static str, f64)>> {
    // This entry point asks for a fixed superset, not a caller-chosen list, and
    // its contract is "an ordered subset". Pricing rejects any requested metric
    // the instrument type has no calculator for, so narrow the superset here
    // instead of letting the rejection escape as a spurious error.
    let metric_registry = match pricing_options.metric_registry.as_deref() {
        Some(registry) => registry,
        None => crate::pricer::STANDARD.metrics(),
    };
    let candidates: Vec<MetricId> = STANDARD_OPTION_GREEKS
        .iter()
        .filter_map(|m| m.parse::<MetricId>().ok())
        .collect();
    let metric_ids: Vec<String> = metric_registry
        .applicable_subset(&candidates, instrument.as_instrument().key())
        .iter()
        .map(|id| id.as_str().to_string())
        .collect();
    let result = price_instrument(
        instrument,
        market,
        as_of,
        model,
        &metric_ids,
        None,
        pricing_options,
    )?;
    let pairs: Vec<(&'static str, f64)> = STANDARD_OPTION_GREEKS
        .iter()
        .filter_map(|m| result.metric_str(m).map(|v| (*m, v)))
        .collect();
    if let Some((metric, value)) = pairs.iter().find(|(_, v)| !v.is_finite()) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "greek '{metric}' evaluated to a non-finite value ({value})"
        )));
    }
    Ok(pairs)
}
