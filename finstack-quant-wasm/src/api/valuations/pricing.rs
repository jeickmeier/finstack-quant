//! WASM bindings for instrument pricing and metric introspection.
//!
//! CDS-family example payloads live in [`super::credit_derivatives`].
//! Structural credit-model factories live under `crate::api::models::credit`.
//!
//! # Monte-Carlo determinism
//!
//! `priceInstrument` (and its `Market` variant) accept Monte-Carlo models
//! (e.g. `monte_carlo_gbm`,
//! `monte_carlo_hull_white_1f`). These bindings deliberately expose **no**
//! explicit RNG-seed parameter: the seed is part of the *instrument*
//! contract, not the pricing call.
//!
//! The determinism guarantee is provided by the Rust core, not by these
//! wrappers: when an instrument's `metric_pricing_overrides.mc_seed_scenario`
//! is `None`, the core MC pricers derive a **stable** seed deterministically
//! from the instrument ID (see
//! `finstack_quant_valuations::instruments::InstrumentPricingOverrides`). Repricing the same
//! instrument JSON therefore yields bit-identical results without the caller
//! supplying a seed. Callers who need a distinct deterministic stream set
//! `mc_seed_scenario` inside the instrument JSON. This contract is verified by
//! `tests::price_instrument_mc_is_deterministic_without_explicit_seed`.

use super::market_handle::JsMarket;
use crate::utils::{to_js_err, to_js_error, to_js_value, to_js_value_with_bigints};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::results::ValuationResult;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Attach the host-owned cached recalibration provider.
///
/// Lives here rather than in `finstack-quant-valuations` because that crate
/// cannot depend on `finstack-quant-calibration`.
fn binding_pricing_options() -> PricingOptions {
    PricingOptions::default().with_recalibration_provider(Arc::new(
        finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new(),
    ))
}

pub(super) fn parse_market_json(market_json: &str) -> Result<MarketContext, JsValue> {
    serde_json::from_str(market_json).map_err(to_js_err)
}

pub(super) fn parse_pricing_instrument_json(
    instrument_json: &str,
    pricing_options: Option<&str>,
) -> Result<finstack_quant_valuations::pricer::ParsedInstrument, JsValue> {
    finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
        instrument_json,
        pricing_options,
    )
    .map_err(|e| to_js_error(&e))
}

pub(super) fn valuation_result_json(result: ValuationResult) -> Result<String, JsValue> {
    serde_json::to_string(&result).map_err(to_js_err)
}

/// Convert a [`ValuationResult`] into a structured JavaScript object.
///
/// The `price*` entry points return this rather than a JSON string so JS
/// callers read `result.value.amount` / `result.measures.dv01` directly, the
/// same way Python callers read `PyValuationResult` getters. It uses
/// JSON-compatible map/object conventions while preserving every 64-bit
/// integer as JavaScript `BigInt`. In particular, Monte Carlo seeds span the
/// full `u64` range and cannot be narrowed to a JavaScript `number`.
fn valuation_result_value(result: &ValuationResult) -> Result<JsValue, JsValue> {
    to_js_value_with_bigints(result)
}

pub(super) fn price_result_with_context(
    instrument: &finstack_quant_valuations::pricer::ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: Vec<String>,
    market_history_json: Option<&str>,
) -> Result<ValuationResult, JsValue> {
    finstack_quant_valuations::pricer::price_instrument(
        instrument,
        market,
        as_of,
        model,
        &metrics,
        market_history_json,
        binding_pricing_options(),
    )
    .map_err(|e| to_js_error(&e))
}

#[cfg(test)]
fn price_instrument_with_context(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: Vec<String>,
    pricing_options: Option<&str>,
    market_history_json: Option<&str>,
) -> Result<String, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, pricing_options)?;
    valuation_result_json(price_result_with_context(
        &instrument,
        market,
        as_of,
        model,
        metrics,
        market_history_json,
    )?)
}

/// Native-testable core of [`price_instrument`].
///
/// Kept separate from the `#[wasm_bindgen]` wrapper because `JsValue`
/// construction aborts on non-wasm32 targets, so the unit tests below drive
/// this function and assert on the `ValuationResult` directly.
#[cfg(test)]
fn price_instrument_result(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<&str>,
) -> Result<ValuationResult, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, None)?;
    let market = parse_market_json(market_json)?;
    price_result_with_context(
        &instrument,
        &market,
        as_of,
        model.unwrap_or("default"),
        Vec::new(),
        None,
    )
}

pub(super) fn metric_value_with_context(
    instrument: &finstack_quant_valuations::pricer::ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metric: &str,
) -> Result<f64, JsValue> {
    finstack_quant_valuations::pricer::metric_value(
        instrument,
        market,
        as_of,
        model,
        metric,
        binding_pricing_options(),
    )
    .map_err(to_js_err)
}

