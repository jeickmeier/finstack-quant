//! Shared JSON pricing helpers for host-language bindings.
//!
//! This module centralizes the tagged-instrument JSON pipeline used by the
//! Python and WASM bindings: parse instrument JSON, optionally merge metric
//! pricing overrides, parse the as-of date and model key, and dispatch through
//! the caller-selected registry or the shared standard registry.

use super::{shared_standard_registry, ModelKey};
use crate::instruments::json_loader::MAX_JSON_BYTES;
use crate::instruments::{Instrument, InstrumentEnvelope, InstrumentJson, MetricPricingOverrides};
use crate::metrics::MetricId;
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Error;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeMap;
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
    fn new(instrument: Box<dyn Instrument>) -> Self {
        Self { instrument }
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
    if json.len() > MAX_JSON_BYTES {
        return Err(Error::Validation(format!(
            "Instrument JSON input exceeds the {} MiB size limit",
            MAX_JSON_BYTES / (1024 * 1024)
        )));
    }
    let envelope: InstrumentEnvelope = serde_json::from_str(json)
        .map_err(|error| Error::Validation(format!("invalid instrument envelope JSON: {error}")))?;
    let instrument = envelope.instrument;
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
/// Canonical serialized v1 envelope after type-specific deserialization and
/// validation has succeeded.
///
/// # Errors
///
/// Returns `Error::Validation` when `spec` is not a bare object, the payload is
/// not a supported instrument, or canonical serialization fails.
pub fn instrument_envelope_from_spec(
    type_tag: &str,
    spec: Value,
) -> finstack_quant_core::Result<String> {
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
    serde_json::to_string(&InstrumentEnvelope::new(instrument))
        .map_err(|error| Error::Validation(format!("invalid instrument JSON: {error}")))
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
        return Err(Error::Validation(format!(
            "expected instrument type `{type_tag}`, got `{actual}`"
        )));
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
///
/// # Errors
///
/// Returns `Error::Validation` when parsing, instrument validation, or
/// canonical serialization fails.
pub fn validate_instrument_json(json: &str) -> finstack_quant_core::Result<String> {
    let instrument = parse_instrument_from_json(json)?;
    serde_json::to_string(&InstrumentEnvelope::new(instrument))
        .map_err(|e| Error::Validation(format!("invalid instrument JSON: {e}")))
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
    crate::pricer::standard_pricer_registry()
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
    crate::pricer::standard_pricer_registry()
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

/// Parse a canonical instrument envelope, optionally merge metric pricing
/// overrides, and return a validated handle for pricing dispatch.
///
/// # Arguments
///
/// * `instrument_json` - Required canonical v1 instrument envelope.
/// * `pricing_options` - Optional JSON overrides merged into the instrument's
///   metric-pricing configuration before validation.
///
/// # Errors
///
/// Returns `Error::Validation` when either JSON value is malformed, the
/// override cannot be merged, or the resulting instrument is invalid.
pub fn parse_boxed_instrument_from_json(
    instrument_json: &str,
    pricing_options: Option<&str>,
) -> finstack_quant_core::Result<ParsedInstrument> {
    let effective_json = instrument_json_for_pricing(instrument_json, pricing_options)?;
    let instrument = parse_instrument_from_json(effective_json.as_ref())?;
    Ok(ParsedInstrument::new(
        instrument.into_boxed_assuming_validated()?,
    ))
}

/// Parse a concrete model key used by the JSON pricing helpers.
///
/// This function only accepts named [`ModelKey`] values. The special
/// exact `"default"` selector is handled by the pricing entry
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

fn resolve_model_key(
    instrument: &dyn Instrument,
    model: &str,
) -> finstack_quant_core::Result<ModelKey> {
    if model == "default" {
        Ok(instrument.default_model())
    } else {
        parse_model_key(model)
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
/// * `market_history_json` - Optional serialized market history required by
///   historical risk metrics.
/// * `pricing_options` - Pricing services and configuration supplied by the
///   caller. When it contains a pricer registry, that registry drives both the
///   base value and every nested metric reprice; otherwise the shared standard
///   registry is used.
///
/// # Errors
///
/// Returns an error for an invalid date, model, metric identifier, or market
/// history; missing required market data; or a failure in the selected pricer
/// or metric calculation.
pub fn price_instrument(
    instrument: &ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: &[String],
    market_history_json: Option<&str>,
    pricing_options: crate::instruments::PricingOptions,
) -> finstack_quant_core::Result<ValuationResult> {
    let instrument = instrument.as_instrument();
    let as_of = finstack_quant_core::dates::parse_iso_date(as_of)?;
    let model = resolve_model_key(instrument, model)?;
    let registry = pricing_options
        .registry
        .as_ref()
        .map_or_else(shared_standard_registry, Arc::clone);
    let metric_registry = pricing_options.metric_registry.clone();
    let metric_registry = metric_registry
        .as_deref()
        .unwrap_or_else(|| crate::metrics::standard_registry());
    let metric_ids: Vec<MetricId> = metrics
        .iter()
        .map(|metric| {
            MetricId::parse_strict(metric).or_else(|strict_error| {
                let registered = MetricId::custom(metric);
                if metric_registry.has_metric(registered.clone()) {
                    Ok(registered)
                } else {
                    Err(strict_error)
                }
            })
        })
        .collect::<finstack_quant_core::Result<_>>()?;
    let pricing_options = if let Some(json) = market_history_json {
        let history: crate::metrics::risk::MarketHistory = serde_json::from_str(json)
            .map_err(|e| Error::Validation(format!("invalid market history JSON: {e}")))?;
        pricing_options.with_market_history(std::sync::Arc::new(history))
    } else {
        pricing_options
    };
    let pricing_options = pricing_options.mark_instrument_validated();
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
    let metric_ids: Vec<String> = STANDARD_OPTION_GREEKS
        .iter()
        .map(|m| (*m).to_string())
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

/// Best-effort extraction of `instrument.spec.id` from an envelope.
///
/// Used purely to enrich error messages so an analyst running a batch can
/// identify the offending row. Returns `None` when the JSON is malformed or
/// the `id` field is absent — callers must not depend on the id being present.
fn extract_spec_id_lossy(instrument_json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(instrument_json).ok()?;
    value
        .get("instrument")?
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
        .get_mut("instrument")
        .and_then(|instrument| instrument.get_mut("spec"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            Error::Validation(with_id_suffix(
                "instrument envelope must contain an object instrument.spec".into(),
                id,
            ))
        })?;
    let metric_pricing_overrides = spec
        .entry("metric_pricing_overrides".to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let metric_pricing_overrides = metric_pricing_overrides.as_object_mut().ok_or_else(|| {
        Error::Validation(with_id_suffix(
            "instrument.spec.metric_pricing_overrides must be an object".to_string(),
            id,
        ))
    })?;
    metric_pricing_overrides.extend(patch);

    serde_json::to_string(&document)
        .map(Cow::Owned)
        .map_err(|e| Error::Validation(with_id_suffix(format!("invalid instrument JSON: {e}"), id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::commodity::commodity_option::CommodityOption;
    use crate::instruments::credit_derivatives::cds_option::CDSOption;
    use crate::instruments::equity::equity_option::EquityOption;
    use crate::instruments::equity::pe_fund::PrivateMarketsFund;
    use crate::instruments::fixed_income::bond::Bond;
    use crate::instruments::fixed_income::cmo::AgencyCmo;
    use crate::instruments::fixed_income::convertible::ConvertibleBond;
    use crate::instruments::fixed_income::revolving_credit::RevolvingCredit;
    use crate::instruments::fixed_income::structured_credit::StructuredCredit;
    use crate::instruments::fixed_income::term_loan::TermLoan;
    use crate::instruments::fx::FxOption;
    use crate::instruments::rates::ir_future::InterestRateFuture;
    use crate::instruments::rates::swaption::Swaption;
    use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};
    use crate::pricer::{
        expect_inst, InstrumentType, Pricer, PricerKey, PricerRegistry, PricingError,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn price_instrument_from_json(
        instrument_json: &str,
        market: &MarketContext,
        as_of: &str,
        model: &str,
        metrics: &[String],
        instrument_pricing_overrides_json: Option<&str>,
        market_history_json: Option<&str>,
    ) -> finstack_quant_core::Result<ValuationResult> {
        let instrument =
            parse_boxed_instrument_from_json(instrument_json, instrument_pricing_overrides_json)?;
        price_instrument(
            &instrument,
            market,
            as_of,
            model,
            metrics,
            market_history_json,
            crate::instruments::PricingOptions::default(),
        )
    }

    fn metric_value_from_instrument_json(
        instrument_json: &str,
        market: &MarketContext,
        as_of: &str,
        model: &str,
        metric: &str,
    ) -> finstack_quant_core::Result<f64> {
        let instrument = parse_boxed_instrument_from_json(instrument_json, None)?;
        metric_value(
            &instrument,
            market,
            as_of,
            model,
            metric,
            crate::instruments::PricingOptions::default(),
        )
    }

    fn envelope_value(instrument: InstrumentJson) -> Value {
        serde_json::to_value(InstrumentEnvelope::new(instrument)).expect("serialize envelope")
    }

    fn envelope_json(instrument: InstrumentJson) -> String {
        serde_json::to_string(&InstrumentEnvelope::new(instrument)).expect("serialize envelope")
    }

    fn bond_instrument_json() -> String {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        envelope_json(InstrumentJson::Bond(bond))
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

    struct FixedJsonBondPricer {
        calls: Arc<AtomicUsize>,
    }

    impl Pricer for FixedJsonBondPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::Discounting)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            _market: &MarketContext,
            as_of: Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ValuationResult::stamped(
                bond.id(),
                as_of,
                Money::from((321_i64, bond.notional.currency())),
            ))
        }
    }

    struct RepriceJsonMetric;

    impl MetricCalculator for RepriceJsonMetric {
        fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
            context.reprice_raw(context.curves.as_ref(), context.as_of)
        }
    }

    #[test]
    fn default_model_resolves_to_instrument_native_model() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            finstack_quant_core::dates::StubKind::ShortFront,
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

        let equity_option = EquityOption::example().expect("equity option");
        let swaption = Swaption::example();
        let commodity_option = CommodityOption::example();
        for instrument in [
            &equity_option as &dyn Instrument,
            &swaption as &dyn Instrument,
            &commodity_option as &dyn Instrument,
        ] {
            assert_eq!(
                resolve_model_key(instrument, "default").expect("model"),
                ModelKey::Black76
            );
        }

        let mut normal_swaption = Swaption::example();
        normal_swaption.vol_model = crate::instruments::rates::swaption::VolatilityModel::Normal;
        assert_eq!(
            resolve_model_key(&normal_swaption, "default").expect("model"),
            ModelKey::Normal
        );

        let convertible = ConvertibleBond::example().expect("convertible");
        assert_eq!(
            resolve_model_key(&convertible, "default").expect("model"),
            ModelKey::Tree
        );
    }

    fn equity_option_json_with_negative_vol_override() -> String {
        let option = EquityOption::example().expect("option");
        let mut json = envelope_value(InstrumentJson::EquityOption(option));
        json["instrument"]["spec"]["instrument_pricing_overrides"]["market_quotes"]
            ["implied_volatility"] = Value::from(-0.20);
        serde_json::to_string(&json).expect("serialize")
    }

    fn equity_option_json_with_invalid_strike() -> String {
        let mut option = EquityOption::example().expect("option");
        option.strike = -100.0;
        envelope_json(InstrumentJson::EquityOption(option))
    }

    fn fx_spot_spec_value() -> Value {
        serde_json::json!({
            "id": "EURUSD-SPOT",
            "base_currency": "EUR",
            "quote_currency": "USD",
            "settlement": "2025-01-17",
            "spot_rate": 1.20,
            "notional": {"amount": "1000000", "currency": "EUR"},
            "attributes": {},
        })
    }

    #[test]
    fn instrument_envelope_from_spec_wraps_bare_fx_spec() {
        let canonical = instrument_envelope_from_spec("fx_spot", fx_spot_spec_value())
            .expect("canonical fx spot");
        let parsed: Value = serde_json::from_str(&canonical).expect("json");
        assert_eq!(parsed["schema"], InstrumentEnvelope::CURRENT_SCHEMA);
        assert_eq!(parsed["instrument"]["type"], "fx_spot");
        assert_eq!(parsed["instrument"]["spec"]["id"], "EURUSD-SPOT");
    }

    #[test]
    fn instrument_envelope_from_spec_rejects_tagged_payload() {
        let err = instrument_envelope_from_spec(
            "fx_forward",
            serde_json::json!({"type": "fx_spot", "spec": fx_spot_spec_value()}),
        )
        .expect_err("tagged payload should be rejected");
        assert!(err
            .to_string()
            .contains("constructor requires a bare spec object"));
    }

    #[test]
    fn validate_typed_instrument_json_rejects_other_envelope_type() {
        let fx_spot = instrument_envelope_from_spec("fx_spot", fx_spot_spec_value())
            .expect("canonical fx spot");
        let err = validate_typed_instrument_json("fx_forward", &fx_spot)
            .expect_err("wrong envelope type should be rejected");
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
        assert_eq!(
            parsed["instrument"]["spec"]["metric_pricing_overrides"]["theta_period"],
            "1D"
        );
        assert_eq!(
            parsed["instrument"]["spec"]["metric_pricing_overrides"]["breakeven_config"]["target"],
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
    fn parse_boxed_instrument_from_json_rejects_invalid_pricing_overrides() {
        let Err(err) = parse_boxed_instrument_from_json(
            &equity_option_json_with_negative_vol_override(),
            None,
        ) else {
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
    fn validate_instrument_json_rejects_credit_and_convertible_invariants() {
        let mut cds_option = CDSOption::example().expect("CDS option");
        cds_option.exercise_style = crate::instruments::ExerciseStyle::American;
        let json = envelope_json(InstrumentJson::CDSOption(cds_option));
        assert!(validate_instrument_json(&json)
            .expect_err("unsupported exercise style must fail")
            .to_string()
            .contains("European"));

        let mut convertible = ConvertibleBond::example().expect("convertible");
        convertible.conversion.ratio = None;
        convertible.conversion.price = None;
        let json = envelope_json(InstrumentJson::ConvertibleBond(convertible));
        assert!(validate_instrument_json(&json)
            .expect_err("missing conversion terms must fail")
            .to_string()
            .contains("conversion.ratio"));
    }

    #[test]
    fn validate_instrument_json_rejects_invalid_revolving_credit_path() {
        let mut facility = RevolvingCredit::example().expect("revolving credit");
        facility.draw_repay_spec =
            crate::instruments::fixed_income::revolving_credit::DrawRepaySpec::Deterministic(vec![
                crate::instruments::fixed_income::revolving_credit::DrawRepayEvent {
                    date: facility.maturity + time::Duration::days(1),
                    amount: Money::from((1_000_000_i64, Currency::USD)),
                    is_draw: true,
                },
            ]);
        let json = envelope_json(InstrumentJson::RevolvingCredit(facility));
        assert!(validate_instrument_json(&json)
            .expect_err("post-maturity draw must fail")
            .to_string()
            .contains("maturity"));
    }

    #[test]
    fn validate_instrument_json_rejects_rates_and_securitized_invariants() {
        let mut future = InterestRateFuture::example().expect("IR future");
        future.contract_specs.convexity_adjustment = None;
        future.vol_surface_id = None;
        let json = envelope_json(InstrumentJson::InterestRateFuture(future));
        assert!(validate_instrument_json(&json)
            .expect_err("missing convexity source must fail")
            .to_string()
            .contains("convexity_adjustment"));

        let mut cmo = AgencyCmo::example().expect("CMO");
        cmo.reference_tranche_id = "MISSING".to_string();
        let json = envelope_json(InstrumentJson::AgencyCmo(cmo));
        assert!(validate_instrument_json(&json)
            .expect_err("unknown reference tranche must fail")
            .to_string()
            .contains("reference tranche"));
    }

    #[test]
    fn validate_instrument_json_rejects_invalid_term_loan_notional() {
        let mut loan = TermLoan::example().expect("term loan");
        loan.notional_limit = Money::from((-1_i64, Currency::USD));
        let json = envelope_json(InstrumentJson::TermLoan(loan));
        assert!(validate_instrument_json(&json)
            .expect_err("negative notional must fail")
            .to_string()
            .contains("notional_limit"));
    }

    #[test]
    fn validate_instrument_json_rejects_non_conserving_waterfall() {
        let fund = PrivateMarketsFund::example().expect("fund");
        let mut json = envelope_value(InstrumentJson::PrivateMarketsFund(fund));
        json["instrument"]["spec"]["waterfall_spec"]["tranches"][3]["promote_tier"]["gp_share"] =
            Value::from(0.6);
        let json = serde_json::to_string(&json).expect("json");

        validate_instrument_json(&json).expect_err("LP and GP shares above 100% must be rejected");
    }

    #[test]
    fn validate_instrument_json_rejects_invalid_cleanup_call_threshold() {
        let mut deal = StructuredCredit::example();
        deal.cleanup_call_pct = Some(-0.5);
        let json = envelope_json(InstrumentJson::StructuredCredit(Box::new(deal)));

        let err = validate_instrument_json(&json)
            .expect_err("cleanup-call threshold outside (0, 1) must be rejected");
        assert!(err.to_string().contains("cleanup_call_pct"));
    }

    #[test]
    fn validate_instrument_json_accepts_versioned_envelope() {
        let bond = Bond::fixed(
            "ENVELOPE-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            finstack_quant_core::dates::StubKind::ShortFront,
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
    fn parse_instrument_from_json_accepts_envelope_and_enforces_size_cap() {
        let json = bond_instrument_json();
        assert!(matches!(
            parse_instrument_from_json(&json).expect("envelope payload"),
            InstrumentJson::Bond(_)
        ));

        let oversized = " ".repeat(MAX_JSON_BYTES + 1);
        let error = parse_instrument_from_json(&oversized).expect_err("oversized payload fails");
        assert!(error.to_string().contains("size limit"), "{error}");
    }

    #[test]
    fn parse_boxed_instrument_from_json_rejects_domain_invariants() {
        let Err(err) =
            parse_boxed_instrument_from_json(&equity_option_json_with_invalid_strike(), None)
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

        let plain = price_instrument_from_json(
            &invalid,
            &market,
            "not-a-date",
            "not-a-model",
            &[],
            None,
            None,
        )
        .expect_err("instrument validation must win");
        let with_metrics = price_instrument_from_json(
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
        assert_eq!(
            parse_model_key("rates_credit").expect("ok"),
            ModelKey::RatesCredit
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
    fn price_instrument_from_json_prices_bond() {
        let result = price_instrument_from_json(
            &bond_instrument_json(),
            &market_context(),
            "2024-01-01",
            "discounting",
            &[],
            None,
            None,
        )
        .expect("price");
        assert_eq!(result.instrument_id, "TEST-BOND");
    }

    #[test]
    fn json_pricing_preserves_custom_registry_for_base_and_metric_repricing() {
        let parsed = parse_boxed_instrument_from_json(&bond_instrument_json(), None)
            .expect("validated bond JSON");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut pricers = PricerRegistry::new();
        pricers
            .register(FixedJsonBondPricer {
                calls: Arc::clone(&calls),
            })
            .expect("unique custom pricer");
        let metric_id = MetricId::custom("json_registry_reprice");
        let mut metrics = MetricRegistry::new();
        metrics
            .register_metric(
                metric_id.clone(),
                Arc::new(RepriceJsonMetric),
                &[InstrumentType::Bond],
            )
            .expect("unique custom metric");

        let result = price_instrument(
            &parsed,
            &MarketContext::new(),
            "2024-01-01",
            "discounting",
            &[metric_id.to_string()],
            None,
            crate::instruments::PricingOptions::default()
                .with_registry(Arc::new(pricers))
                .with_metric_registry(Arc::new(metrics)),
        )
        .expect("custom JSON registry must drive base and nested repricing");

        assert_eq!(result.value.amount(), 321.0);
        assert_eq!(result.measures.get(&metric_id), Some(&321.0));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "base PV and metric reprice must use the same custom registry"
        );
    }

    #[test]
    fn price_instrument_from_json_accepts_pricing_options() {
        let result = price_instrument_from_json(
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
    fn json_pricing_accepts_registered_custom_term_loan_metrics() {
        let loan = TermLoan::example().expect("term loan");
        let json = envelope_json(InstrumentJson::TermLoan(loan));
        let result = price_instrument_from_json(
            &json,
            &market_context(),
            "2024-01-01",
            "discounting",
            &["all_in_rate".to_string(), "yt2y".to_string()],
            None,
            None,
        )
        .expect("registered custom metrics must cross the JSON boundary");

        assert!(result.measures.contains_key("all_in_rate"));
        assert!(result.measures.contains_key("yt2y"));
    }

    #[test]
    fn revolving_credit_custom_metrics_use_as_of_drawn_balance() {
        use crate::instruments::fixed_income::revolving_credit::BaseRateSpec;

        let mut facility = RevolvingCredit::example().expect("revolving credit");
        facility.base_rate_spec = BaseRateSpec::Fixed { rate: 0.05 };
        let json = envelope_json(InstrumentJson::RevolvingCredit(facility));
        let result = price_instrument_from_json(
            &json,
            &market_context(),
            "2024-07-01",
            "discounting",
            &[
                "utilization_rate".to_string(),
                "available_capacity".to_string(),
            ],
            None,
            None,
        )
        .expect("registered revolving-credit metrics must cross the JSON boundary");

        assert_eq!(result.metric_str("utilization_rate"), Some(0.30));
        assert_eq!(result.metric_str("available_capacity"), Some(35_000_000.0));
    }

    #[test]
    fn price_instrument_from_json_rejects_unknown_metric_names() {
        let err = price_instrument_from_json(
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
    fn price_instrument_from_json_accepts_market_history_for_hvar() {
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

        let result = price_instrument_from_json(
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

        let result = price_instrument_from_json(
            &json,
            &market_context(),
            "2024-01-01",
            "discounting",
            &["dirty_price".to_string(), "vega".to_string()],
            None,
            None,
        )
        .expect("metrics");
        assert_eq!(result.metric_str("dirty_price"), Some(dirty_price));
        assert_eq!(result.metric_str("vega"), Some(0.0));
    }
}
