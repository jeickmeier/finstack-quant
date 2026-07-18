//! Shared JSON pricing helpers for host-language bindings.
//!
//! This module centralizes the tagged-instrument JSON pipeline used by the
//! Python and WASM bindings: parse instrument JSON, optionally merge metric
//! pricing overrides, parse the as-of date and model key, and dispatch through
//! the standard pricer registry.

use super::{shared_standard_registry, ModelKey, PricerRegistry};
use crate::instruments::{Instrument, InstrumentEnvelope, InstrumentJson, MetricPricingOverrides};
use crate::metrics::MetricId;
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Error;
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::BTreeMap;

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

/// Parse a tagged instrument JSON payload into the canonical Rust enum.
///
/// This accepts the `InstrumentJson` form used by direct Rust callers. For an
/// envelope that can also carry schema metadata, use
/// [`parse_boxed_instrument_json`].
///
/// # Arguments
///
/// * `json` - UTF-8 JSON containing a recognized instrument `type` tag and
///   the matching instrument specification.
///
/// # Errors
///
/// Returns `Error::Validation` when `json` is malformed or does not match a
/// supported tagged instrument shape.
pub fn parse_instrument_json(json: &str) -> finstack_quant_core::Result<InstrumentJson> {
    serde_json::from_str(json)
        .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))
}

/// Build and validate canonical tagged instrument JSON from either a bare spec
/// object or an already-tagged instrument object.
///
/// A bare object is wrapped as `{ "type": type_tag, "spec": value }`. A
/// tagged object must carry the same `type_tag`; this prevents an API caller
/// from accidentally pricing one instrument under another's route.
///
/// # Arguments
///
/// * `type_tag` - Canonical instrument discriminator expected by the caller's
///   API route.
/// * `value` - A bare instrument spec or an already-tagged instrument object.
///
/// # Returns
///
/// Canonical serialized JSON after the type-specific deserialization and
/// validation path has succeeded.
///
/// # Errors
///
/// Returns `Error::Validation` when the tag is missing or non-string, differs
/// from `type_tag`, the payload is not a supported instrument, or canonical
/// serialization fails.
pub fn canonical_instrument_json(
    type_tag: &str,
    value: Value,
) -> finstack_quant_core::Result<String> {
    let payload = if value.get("type").is_some() {
        let actual = value.get("type").and_then(Value::as_str).ok_or_else(|| {
            Error::Validation("instrument JSON field `type` must be a string".to_string())
        })?;
        if actual != type_tag {
            return Err(Error::Validation(format!(
                "expected instrument type `{type_tag}`, got `{actual}`"
            )));
        }
        value
    } else {
        let mut payload = Map::new();
        payload.insert("type".to_string(), Value::String(type_tag.to_string()));
        payload.insert("spec".to_string(), value);
        Value::Object(payload)
    };

    let json = serde_json::to_string(&payload)
        .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))?;
    validate_instrument_json(&json)
}

/// Build and validate canonical tagged instrument JSON from a JSON string.
///
/// This is the string-input counterpart to [`canonical_instrument_json`].
///
/// # Arguments
///
/// * `type_tag` - Canonical instrument discriminator expected by the caller.
/// * `json` - A JSON object containing either a bare spec or tagged payload.
///
/// # Errors
///
/// Returns `Error::Validation` when `json` is malformed or when canonicalizing
/// the resulting value fails the rules documented by
/// [`canonical_instrument_json`].
pub fn canonical_instrument_json_from_str(
    type_tag: &str,
    json: &str,
) -> finstack_quant_core::Result<String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))?;
    canonical_instrument_json(type_tag, value)
}