pub(super) fn standard_option_greeks_with_context(
    instrument: &finstack_quant_valuations::pricer::ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<Vec<(&'static str, f64)>, JsValue> {
    finstack_quant_valuations::pricer::present_standard_option_greeks(
        instrument,
        market,
        as_of,
        model,
        binding_pricing_options(),
    )
    .map_err(to_js_err)
}

/// Deserialize a `ValuationResult` from JSON and return the canonical JSON.
///
/// Validates the input conforms to the `ValuationResult` schema.
/// @param json - Canonical valuation-result JSON to validate and reserialize.
///
/// # Errors
///
/// Throws a JavaScript exception if `json` is malformed or does not match the
/// `ValuationResult` schema, or the canonical result cannot be serialized.
#[wasm_bindgen(js_name = validateValuationResultJson)]
pub fn validate_valuation_result_json(json: &str) -> Result<String, JsValue> {
    let result: ValuationResult = serde_json::from_str(json).map_err(to_js_err)?;
    valuation_result_json(result)
}

/// Validate a canonical v1 instrument envelope.
///
/// Deserializes the input against the known instrument schema and
/// returns the canonical (re-serialized) JSON.
/// @param json - Required `finstack_quant.instrument/1` envelope.
///
/// # Errors
///
/// Throws a JavaScript exception if `json` is malformed, is not a canonical v1
/// instrument envelope, fails instrument validation, or cannot be canonically
/// serialized.
#[wasm_bindgen(js_name = validateInstrumentJson)]
pub fn validate_instrument_json(json: &str) -> Result<String, JsValue> {
    finstack_quant_valuations::pricer::validate_instrument_json(json).map_err(|e| to_js_error(&e))
}

/// Construct a canonical bond instrument envelope from a cashflow schedule.
/// @param instrument_id - Stable instrument identifier used for pricing and metric keys.
/// @param schedule_json - Canonical cashflow-schedule JSON used to construct the fixed-income instrument.
/// @param discount_curve_id - Market-context discount-curve identifier for the instrument currency.
/// @param quoted_clean - Optional observed clean bond price in the schedule's documented price quotation convention.
///
/// # Errors
///
/// Throws a JavaScript exception if `scheduleJson` is malformed or violates
/// cash-flow invariants, bond construction fails, or the canonical bond
/// envelope cannot be serialized.
#[wasm_bindgen(js_name = bondFromCashflowsJson)]
pub fn bond_from_cashflows_json(
    instrument_id: &str,
    schedule_json: &str,
    discount_curve_id: &str,
    quoted_clean: Option<f64>,
) -> Result<String, JsValue> {
    finstack_quant_valuations::instruments::fixed_income::bond::bond_from_cashflows_json(
        instrument_id,
        schedule_json,
        discount_curve_id,
        quoted_clean,
    )
    .map_err(to_js_err)
}

/// Price an instrument from its canonical envelope and return a ValuationResult object.
///
/// Returns a plain JavaScript object (`instrument_id`, `as_of`, `value`,
/// `measures`, `meta`, …) — the same document Python callers reach through
/// `ValuationResult`.
///
/// Omit `model` (or pass `"default"`) to use the instrument-native default
/// model — matching the Python binding's `model="default"` default.
/// For bonds, `"discounting"` is non-callable rates-only PV,
/// `"hazard_rate"` is non-callable fractional recovery of par, `"tree"`
/// values rates-only exercise rights, and `"rates_credit"` values joint
/// rates-credit bonds including call, put, and return floors. When stochastic
/// factors are configured under `"rates_credit"`, `result.details` is tagged
/// `{ type: "monte_carlo", data: ... }` and reports the standard error, path
/// counts, random seed, simulation time grid, and variance-reduction flags.
/// That seed is a lossless JavaScript `BigInt`; callers serializing stochastic
/// results must use a BigInt-aware replacer, for example
/// `JSON.stringify(result, (_, value) => typeof value === "bigint" ? value.toString() : value)`.
/// @param instrument_json - Required `finstack_quant.instrument/1` envelope.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Optional pricing-model identifier; omit for the instrument-native model.
/// @param metrics - Optional canonical metric IDs such as `"ytm"`, `"dv01"`,
/// `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a
/// valuation-only result.
/// @param pricing_options - Optional JSON metric-pricing overrides merged into
/// the envelope before validation. Omit, `null`, or `undefined` to use the
/// envelope as-is.
/// @param market_history - Optional serialized market-history JSON required by
/// historical risk metrics such as historical VaR.
/// @returns Plain JavaScript `ValuationResult` (`instrument_id`, `as_of`,
/// `value`, `measures`, `meta`, …).
///
/// # Errors
///
/// Throws a JavaScript exception if an instrument, market, pricing-option, or
/// market-history payload is invalid; `metrics` is not a string array; `asOf`,
/// `model`, or a metric identifier is invalid; required market data is missing;
/// pricing or a metric calculation fails; or the valuation cannot be converted
/// to a JavaScript value.
#[wasm_bindgen(js_name = priceInstrument)]
pub fn price_instrument(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<String>,
    metrics: Option<JsValue>,
    pricing_options: Option<String>,
    market_history: Option<String>,
) -> Result<JsValue, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, pricing_options.as_deref())?;
    let market = parse_market_json(market_json)?;
    let model = model.as_deref().unwrap_or("default");
    let metric_strs: Vec<String> = match metrics {
        None => Vec::new(),
        Some(value) if value.is_undefined() || value.is_null() => Vec::new(),
        Some(value) => serde_wasm_bindgen::from_value(value).map_err(to_js_err)?,
    };
    let result = price_result_with_context(
        &instrument,
        &market,
        as_of,
        model,
        metric_strs,
        market_history.as_deref(),
    )?;
    valuation_result_value(&result)
}

/// Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.
///
/// `model` must be `"discounting"` or `"hazard_rate"`. Unsupported models or
/// incompatible instrument types throw. Hazard-rate export also rejects bonds
/// with call, put, or return-floor rights because static rows cannot represent
/// exercise-contingent value. For supported static-flow pairs, the envelope's
/// `total_pv` matches the instrument's `base_value` within rounding.
/// @param instrument_json - Required `finstack_quant.instrument/1` envelope.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
///
/// # Errors
///
/// Throws a JavaScript exception if the instrument or market JSON or `asOf` is
/// invalid, `model` is unsupported or incompatible with the instrument, a
/// bond with embedded exercise rights is requested under a static cashflow
/// model, required curves are missing, the schedule mixes currencies,
/// canonical pricing fails, or the cash-flow envelope cannot be serialized.
#[wasm_bindgen(js_name = instrumentCashflowsJson)]
pub fn instrument_cashflows_json(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: &str,
) -> Result<String, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, None)?;
    let market = parse_market_json(market_json)?;
    let envelope = finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
        &instrument,
        &market,
        as_of,
        model,
    )
    .map_err(|e| to_js_error(&e))?;
    serde_json::to_string(&envelope).map_err(to_js_err)
}

/// List all metric IDs in the standard metric registry.
///
/// # Errors
///
/// Throws a JavaScript exception if the metric identifier list cannot be
/// converted to a JavaScript value.
#[wasm_bindgen(js_name = listStandardMetrics)]
pub fn list_standard_metrics() -> Result<JsValue, JsValue> {
    let ids = finstack_quant_valuations::pricer::list_standard_metrics();
    to_js_value(&ids)
}

/// List all standard metrics organized by group.
///
/// Returns a JSON object `{ group_name: [metric_id, ...], ... }` where
/// each key is a human-readable group name (e.g. "Pricing", "Greeks",
/// "Sensitivity") and the value is a sorted array of metric ID strings.
///
/// # Errors
///
/// Throws a JavaScript exception if the grouped metric registry cannot be
/// converted to a JavaScript value.
#[wasm_bindgen(js_name = listStandardMetricsGrouped)]
pub fn list_standard_metrics_grouped() -> Result<JsValue, JsValue> {
    let map = finstack_quant_valuations::pricer::list_standard_metrics_grouped();
    to_js_value(&map)
}

/// List every pricing model key registered in the standard pricer registry.
///
/// The list is registry-derived rather than enum-derived, so it reflects real
/// dispatch coverage: a model with no registered pricer is omitted. Returns a
/// sorted array of canonical keys (`"discounting"`, `"rates_credit"`, …)
/// accepted by the `model` argument of `priceInstrument`.
///
/// # Errors
///
/// Throws a JavaScript exception if the model key list cannot be converted to
/// a JavaScript value.
#[wasm_bindgen(js_name = listModels)]
pub fn list_models() -> Result<JsValue, JsValue> {
    let models = finstack_quant_valuations::pricer::list_models();
    to_js_value(&models)
}

/// List the standard registry's pricing models grouped by instrument type.
///
/// Returns a JSON object `{ instrument_type: [model_key, ...], ... }`. Only
/// instrument types with at least one registered pricer appear, and each entry
/// lists only the models that can actually price that instrument. The
/// `"bond"` entry includes `"discounting"`, `"hazard_rate"`, `"tree"`, and
/// `"rates_credit"`.
///
/// # Errors
///
/// Throws a JavaScript exception if the grouped model registry cannot be
/// converted to a JavaScript value.
#[wasm_bindgen(js_name = listModelsGrouped)]
pub fn list_models_grouped() -> Result<JsValue, JsValue> {
    let grouped = finstack_quant_valuations::pricer::list_models_grouped();
    to_js_value(&grouped)
}

/// Return the maintained liquid listed-derivatives coverage catalog.
///
/// The catalog maps stable CME, Eurex, Montréal, and SGX product families to
/// canonical Finstack instrument types and names residual model gaps. It is not
/// a live contract-month or liquidity feed.
/// @param exchange - Optional exact filter: `"cme"`, `"eurex"`, `"montreal"`, or `"sgx"`.
/// @returns Product-family coverage rows with instrument routes and official source URLs.
/// @throws Error - Throws when `exchange` is unsupported, the embedded listed-product sidecar is invalid, or rows cannot be converted to JavaScript.
#[wasm_bindgen(js_name = listedProductCatalog)]
pub fn listed_product_catalog(exchange: Option<String>) -> Result<JsValue, JsValue> {
    let exchange = exchange
        .as_deref()
        .map(str::parse::<finstack_quant_valuations::market::listed::ListedExchange>)
        .transpose()
        .map_err(|error| JsValue::from_str(&error))?;
    let rows = finstack_quant_valuations::market::listed::listed_product_catalog(exchange)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_js_value(&rows)
}

// JsMarket overloads — parse market once, reuse across pricing calls