/// Validate tagged instrument JSON against the pricing contract and return its
/// canonical JSON representation.
///
/// A payload with a `schema` member is parsed as an [`InstrumentEnvelope`]; all
/// other payloads are parsed as [`InstrumentJson`]. This function is useful for
/// accepting external configuration before any market data or pricing model is
/// involved.
///
/// # Arguments
///
/// * `json` - Tagged instrument JSON in direct or envelope form.
///
/// # Errors
///
/// Returns `Error::Validation` when parsing, instrument validation, or
/// canonical serialization fails.
pub fn validate_instrument_json(json: &str) -> finstack_quant_core::Result<String> {
    parse_boxed_instrument_json(json, None)?;
    let value: Value = serde_json::from_str(json)
        .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))?;
    if value.get("schema").is_some() {
        let parsed: InstrumentEnvelope = serde_json::from_value(value)
            .map_err(|e| Error::Validation(format!("invalid instrument envelope JSON: {e}")))?;
        serde_json::to_string(&parsed)
            .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))
    } else {
        let parsed: InstrumentJson = serde_json::from_value(value)
            .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))?;
        serde_json::to_string(&parsed)
            .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))
    }
}

/// List all metric IDs in the standard metric registry.
pub fn list_standard_metrics() -> Vec<String> {
    crate::metrics::standard_registry()
        .available_metrics()
        .into_iter()
        .map(|id| id.to_string())
        .collect()
}

/// List all standard metrics grouped by display category.
pub fn list_standard_metrics_grouped() -> BTreeMap<String, Vec<String>> {
    crate::metrics::standard_registry()
        .available_metrics_grouped()
        .into_iter()
        .map(|(group, metrics)| {
            (
                group.display_name().to_string(),
                metrics
                    .into_iter()
                    .map(|metric| metric.to_string())
                    .collect(),
            )
        })
        .collect()
}

/// List every pricing model key that has a pricer in the standard registry.
///
/// The list is **registry-derived**, not enum-derived: it reports real dispatch
/// coverage. A [`ModelKey`] variant that exists in the enum but has no
/// registered pricer is omitted, whereas iterating `ModelKey` itself would
/// advertise models that cannot price any instrument. Names are the canonical
/// `ModelKey` display strings (`"discounting"`, `"black76"`, …) accepted by
/// [`parse_model_key`] and by the `model` argument of the JSON pricing entry
/// points.
///
/// # Returns
///
/// Deduplicated canonical model keys in ascending [`ModelKey`] order.
pub fn list_models() -> Vec<String> {
    crate::pricer::standard_registry()
        .all_models()
        .into_iter()
        .map(|model| model.to_string())
        .collect()
}

/// List the standard registry's pricing models grouped by instrument type.
///
/// This is the grouped counterpart to [`list_models`] and shares its
/// registry-derived semantics: only instrument types with at least one
/// registered pricer appear, and each entry lists only the models that can
/// actually price that instrument. Keys are canonical [`crate::pricer::InstrumentType`]
/// display strings; values are canonical [`ModelKey`] display strings.
///
/// # Returns
///
/// A map from instrument type to its ascending, deduplicated model keys, in
/// ascending instrument-type order.
pub fn list_models_grouped() -> BTreeMap<String, Vec<String>> {
    crate::pricer::standard_registry()
        .all_models_grouped()
        .into_iter()
        .map(|(instrument, models)| {
            (
                instrument.to_string(),
                models.into_iter().map(|model| model.to_string()).collect(),
            )
        })
        .collect()
}

/// Parse tagged instrument JSON, optionally merge metric pricing overrides, and
/// box the concrete instrument for pricing dispatch.
///
/// # Arguments
///
/// * `instrument_json` - Tagged direct or envelope instrument JSON.
/// * `pricing_options` - Optional JSON overrides merged into the instrument's
///   metric-pricing configuration before validation.
///
/// # Errors
///
/// Returns `Error::Validation` when either JSON value is malformed, the
/// override cannot be merged, or the resulting instrument is invalid.
pub fn parse_boxed_instrument_json(
    instrument_json: &str,
    pricing_options: Option<&str>,
) -> finstack_quant_core::Result<Box<dyn Instrument>> {
    let effective_json = instrument_json_for_pricing(instrument_json, pricing_options)?;
    InstrumentEnvelope::from_str(effective_json.as_ref())
}

/// Parse a concrete model key used by the JSON pricing helpers.
///
/// This function only accepts named [`ModelKey`] values. The special
/// case-insensitive `"default"` selector is handled by the pricing entry
/// points, where it resolves to the instrument's default model.
///
/// # Arguments
///
/// * `model` - Canonical textual model key, such as `"discounting"` or
///   `"black76"`.
///
/// # Errors
///
/// Returns `Error::Validation` when `model` is not a supported concrete model
/// key.
pub fn parse_model_key(model: &str) -> finstack_quant_core::Result<ModelKey> {
    model
        .parse::<ModelKey>()
        .map_err(|e| Error::Validation(format!("Unknown model key: '{model}'. {e}")))
}

/// Pretty-print tagged instrument JSON for inspection-oriented binding APIs.
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

fn resolve_model_key(
    instrument: &dyn Instrument,
    model: &str,
) -> finstack_quant_core::Result<ModelKey> {
    if model.trim().eq_ignore_ascii_case("default") {
        Ok(instrument.default_model())
    } else {
        parse_model_key(model)
    }
}

/// Price a tagged instrument JSON payload using the shared standard registry.
///
/// This is the host-language-friendly path for a single valuation with no
/// explicit metric requests. Pass `"default"` for `model` to use the
/// instrument's native pricing model.
///
/// # Arguments
///
/// * `instrument_json` - Tagged direct or envelope instrument JSON.
/// * `market` - Market context supplying all required curves, surfaces,
///   fixings, and FX data.
/// * `as_of` - ISO-8601 valuation date.
/// * `model` - A concrete model key or the case-insensitive `"default"`.
///
/// # Errors
///
/// Returns an error when the instrument JSON, valuation date, or model key is
/// invalid, required market data is missing, or the selected pricer fails.
pub fn price_instrument_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> finstack_quant_core::Result<ValuationResult> {
    price_instrument_json_request(instrument_json, market, as_of, model, &[], None, None)
}

/// Price a tagged instrument JSON payload with explicit metric requests and
/// optional historical scenarios for VaR-style metrics.
///
/// # Arguments
///
/// * `instrument_json` - Tagged direct or envelope instrument JSON.
/// * `market` - Market context used for pricing and risk calculations.
/// * `as_of` - ISO-8601 valuation date.
/// * `model` - A concrete model key or the case-insensitive `"default"`.
/// * `metrics` - Strict metric identifiers requested in addition to the base
///   valuation.
/// * `pricing_options` - Optional JSON metric-pricing overrides applied to the
///   instrument before validation.
/// * `market_history_json` - Optional serialized market history for metrics
///   that require historical scenarios, such as historical VaR.
///
/// # Errors
///
/// Returns an error for invalid JSON, date, model, metric identifier, or market
/// history; missing required market data; or a failure in the selected pricer or
/// metric calculation.
pub fn price_instrument_json_with_metrics_and_history(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: &[String],
    pricing_options: Option<&str>,
    market_history_json: Option<&str>,
) -> finstack_quant_core::Result<ValuationResult> {
    price_instrument_json_request(
        instrument_json,
        market,
        as_of,
        model,
        metrics,
        pricing_options,
        market_history_json,
    )
}

fn price_instrument_json_request(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: &[String],
    pricing_options: Option<&str>,
    market_history_json: Option<&str>,
) -> finstack_quant_core::Result<ValuationResult> {
    let instrument = parse_boxed_instrument_json(instrument_json, pricing_options)?;
    let as_of = finstack_quant_core::dates::parse_iso_date(as_of)?;
    let model = resolve_model_key(instrument.as_ref(), model)?;
    let metric_ids: Vec<MetricId> = metrics
        .iter()
        .map(|metric| MetricId::parse_strict(metric))
        .collect::<finstack_quant_core::Result<_>>()?;
    let pricing_options = if let Some(json) = market_history_json {
        let history: crate::metrics::risk::MarketHistory = serde_json::from_str(json)
            .map_err(|e| Error::Validation(format!("invalid market history JSON: {e}")))?;
        crate::instruments::PricingOptions::default()
            .with_market_history(std::sync::Arc::new(history))
    } else {
        crate::instruments::PricingOptions::default()
    };
    let registry = shared_standard_registry();
    PricerRegistry::price_with_metrics_shared(
        &registry,
        instrument.as_ref(),
        model,
        market,
        as_of,
        &metric_ids,
        pricing_options,
    )
    .map_err(Into::into)
}