/// Price an instrument using a pre-parsed [`JsMarket`].
///
/// Avoids the per-call market-parse overhead of `priceInstrument`. Returns the
/// same plain JavaScript ValuationResult object. For bonds, `"discounting"`
/// is non-callable rates-only PV, `"hazard_rate"` is non-callable fractional
/// recovery of par, `"tree"` values rates-only exercise rights, and
/// `"rates_credit"` values joint rates-credit bonds including call, put, and
/// return floors. Stochastic `"rates_credit"` runs add tagged Monte Carlo diagnostics to
/// `result.details`. Their `seed` is a lossless JavaScript `BigInt`, so
/// `JSON.stringify` requires a BigInt-aware replacer.
/// @param instrument_json - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param market - Pre-parsed `Market` handle supplying curves, quotes, and FX data for this call.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
/// @param metrics - Optional canonical metric IDs such as `"ytm"`, `"dv01"`,
/// `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a
/// valuation-only result.
/// @param pricing_options - Optional JSON metric-pricing overrides merged into
/// the envelope before validation. Omit, `null`, or `undefined` to use the
/// envelope as-is.
/// @param market_history - Optional serialized market-history JSON required by
/// historical risk metrics such as historical VaR.
/// @returns Plain JavaScript `ValuationResult` (`instrument_id`, `as_of`,
/// `value`, `measures`, `meta`, …).
///
/// # Errors
///
/// Throws a JavaScript exception if an instrument, pricing-option, or market-
/// history payload is invalid; `metrics` is not a string array; `asOf`, `model`,
/// or a metric identifier is invalid; required market data is missing; pricing
/// or a metric calculation fails; or the valuation cannot be converted to a
/// JavaScript value.
#[wasm_bindgen(js_name = priceInstrumentWithMarket)]
pub fn price_instrument_with_market(
    instrument_json: &str,
    market: &JsMarket,
    as_of: &str,
    model: &str,
    metrics: Option<JsValue>,
    pricing_options: Option<String>,
    market_history: Option<String>,
) -> Result<JsValue, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, pricing_options.as_deref())?;
    let metric_strs: Vec<String> = match metrics {
        None => Vec::new(),
        Some(value) if value.is_undefined() || value.is_null() => Vec::new(),
        Some(value) => serde_wasm_bindgen::from_value(value).map_err(to_js_err)?,
    };
    let result = price_result_with_context(
        &instrument,
        market.inner(),
        as_of,
        model,
        metric_strs,
        market_history.as_deref(),
    )?;
    valuation_result_value(&result)
}