/// Price a tagged instrument JSON payload and return one requested scalar
/// metric, failing when the metric is not produced by the selected model.
///
/// # Errors
///
/// Propagates pricing and input-validation failures from
/// [`price_instrument_json`], and returns `Error::Validation` when the selected
/// model does not produce `metric`.
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 canonical tagged instrument JSON to construct
///   and price.
/// * `market` - Market context supplying model-required curves, quotes, and FX
///   data.
/// * `as_of` - ISO-8601 valuation date passed to the pricing lifecycle.
/// * `model` - Canonical model key, or `"default"` to use the instrument's
///   registered default model.
/// * `metric` - Scalar metric name that must be produced by the selected model.
pub fn metric_value_from_instrument_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metric: &str,
) -> finstack_quant_core::Result<f64> {
    let metric_ids = [metric.to_string()];
    let result = price_instrument_json_request(
        instrument_json,
        market,
        as_of,
        model,
        &metric_ids,
        None,
        None,
    )?;
    result
        .metric_str(metric)
        .ok_or_else(|| Error::Validation(format!("metric `{metric}` was not returned")))
}

/// Price a tagged instrument JSON payload and return the requested scalar
/// metrics that were produced by the selected model.
///
/// The returned pairs preserve the requested order but omit unavailable
/// metrics. Use [`metric_value_from_instrument_json`] when an unavailable
/// metric must be treated as an error.
///
/// # Errors
///
/// Returns an error for the same input, market-data, or pricing failures as
/// [`price_instrument_json`]. Missing individual metrics are omitted rather
/// than causing an error.
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 canonical tagged instrument JSON to construct
///   and price.
/// * `market` - Market context supplying model-required curves, quotes, and FX
///   data.
/// * `as_of` - ISO-8601 valuation date passed to the pricing lifecycle.
/// * `model` - Canonical model key, or `"default"` to use the instrument's
///   registered default model.
/// * `metrics` - Requested scalar metric names in desired output order;
///   unavailable entries are omitted from the returned pairs.
pub fn present_metric_values_from_instrument_json<'a>(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: &'a [&'a str],
) -> finstack_quant_core::Result<Vec<(&'a str, f64)>> {
    let metric_ids: Vec<String> = metrics.iter().map(|m| (*m).to_string()).collect();
    let result = price_instrument_json_request(
        instrument_json,
        market,
        as_of,
        model,
        &metric_ids,
        None,
        None,
    )?;
    Ok(metrics
        .iter()
        .filter_map(|m| result.metric_str(m).map(|v| (*m, v)))
        .collect())
}

/// Price a tagged option instrument JSON payload and return the standard sparse
/// option Greek set produced by the selected model.
///
/// The result is an ordered subset of [`STANDARD_OPTION_GREEKS`]. Models that
/// cannot produce a requested Greek omit it rather than fabricating a zero.
///
/// # Errors
///
/// Returns an error for the same input, market-data, or pricing failures as
/// [`price_instrument_json`].
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 canonical tagged option-instrument JSON to
///   construct and price.
/// * `market` - Market context supplying model-required curves, volatilities,
///   quotes, and FX data.
/// * `as_of` - ISO-8601 valuation date passed to the pricing lifecycle.
/// * `model` - Canonical option model key, or `"default"` for the
///   instrument's registered default model.
pub fn present_standard_option_greeks_from_instrument_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> finstack_quant_core::Result<Vec<(&'static str, f64)>> {
    present_metric_values_from_instrument_json(
        instrument_json,
        market,
        as_of,
        model,
        STANDARD_OPTION_GREEKS,
    )
}