/// Per-flow cashflow envelope using a pre-parsed [`JsMarket`]. Hazard-rate
/// export rejects bonds with call, put, or return-floor rights because static
/// rows cannot represent exercise-contingent value.
/// @param instrument_json - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param market - Market context or JSON payload supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
///
/// # Errors
///
/// Throws a JavaScript exception if `instrumentJson` or `asOf` is invalid,
/// `model` is unsupported or incompatible with the instrument, a bond with
/// embedded exercise rights is requested under a static cashflow model,
/// required curves are missing, the schedule mixes currencies, canonical
/// pricing fails, or the cash-flow envelope cannot be serialized.
#[wasm_bindgen(js_name = instrumentCashflowsWithMarketJson)]
pub fn instrument_cashflows_with_market_json(
    instrument_json: &str,
    market: &JsMarket,
    as_of: &str,
    model: &str,
) -> Result<String, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, None)?;
    let envelope = finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
        &instrument,
        market.inner(),
        as_of,
        model,
    )
    .map_err(|e| to_js_error(&e))?;
    serde_json::to_string(&envelope).map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope_json(instrument: finstack_quant_valuations::instruments::InstrumentJson) -> String {
        serde_json::to_string(
            &finstack_quant_valuations::instruments::InstrumentEnvelope::new(instrument),
        )
        .expect("serialize instrument envelope")
    }

    #[test]
    fn parse_model_key_recognizes_standard_keys() {
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("discounting").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::Discounting
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("tree").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::Tree
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("black76").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::Black76
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("hull_white_1f").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::HullWhite1F
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("hazard_rate").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::HazardRate
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("rates_credit").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::RatesCredit
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("normal").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::Normal
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("monte_carlo_gbm").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::MonteCarloGBM
        );
    }

    #[test]
    fn public_json_routes_validate_instrument_before_market_json() {
        assert!(price_instrument(
            "{}",
            "not-market-json",
            "not-a-date",
            Some("not-a-model".to_string()),
            None,
            None,
            None,
        )
        .is_err());
        assert!(
            instrument_cashflows_json("{}", "not-market-json", "not-a-date", "not-a-model",)
                .is_err()
        );
    }

    pub(crate) fn bond_instrument_json() -> String {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::Rate;
        use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
        use finstack_quant_valuations::instruments::InstrumentJson;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            Rate::from_decimal(0.05).expect("valid rate fixture"),
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        envelope_json(InstrumentJson::Bond(bond))
    }

    fn revolving_credit_json(invalid_gearing: bool, with_credit: bool) -> String {
        use finstack_quant_valuations::instruments::fixed_income::revolving_credit::BaseRateSpec;
        use finstack_quant_valuations::instruments::{InstrumentJson, RevolvingCredit};

        let mut facility = RevolvingCredit::example().expect("facility");
        if invalid_gearing {
            let BaseRateSpec::Floating(spec) = &mut facility.base_rate_spec else {
                unreachable!("example is floating");
            };
            spec.gearing = Default::default();
        } else {
            facility.base_rate_spec = BaseRateSpec::Fixed { rate: 0.05 };
        }
        if with_credit {
            facility.credit_curve_id = Some("USD-HZ".into());
            facility.recovery_rate = 0.4;
        }
        envelope_json(InstrumentJson::RevolvingCredit(facility))
    }

    fn revolving_credit_market(with_credit: bool) -> MarketContext {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
        use time::macros::date;

        let mut market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(date!(2024 - 01 - 01))
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
                .build()
                .expect("discount curve"),
        );
        if with_credit {
            market = market.insert(
                HazardCurve::builder("USD-HZ")
                    .base_date(date!(2024 - 01 - 01))
                    .recovery_rate(0.4)
                    .knots([(1.0, 0.02), (5.0, 0.02)])
                    .build()
                    .expect("hazard curve"),
            );
        }
        market
    }

    pub(crate) fn bermudan_swaption_json() -> String {
        use finstack_quant_valuations::instruments::rates::swaption::BermudanSwaption;
        use finstack_quant_valuations::instruments::InstrumentJson;

        envelope_json(InstrumentJson::BermudanSwaption(BermudanSwaption::example()))
    }

    pub(crate) fn tarn_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount, Tenor};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::exotics::tarn::Tarn;
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let mut instrument_pricing_overrides = InstrumentPricingOverrides::default();
        instrument_pricing_overrides.model_config.mc_paths = Some(32);
        instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        instrument_pricing_overrides.model_config.hw1f_sigma = Some(1e-12);

        let tarn = Tarn {
            id: InstrumentId::new("TARN-WASM-E2E"),
            fixed_rate: 0.06,
            coupon_floor: 0.0,
            target_coupon: 1.0,
            notional: Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD)
                .expect("valid money fixture"),
            coupon_dates: vec![
                Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                Date::from_calendar_date(2026, Month::January, 1).expect("date"),
                Date::from_calendar_date(2026, Month::July, 1).expect("date"),
            ],
            floating_tenor: Tenor::semi_annual(),
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            day_count: DayCount::Act365F,
            instrument_pricing_overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        envelope_json(InstrumentJson::Tarn(tarn))
    }

    pub(crate) fn snowball_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount, Tenor};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::exotics::snowball::{
            Snowball, SnowballVariant,
        };
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let mut instrument_pricing_overrides = InstrumentPricingOverrides::default();
        instrument_pricing_overrides.model_config.mc_paths = Some(32);
        instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        instrument_pricing_overrides.model_config.hw1f_sigma = Some(1e-12);

        let snowball = Snowball {
            id: InstrumentId::new("SNOWBALL-WASM-E2E"),
            variant: SnowballVariant::Snowball,
            initial_coupon: 0.03,
            fixed_rate: 0.05,
            leverage: 1.0,
            coupon_floor: 0.0,
            coupon_cap: None,
            notional: Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD)
                .expect("valid money fixture"),
            coupon_dates: vec![
                Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                Date::from_calendar_date(2026, Month::January, 1).expect("date"),
                Date::from_calendar_date(2026, Month::July, 1).expect("date"),
            ],
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            floating_tenor: Tenor::semi_annual(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            callable: None,
            day_count: DayCount::Act365F,
            instrument_pricing_overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        envelope_json(InstrumentJson::Snowball(snowball))
    }

    pub(crate) fn inverse_floater_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount, Tenor};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::exotics::snowball::{
            Snowball, SnowballVariant,
        };
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let inverse_floater = Snowball {
            id: InstrumentId::new("INV-FLOATER-WASM-E2E"),
            variant: SnowballVariant::InverseFloater,
            initial_coupon: 0.0,
            fixed_rate: 0.08,
            leverage: 1.5,
            coupon_floor: 0.0,
            coupon_cap: Some(0.10),
            notional: Money::new(500_000.0, finstack_quant_core::currency::Currency::USD)
                .expect("valid money fixture"),
            coupon_dates: vec![
                Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                Date::from_calendar_date(2026, Month::January, 1).expect("date"),
                Date::from_calendar_date(2026, Month::July, 1).expect("date"),
            ],
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            floating_tenor: Tenor::semi_annual(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            callable: None,
            day_count: DayCount::Act365F,
            instrument_pricing_overrides: InstrumentPricingOverrides::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        envelope_json(InstrumentJson::Snowball(inverse_floater))
    }

    pub(crate) fn callable_range_accrual_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::exotics::callable_range_accrual::CallableRangeAccrual;
        use finstack_quant_valuations::instruments::exotics::range_accrual::{
            BoundsType, RangeAccrual,
        };
        use finstack_quant_valuations::instruments::rates::hw1f::bermudan_call::BermudanCallProvision;
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let mut instrument_pricing_overrides = InstrumentPricingOverrides::default();
        instrument_pricing_overrides.model_config.mc_paths = Some(8);
        instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        instrument_pricing_overrides.model_config.hw1f_sigma = Some(1e-12);

        let range_accrual = RangeAccrual::builder()
            .id(InstrumentId::new("RA-WASM-E2E"))
            .underlying_ticker("SOFR".to_string())
            .observation_dates(vec![
                Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                Date::from_calendar_date(2026, Month::January, 1).expect("date"),
                Date::from_calendar_date(2026, Month::July, 1).expect("date"),
            ])
            .lower_bound(0.02)
            .upper_bound(0.04)
            .bounds_type(BoundsType::Absolute)
            .coupon_rate(0.06)
            .notional(
                Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD)
                    .expect("valid money fixture"),
            )
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .accrual_start_date(Date::from_calendar_date(2025, Month::January, 1).expect("date"))
            .rate_index_id_opt(Some("SOFR".into()))
            .projection_curve_id_opt(Some(CurveId::new("USD-OIS")))
            .reference_tenor_opt(Some(
                finstack_quant_core::dates::Tenor::new(
                    6,
                    finstack_quant_core::dates::TenorUnit::Months,
                )
                .expect("valid tenor fixture"),
            ))
            .spot_id("SOFR-RATE".into())
            .vol_surface_id(CurveId::new("SOFR-VOL"))
            .div_yield_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Default::default())
            .payment_date_opt(None)
            .past_fixings_in_range_opt(None)
            .total_past_observations_opt(None)
            .build()
            .expect("range accrual");

        let callable = CallableRangeAccrual {
            id: InstrumentId::new("CALLABLE-RA-WASM-E2E"),
            range_accrual,
            call_provision: BermudanCallProvision::new(
                vec![Date::from_calendar_date(2025, Month::July, 1).expect("date")],
                1.0,
                0,
            ),
            instrument_pricing_overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        envelope_json(InstrumentJson::CallableRangeAccrual(Box::new(callable)))
    }

    pub(crate) fn cms_spread_option_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount, Tenor, TenorUnit};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::rates::cms_spread_option::{
            CmsSpreadOption, CmsSpreadOptionType,
        };
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let option = CmsSpreadOption {
            id: InstrumentId::new("CMS-SPREAD-WASM-E2E"),
            long_cms_tenor: Tenor::new(10, TenorUnit::Years).expect("valid tenor fixture"),
            short_cms_tenor: Tenor::new(2, TenorUnit::Years).expect("valid tenor fixture"),
            strike: 0.005,
            option_type: CmsSpreadOptionType::Call,
            notional: Money::new(10_000_000.0, finstack_quant_core::currency::Currency::USD)
                .expect("valid money fixture"),
            expiry_date: Date::from_calendar_date(2026, Month::January, 1).expect("date"),
            payment_date: Date::from_calendar_date(2026, Month::January, 5).expect("date"),
            long_vol_surface_id: CurveId::new("USD-SWAPTION-VOL-10Y"),
            short_vol_surface_id: CurveId::new("USD-SWAPTION-VOL-2Y"),
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            spread_correlation: 0.5,
            day_count: DayCount::Act365F,
            swap_convention: None,
            swap_fixed_frequency: None,
            swap_float_frequency: None,
            swap_day_count: None,
            swap_float_day_count: None,
            instrument_pricing_overrides: InstrumentPricingOverrides::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        envelope_json(InstrumentJson::CmsSpreadOption(option))
    }

    pub(crate) fn market_context_json() -> String {
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        let base = time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.5, 0.99), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
            .build()
            .expect("curve");
        let ctx = MarketContext::new().insert(disc);
        serde_json::to_string(&ctx).expect("serialize")
    }

    pub(crate) fn tarn_market_context_json() -> String {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::scalars::MarketScalar;
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
        let base = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (6.0, (-0.02_f64 * 6.0).exp())])
            .build()
            .expect("discount curve");
        let fwd = ForwardCurve::builder("USD-SOFR-6M", 0.5)
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.03), (6.0, 0.03)])
            .build()
            .expect("forward curve");
        let ctx = MarketContext::new()
            .insert(disc)
            .insert(fwd)
            .insert_price("SOFR-RATE", MarketScalar::Unitless(0.03));
        serde_json::to_string(&ctx).expect("serialize")
    }

    pub(crate) fn cms_spread_market_context_json() -> String {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

        fn sabr_cube(id: &str, alpha: f64, forward: f64) -> VolCube {
            let params =
                SabrParameterData::new(alpha, 0.5, -0.20, 0.40).expect("valid SABR params");
            VolCube::builder(id)
                .expiries(&[0.25, 1.0, 5.0])
                .tenors(&[2.0, 10.0])
                .node(params, forward)
                .node(params, forward)
                .node(params, forward)
                .node(params, forward)
                .node(params, forward)
                .node(params, forward)
                .build()
                .expect("vol cube")
        }

        let base = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (30.0, (-0.035_f64 * 30.0).exp())])
            .build()
            .expect("discount curve");
        let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.025), (2.0, 0.030), (10.0, 0.045), (30.0, 0.055)])
            .build()
            .expect("forward curve");
        let ctx = MarketContext::new()
            .insert(disc)
            .insert(fwd)
            .insert_vol_cube(sabr_cube("USD-SWAPTION-VOL-10Y", 0.035, 0.045))
            .insert_vol_cube(sabr_cube("USD-SWAPTION-VOL-2Y", 0.035, 0.030));
        serde_json::to_string(&ctx).expect("serialize")
    }

    /// Price through the native-testable core and expose the canonical
    /// serde document the `priceInstrument` binding hands to JavaScript.
    ///
    /// The `#[wasm_bindgen]` wrappers return `JsValue`, whose construction
    /// aborts on non-wasm32 targets, so unit tests drive the core instead. The
    /// JS-object shape itself is covered by `tests/wasm_valuations.rs` and
    /// `tests/facade/return_floor.test.mjs`.
    fn priced_document(
        instrument_json: &str,
        market_json: &str,
        as_of: &str,
        model: &str,
    ) -> serde_json::Value {
        let result = price_instrument_result(instrument_json, market_json, as_of, Some(model))
            .expect("price");
        serde_json::to_value(&result).expect("serialize valuation result")
    }

    fn amount_from_result(parsed: &serde_json::Value) -> f64 {
        parsed["value"]["amount"]
            .as_f64()
            .or_else(|| {
                parsed["value"]["amount"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .expect("amount")
    }

    #[test]
    fn validate_instrument_json_bond() {
        let json = bond_instrument_json();
        let canonical = validate_instrument_json(&json).expect("validate");
        assert!(!canonical.is_empty());
    }

    #[test]
    fn validate_instrument_json_bermudan_swaption() {
        let json = bermudan_swaption_json();
        let canonical = validate_instrument_json(&json).expect("validate");
        let parsed: serde_json::Value = serde_json::from_str(&canonical).expect("json");
        assert_eq!(parsed["instrument"]["type"], "bermudan_swaption");
    }

    #[test]
    fn validate_revolving_credit_rejects_invalid_floating_rate_spec() {
        assert!(validate_instrument_json(&revolving_credit_json(true, false)).is_err());
    }

    #[test]
    fn revolving_credit_metrics_and_cashflow_fail_closed_cross_wasm_context() {
        let instrument = revolving_credit_json(false, false);
        let market = revolving_credit_market(false);
        let result = price_instrument_with_context(
            &instrument,
            &market,
            "2024-07-01",
            "discounting",
            vec![
                "utilization_rate".to_string(),
                "available_capacity".to_string(),
            ],
            None,
            None,
        )
        .expect("metrics");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("result");
        assert_eq!(parsed["measures"]["utilization_rate"], 0.30);
        assert_eq!(parsed["measures"]["available_capacity"], 35_000_000.0);

        let credit_instrument = revolving_credit_json(false, true);
        let credit_market_json =
            serde_json::to_string(&revolving_credit_market(true)).expect("market json");
        let credit_market = JsMarket::new(&credit_market_json).expect("market handle");
        assert!(instrument_cashflows_with_market_json(
            &credit_instrument,
            &credit_market,
            "2024-01-01",
            "discounting",
        )
        .is_err());
    }

    #[test]
    fn price_instrument_bond() {
        let parsed = priced_document(
            &bond_instrument_json(),
            &market_context_json(),
            "2024-01-01",
            "discounting",
        );
        assert!(parsed.is_object());
        assert_eq!(parsed["instrument_id"], "TEST-BOND");
    }

    #[test]
    fn wasm_market_reuses_parsed_market_for_pricing_and_cashflows() {
        let inst = bond_instrument_json();
        let market = JsMarket::new(&market_context_json()).expect("market handle");
        let instrument = parse_pricing_instrument_json(&inst, None).expect("parsed instrument");

        let priced = price_result_with_context(
            &instrument,
            market.inner(),
            "2024-01-01",
            "discounting",
            Vec::new(),
            None,
        )
        .expect("price");
        let parsed = serde_json::to_value(&priced).expect("price json");
        assert!(parsed.is_object());

        let cashflows =
            instrument_cashflows_with_market_json(&inst, &market, "2024-01-01", "discounting")
                .expect("cashflows");
        let parsed_cashflows: serde_json::Value =
            serde_json::from_str(&cashflows).expect("cashflow json");
        assert!(parsed_cashflows.is_object());
    }

    #[test]
    fn price_instrument_tarn_hull_white_mc() {
        let parsed = priced_document(
            &tarn_json(),
            &tarn_market_context_json(),
            "2025-01-01",
            "monte_carlo_hull_white_1f",
        );
        let amount = amount_from_result(&parsed);
        assert!(amount > 0.0);
        assert_eq!(parsed["measures"]["mc_num_paths"], 32.0);
    }

    #[test]
    fn price_instrument_mc_is_deterministic_without_explicit_seed() {
        // The MC pricing bindings expose no explicit seed parameter. The
        // determinism contract (documented at the module level) is that the
        // Rust core derives a stable seed from the instrument ID, so repricing
        // the *same* instrument JSON yields bit-identical priced values and MC
        // diagnostics. Only the wall-clock `meta.timestamp` differs between
        // calls, so the assertion targets `value` and `measures` rather than
        // the whole serialized envelope.
        let inst = tarn_json();
        let mkt = tarn_market_context_json();
        let first_parsed = priced_document(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f");
        let second_parsed = priced_document(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f");
        assert_eq!(
            first_parsed["value"], second_parsed["value"],
            "MC priced value must be deterministic across repeated calls with no explicit seed"
        );
        assert_eq!(
            first_parsed["measures"], second_parsed["measures"],
            "MC diagnostics (paths, stderr, CI) must be deterministic across repeated calls"
        );
    }

    #[test]
    fn price_instrument_snowball_hull_white_mc() {
        let parsed = priced_document(
            &snowball_json(),
            &tarn_market_context_json(),
            "2025-01-01",
            "monte_carlo_hull_white_1f",
        );
        assert!(amount_from_result(&parsed) > 0.0);
        assert_eq!(parsed["measures"]["mc_num_paths"], 32.0);
    }

    #[test]
    fn price_instrument_inverse_floater_discounting() {
        let parsed = priced_document(
            &inverse_floater_json(),
            &tarn_market_context_json(),
            "2025-01-01",
            "discounting",
        );
        assert!(amount_from_result(&parsed) > 0.0);
    }

    #[test]
    fn price_instrument_callable_range_accrual_hull_white_mc() {
        let parsed = priced_document(
            &callable_range_accrual_json(),
            &tarn_market_context_json(),
            "2025-01-01",
            "monte_carlo_hull_white_1f",
        );
        assert!(amount_from_result(&parsed) > 0.0);
        assert_eq!(parsed["measures"]["mc_num_paths"], 8.0);
    }

    #[test]
    fn price_instrument_cms_spread_option_static_replication() {
        let parsed = priced_document(
            &cms_spread_option_json(),
            &cms_spread_market_context_json(),
            "2025-01-01",
            "static_replication",
        );
        assert!(amount_from_result(&parsed) > 0.0);
        assert!(
            parsed["measures"]["cms_spread_forward"]
                .as_f64()
                .expect("cms spread forward")
                > 0.0
        );
    }

    #[test]
    fn validate_valuation_result_json_roundtrip() {
        // JS callers reach this by stringifying the object `priceInstrument`
        // returns; the wire document is unchanged by that conversion.
        let result = price_instrument_result(
            &bond_instrument_json(),
            &market_context_json(),
            "2024-01-01",
            Some("discounting"),
        )
        .expect("price");
        let result_json = valuation_result_json(result).expect("serialize");
        let canonical = validate_valuation_result_json(&result_json).expect("validate");
        assert!(!canonical.is_empty());
        let parsed: serde_json::Value = serde_json::from_str(&canonical).expect("json");
        assert!(parsed.is_object());
    }

    /// Build a floored-bond InstrumentJson string from a raw JSON spec.
    ///
    /// Uses the same 5-year 10% annual bullet as the Python `test_return_floor.py`
    /// fixture so the two surfaces stay directly comparable.
    fn return_floor_bond_instrument_json(return_floor: serde_json::Value) -> String {
        let spec = serde_json::json!({
            "id": "WASM-RETURN-FLOOR-BOND",
            "notional": { "amount": "1000000", "currency": "USD" },
            "issue_date": "2024-01-01",
            "maturity": "2029-01-01",
            "cashflow_spec": {
                "fixed": {
                    "rate": "0.10",
                    "frequency": { "count": 12, "unit": "months" },
                    "day_count": "30_360",
                    "business_day_convention": "following",
                    "calendar_id": "weekends_only"
                }
            },
            "discount_curve_id": "USD-OIS",
            "settlement_days": 0,
            "ex_coupon_days": 0,
            "attributes": {},
            "return_floor": return_floor
        });
        let bond = serde_json::from_value(spec).expect("valid return-floor bond fixture");
        envelope_json(finstack_quant_valuations::instruments::InstrumentJson::Bond(bond))
    }

    /// Minimal 5-year flat discount market for the return-floor tests.
    fn return_floor_market_json() -> String {
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::interp::InterpStyle;
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(time::macros::date!(2024 - 01 - 01))
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .interp(InterpStyle::MonotoneConvex)
            .build()
            .expect("valid discount curve");
        serde_json::to_string(&MarketContext::new().insert(curve)).expect("serialize market")
    }

    #[test]
    fn return_floor_bond_moic_floor_validates_and_prices() {
        // Smoke test: a bond with a 1.25× MOIC return-floor spec round-trips
        // through the JSON validator and prices successfully via the rates tree
        // model — no new Rust binding code is required; return_floor is already a
        // serde field on the core Bond type.
        let floor_spec = serde_json::json!({
            "kind": { "moic": 1.25 },
            "issue_price": "par",
            "window": "full"
        });
        let inst = return_floor_bond_instrument_json(floor_spec);

        // Validate round-trips through the binding
        let canonical = validate_instrument_json(&inst).expect("validate");
        assert!(
            canonical.contains("return_floor"),
            "return_floor survived round-trip"
        );

        // Price and check the four return metrics via the internal helper
        // (avoids serde_wasm_bindgen which requires a wasm32 target).
        let mkt = return_floor_market_json();
        let market = parse_market_json(&mkt).expect("market");
        let metrics = vec![
            "moic".to_string(),
            "moic_to_worst".to_string(),
            "xirr".to_string(),
            "xirr_to_worst".to_string(),
        ];
        let result_json = price_instrument_with_context(
            &inst,
            &market,
            "2024-01-01",
            "tree",
            metrics,
            None,
            None,
        )
        .expect("price_with_metrics");
        let parsed: serde_json::Value = serde_json::from_str(&result_json).expect("parse");

        // 10% annual 5Y par bullet: MOIC ≈ 1.50 (5 × 0.10 + 1.0 principal)
        let moic = parsed["measures"]["moic"].as_f64().expect("moic");
        assert!(moic > 1.0, "MOIC must be > 1.0 (coupon income)");
        assert!((moic - 1.50).abs() < 0.02, "MOIC ≈ 1.50, got {moic}");

        // XIRR ≈ 10% for a par bullet
        let xirr = parsed["measures"]["xirr"].as_f64().expect("xirr");
        assert!((xirr - 0.10).abs() < 0.005, "XIRR ≈ 0.10, got {xirr}");

        // moic_to_worst ≤ moic: the floored bond has synthetic call options injected
        // by the return-floor machinery, so the worst-exit path can only be equal
        // to or worse than the held-to-maturity multiple.
        let moic_tw = parsed["measures"]["moic_to_worst"]
            .as_f64()
            .expect("moic_to_worst");
        assert!(
            moic_tw <= moic + 1e-9,
            "moic_to_worst must be ≤ moic, got {moic_tw} vs {moic}"
        );
    }

    #[test]
    fn return_floor_bond_xirr_floor_prices_without_error() {
        let floor_spec = serde_json::json!({
            "kind": { "xirr": 0.12 },
            "issue_price": "par",
            "window": "full"
        });
        let inst = return_floor_bond_instrument_json(floor_spec);
        let mkt = return_floor_market_json();
        let market = parse_market_json(&mkt).expect("market");
        let metrics = vec!["xirr".to_string(), "xirr_to_worst".to_string()];
        let result_json = price_instrument_with_context(
            &inst,
            &market,
            "2024-01-01",
            "tree",
            metrics,
            None,
            None,
        )
        .expect("xirr floor bond prices");
        let parsed: serde_json::Value = serde_json::from_str(&result_json).expect("parse");
        // Price > 0 and xirr metric present
        let amount = parsed["value"]["amount"]
            .as_f64()
            .or_else(|| {
                parsed["value"]["amount"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .expect("value.amount");
        assert!(amount > 0.0, "floored bond price must be positive");
        let xirr = parsed["measures"]["xirr"].as_f64().expect("xirr");
        assert!(xirr > 0.0, "xirr must be positive");
    }

    #[test]
    fn return_floor_metrics_in_standard_metrics_list() {
        // The four return-floor metric IDs must be present in the global registry.
        // Uses the underlying Rust pricer function directly (no JsValue).
        let ids = finstack_quant_valuations::pricer::list_standard_metrics();
        for metric in ["moic", "moic_to_worst", "xirr", "xirr_to_worst"] {
            assert!(
                ids.iter().any(|id| id == metric),
                "'{metric}' missing from standard metrics"
            );
        }
    }
}