/// Best-effort extraction of `spec.id` from a tagged instrument JSON payload.
///
/// Used purely to enrich error messages so an analyst running a batch can
/// identify the offending row. Returns `None` when the JSON is malformed or
/// the `id` field is absent — callers must not depend on the id being present.
fn extract_spec_id_lossy(instrument_json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(instrument_json).ok()?;
    value
        .get("spec")?
        .get("id")?
        .as_str()
        .map(ToString::to_string)
}

/// Suffix `[instrument=<id>]` to an error message when an id is known.
fn with_id_suffix(message: String, id: Option<&str>) -> String {
    match id {
        Some(id) => format!("{message} [instrument={id}]"),
        None => message,
    }
}

fn instrument_json_for_pricing<'a>(
    instrument_json: &'a str,
    pricing_options: Option<&str>,
) -> finstack_quant_core::Result<Cow<'a, str>> {
    let Some(pricing_options_json) = pricing_options else {
        return Ok(Cow::Borrowed(instrument_json));
    };

    let instrument_id = extract_spec_id_lossy(instrument_json);
    let id = instrument_id.as_deref();

    let pricing_options: MetricPricingOverrides = serde_json::from_str(pricing_options_json)
        .map_err(|e| {
            Error::Validation(with_id_suffix(
                format!("invalid pricing options JSON: {e}"),
                id,
            ))
        })?;
    let mut document: Value = serde_json::from_str(instrument_json).map_err(|e| {
        Error::Validation(with_id_suffix(format!("invalid instrument JSON: {e}"), id))
    })?;
    let pricing_patch = serde_json::to_value(&pricing_options).map_err(|e| {
        Error::Validation(with_id_suffix(
            format!("invalid pricing options JSON: {e}"),
            id,
        ))
    })?;

    let patch = pricing_patch.as_object().cloned().ok_or_else(|| {
        Error::Validation(with_id_suffix(
            "metric pricing overrides must serialize to an object".to_string(),
            id,
        ))
    })?;
    let spec = document
        .get_mut("spec")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            Error::Validation(with_id_suffix(
                "instrument JSON must contain an object spec".into(),
                id,
            ))
        })?;
    let pricing_overrides = spec
        .entry("pricing_overrides".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let pricing_overrides = pricing_overrides.as_object_mut().ok_or_else(|| {
        Error::Validation(with_id_suffix(
            "instrument spec.pricing_overrides must be an object".to_string(),
            id,
        ))
    })?;
    pricing_overrides.extend(patch);

    serde_json::to_string(&document)
        .map(Cow::Owned)
        .map_err(|e| Error::Validation(with_id_suffix(format!("invalid instrument JSON: {e}"), id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::equity::equity_option::EquityOption;
    use crate::instruments::equity::pe_fund::PrivateMarketsFund;
    use crate::instruments::fixed_income::bond::Bond;
    use crate::instruments::fixed_income::structured_credit::StructuredCredit;
    use crate::instruments::fx::FxOption;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;

    fn bond_instrument_json() -> String {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            "USD-OIS",
        )
        .expect("bond");
        serde_json::to_string(&InstrumentJson::Bond(bond)).expect("serialize")
    }

    fn market_context() -> MarketContext {
        let base = time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.5, 0.99), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
            .build()
            .expect("curve");
        MarketContext::new().insert(disc)
    }

    #[test]
    fn default_model_resolves_to_instrument_native_model() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            "USD-OIS",
        )
        .expect("bond");
        assert_eq!(
            resolve_model_key(&bond, "default").expect("model"),
            ModelKey::Discounting
        );

        let fx_option = FxOption::example().expect("fx option");
        assert_eq!(
            resolve_model_key(&fx_option, "default").expect("model"),
            ModelKey::Black76
        );
    }

    fn equity_option_json_with_negative_vol_override() -> String {
        let option = EquityOption::example().expect("option");
        let mut json = serde_json::to_value(InstrumentJson::EquityOption(option)).expect("json");
        json["spec"]["pricing_overrides"]["implied_volatility"] = Value::from(-0.20);
        serde_json::to_string(&json).expect("serialize")
    }

    fn equity_option_json_with_invalid_strike() -> String {
        let mut option = EquityOption::example().expect("option");
        option.strike = -100.0;
        serde_json::to_string(&InstrumentJson::EquityOption(option)).expect("serialize")
    }

    fn fx_spot_spec_value() -> Value {
        serde_json::json!({
            "id": "EURUSD-SPOT",
            "base_currency": "EUR",
            "quote_currency": "USD",
            "settlement": "2025-01-17",
            "spot_rate": 1.20,
            "notional": {"amount": 1_000_000.0, "currency": "EUR"},
            "attributes": {},
        })
    }

    #[test]
    fn canonical_instrument_json_wraps_bare_fx_spec() {
        let canonical =
            canonical_instrument_json("fx_spot", fx_spot_spec_value()).expect("canonical fx spot");
        let parsed: Value = serde_json::from_str(&canonical).expect("json");
        assert_eq!(parsed["type"], "fx_spot");
        assert_eq!(parsed["spec"]["id"], "EURUSD-SPOT");
    }

    #[test]
    fn canonical_instrument_json_rejects_wrong_existing_type() {
        let err = canonical_instrument_json(
            "fx_forward",
            serde_json::json!({"type": "fx_spot", "spec": fx_spot_spec_value()}),
        )
        .expect_err("wrong tag should be rejected");
        assert!(err
            .to_string()
            .contains("expected instrument type `fx_forward`, got `fx_spot`"));
    }

    #[test]
    fn instrument_json_for_pricing_error_includes_instrument_id() {
        // Malformed pricing options on a well-formed instrument JSON.
        let json = bond_instrument_json();
        let err = instrument_json_for_pricing(&json, Some("not-valid-json"))
            .expect_err("malformed pricing options must error");
        let msg = err.to_string();
        assert!(
            msg.contains("invalid pricing options JSON"),
            "expected pricing options error, got: {msg}"
        );
        assert!(
            msg.contains("[instrument=TEST-BOND]"),
            "expected instrument id suffix, got: {msg}"
        );
    }

    #[test]
    fn instrument_json_for_pricing_error_without_id_when_json_unparseable() {
        // Instrument JSON itself is malformed, so id cannot be extracted; the
        // error message should still be useful but without an [instrument=...]
        // suffix.
        let err = instrument_json_for_pricing("{not-json", Some("{}"))
            .expect_err("malformed instrument JSON must error");
        let msg = err.to_string();
        assert!(
            !msg.contains("[instrument="),
            "no id should be attached when JSON is unparseable, got: {msg}"
        );
    }

    #[test]
    fn instrument_json_for_pricing_merges_metric_overrides() {
        let json = bond_instrument_json();
        let merged = instrument_json_for_pricing(
            &json,
            Some(
                r#"{"theta_period":"1D","breakeven_config":{"target":"z_spread","mode":"linear"}}"#,
            ),
        )
        .expect("merge");
        let parsed: Value = serde_json::from_str(merged.as_ref()).expect("json");
        assert_eq!(parsed["spec"]["pricing_overrides"]["theta_period"], "1D");
        assert_eq!(
            parsed["spec"]["pricing_overrides"]["breakeven_config"]["target"],
            "z_spread"
        );
    }

    #[test]
    fn validate_instrument_json_rejects_invalid_pricing_overrides() {
        let err = validate_instrument_json(&equity_option_json_with_negative_vol_override())
            .expect_err("negative implied volatility override must be rejected");
        assert!(
            err.to_string().contains("NegativeValue") || err.to_string().contains("negative"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_boxed_instrument_json_rejects_invalid_pricing_overrides() {
        let Err(err) =
            parse_boxed_instrument_json(&equity_option_json_with_negative_vol_override(), None)
        else {
            panic!("negative implied volatility override must be rejected")
        };
        assert!(
            err.to_string().contains("NegativeValue") || err.to_string().contains("negative"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_instrument_json_rejects_domain_invariants() {
        let err = validate_instrument_json(&equity_option_json_with_invalid_strike())
            .expect_err("negative equity-option strike must be rejected");
        assert!(
            err.to_string().contains("strike") && err.to_string().contains("positive"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_instrument_json_rejects_non_conserving_waterfall() {
        let fund = PrivateMarketsFund::example().expect("fund");
        let mut json =
            serde_json::to_value(InstrumentJson::PrivateMarketsFund(fund)).expect("serialize");
        json["spec"]["waterfall_spec"]["tranches"][3]["promote_tier"]["gp_share"] =
            Value::from(0.6);
        let json = serde_json::to_string(&json).expect("json");

        validate_instrument_json(&json).expect_err("LP and GP shares above 100% must be rejected");
    }

    #[test]
    fn validate_instrument_json_rejects_invalid_cleanup_call_threshold() {
        let mut deal = StructuredCredit::example();
        deal.cleanup_call_pct = Some(-0.5);
        let json = serde_json::to_string(&InstrumentJson::StructuredCredit(Box::new(deal)))
            .expect("serialize");

        let err = validate_instrument_json(&json)
            .expect_err("cleanup-call threshold outside (0, 1) must be rejected");
        assert!(err.to_string().contains("cleanup_call_pct"));
    }

    #[test]
    fn validate_instrument_json_accepts_versioned_envelope() {
        let bond = Bond::fixed(
            "ENVELOPE-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            "USD-OIS",
        )
        .expect("bond");
        let envelope = InstrumentEnvelope::new(InstrumentJson::Bond(bond));
        let json = serde_json::to_string(&envelope).expect("envelope json");

        let canonical = validate_instrument_json(&json).expect("valid envelope");
        let value: Value = serde_json::from_str(&canonical).expect("canonical json");
        assert_eq!(value["schema"], InstrumentEnvelope::CURRENT_SCHEMA);
        assert_eq!(value["instrument"]["type"], "bond");
    }

    #[test]
    fn parse_boxed_instrument_json_rejects_domain_invariants() {
        let Err(err) = parse_boxed_instrument_json(&equity_option_json_with_invalid_strike(), None)
        else {
            panic!("negative equity-option strike must be rejected")
        };
        assert!(
            err.to_string().contains("strike") && err.to_string().contains("positive"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn json_pricing_routes_validate_instrument_before_model_resolution() {
        let invalid = equity_option_json_with_invalid_strike();
        let market = MarketContext::new();

        let plain = price_instrument_json(&invalid, &market, "not-a-date", "not-a-model")
            .expect_err("instrument validation must win");
        let with_metrics = price_instrument_json_with_metrics_and_history(
            &invalid,
            &market,
            "not-a-date",
            "not-a-model",
            &["not-a-metric".to_string()],
            None,
            None,
        )
        .expect_err("instrument validation must win");
        let scalar = metric_value_from_instrument_json(
            &invalid,
            &market,
            "not-a-date",
            "not-a-model",
            "not-a-metric",
        )
        .expect_err("instrument validation must win");

        for error in [plain, with_metrics, scalar] {
            let message = error.to_string();
            assert!(
                message.contains("strike") && message.contains("positive"),
                "unexpected error ordering: {message}"
            );
        }
    }

    #[test]
    fn parse_model_key_recognizes_standard_keys() {
        assert_eq!(
            parse_model_key("discounting").expect("ok"),
            ModelKey::Discounting
        );
        assert_eq!(parse_model_key("tree").expect("ok"), ModelKey::Tree);
        assert_eq!(parse_model_key("black76").expect("ok"), ModelKey::Black76);
        assert_eq!(
            parse_model_key("hull_white_1f").expect("ok"),
            ModelKey::HullWhite1F
        );
        assert_eq!(
            parse_model_key("hazard_rate").expect("ok"),
            ModelKey::HazardRate
        );
        assert_eq!(parse_model_key("normal").expect("ok"), ModelKey::Normal);
        assert_eq!(
            parse_model_key("monte_carlo_gbm").expect("ok"),
            ModelKey::MonteCarloGBM
        );
        assert_eq!(
            parse_model_key("bond_future_clean_price_proxy").expect("ok"),
            ModelKey::BondFutureCleanPriceProxy
        );
    }

    #[test]
    fn price_instrument_json_prices_bond() {
        let result = price_instrument_json(
            &bond_instrument_json(),
            &market_context(),
            "2024-01-01",
            "discounting",
        )
        .expect("price");
        assert_eq!(result.instrument_id, "TEST-BOND");
    }

    #[test]
    fn price_instrument_json_with_metrics_and_history_accepts_pricing_options() {
        let result = price_instrument_json_with_metrics_and_history(
            &bond_instrument_json(),
            &market_context(),
            "2024-01-01",
            "discounting",
            &["dirty_price".to_string()],
            Some(r#"{"theta_period":"1D"}"#),
            None,
        )
        .expect("price");
        assert_eq!(result.instrument_id, "TEST-BOND");
    }

    #[test]
    fn price_instrument_json_with_metrics_and_history_rejects_unknown_metric_names() {
        let err = price_instrument_json_with_metrics_and_history(
            &bond_instrument_json(),
            &market_context(),
            "2024-01-01",
            "discounting",
            &["dvO1".to_string()],
            None,
            None,
        )
        .expect_err("JSON pricing boundary should parse requested metrics strictly");

        assert!(
            err.to_string().contains("dvO1") || err.to_string().contains("dvo1"),
            "unknown metric error should include the requested metric, got: {err}"
        );
    }

    #[test]
    fn price_instrument_json_with_metrics_and_history_accepts_market_history_for_hvar() {
        let history = crate::metrics::risk::MarketHistory::new(
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            2,
            vec![
                crate::metrics::risk::MarketScenario::new(
                    time::Date::from_calendar_date(2023, time::Month::December, 29).expect("date"),
                    vec![crate::metrics::risk::RiskFactorShift {
                        factor: crate::metrics::risk::RiskFactorType::DiscountRate {
                            curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
                            tenor_years: 5.0,
                        },
                        shift: 0.0005,
                    }],
                ),
                crate::metrics::risk::MarketScenario::new(
                    time::Date::from_calendar_date(2023, time::Month::December, 28).expect("date"),
                    vec![crate::metrics::risk::RiskFactorShift {
                        factor: crate::metrics::risk::RiskFactorType::DiscountRate {
                            curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
                            tenor_years: 10.0,
                        },
                        shift: -0.0003,
                    }],
                ),
            ],
        );
        let history_json = serde_json::to_string(&history).expect("history JSON");

        let result = price_instrument_json_with_metrics_and_history(
            &bond_instrument_json(),
            &market_context(),
            "2024-01-01",
            "discounting",
            &["hvar".to_string(), "expected_shortfall".to_string()],
            None,
            Some(&history_json),
        )
        .expect("HVar should price when market history is supplied");

        assert!(result.measures.contains_key(MetricId::HVar.as_str()));
        assert!(result
            .measures
            .contains_key(MetricId::ExpectedShortfall.as_str()));
    }

    #[test]
    fn metric_helpers_return_requested_present_metrics() {
        let json = bond_instrument_json();
        let dirty_price = metric_value_from_instrument_json(
            &json,
            &market_context(),
            "2024-01-01",
            "discounting",
            "dirty_price",
        )
        .expect("metric");
        assert!(dirty_price.is_finite());

        let metrics = present_metric_values_from_instrument_json(
            &json,
            &market_context(),
            "2024-01-01",
            "discounting",
            &["dirty_price", "vega"],
        )
        .expect("metrics");
        assert_eq!(metrics, vec![("dirty_price", dirty_price), ("vega", 0.0)]);
    }
}
