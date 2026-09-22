//! Direct WASM wrappers for FX valuation instruments.
//!
//! # Monte-Carlo determinism
//!
//! The `price` method accepts Monte-Carlo models for
//! path-dependent FX products (e.g. barrier / touch options). As with the
//! generic `priceInstrument` bindings, no explicit RNG-seed parameter is
//! exposed: the seed is an instrument-level concern. When an instrument's
//! `pricing_overrides.metrics.mc_seed_scenario` is `None`, the core MC pricers
//! derive a stable seed deterministically from the instrument ID, so repricing
//! the same instrument JSON is bit-reproducible. Callers needing a distinct
//! deterministic stream set `mc_seed_scenario` inside the instrument JSON.

use super::pricing::{
    metric_value_with_context, parse_market_json, parse_pricing_instrument_json,
    price_result_with_context, standard_option_greeks_with_context,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::pricer::{
    instrument_envelope_from_spec, pretty_instrument_json, validate_typed_instrument_json,
};
use serde_json::{Map, Value};
use wasm_bindgen::prelude::*;

fn value_from_spec(spec: JsValue) -> Result<Value, JsValue> {
    serde_wasm_bindgen::from_value(spec).map_err(to_js_err)
}

fn from_spec(type_tag: &str, spec: JsValue) -> Result<String, JsValue> {
    instrument_envelope_from_spec(type_tag, value_from_spec(spec)?).map_err(to_js_err)
}

fn from_json_payload(type_tag: &str, json: &str) -> Result<String, JsValue> {
    validate_typed_instrument_json(type_tag, json).map_err(to_js_err)
}

fn pretty_json(json: &str) -> Result<String, JsValue> {
    pretty_instrument_json(json).map_err(to_js_err)
}

/// Shared body for the `id` getter emitted by the FX-class macro.
///
/// Re-parses the stored (already validated) canonical envelope and reads the
/// instrument identifier through the canonical `Instrument` trait, matching
/// the Python typed wrappers' `id` property.
fn instrument_id_from_json(json: &str) -> Result<String, JsValue> {
    finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(json, None)
        .map(|instrument| instrument.as_instrument().id().to_string())
        .map_err(to_js_err)
}

fn price_payload(
    json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<String>,
    metrics: Option<JsValue>,
    pricing_options: Option<String>,
    market_history: Option<String>,
) -> Result<JsValue, JsValue> {
    let instrument = parse_pricing_instrument_json(json, pricing_options.as_deref())?;
    let market = parse_market_json(market_json)?;
    let metrics: Vec<String> = match metrics {
        None => Vec::new(),
        Some(value) if value.is_undefined() || value.is_null() => Vec::new(),
        Some(value) => serde_wasm_bindgen::from_value(value).map_err(to_js_err)?,
    };
    let result = price_result_with_context(
        &instrument,
        &market,
        as_of,
        model.as_deref().unwrap_or("default"),
        metrics,
        market_history.as_deref(),
    )?;
    to_js_value(&result)
}

fn metric_value(
    json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<String>,
    metric: &str,
) -> Result<f64, JsValue> {
    let instrument = parse_pricing_instrument_json(json, None)?;
    let market = parse_market_json(market_json)?;
    metric_value_with_context(
        &instrument,
        &market,
        as_of,
        model.as_deref().unwrap_or("default"),
        metric,
    )
}

/// Shared body for the `greeks` method emitted by both FX-option macros.
///
/// Prices the standard Greek set with market context and returns a JS object.
/// Non-finite Greeks are rejected rather than serialized: `serde_json` maps
/// them to `null`, which would silently look like "not computed".
fn option_greeks_object(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<&str>,
) -> Result<JsValue, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, None)?;
    let market = parse_market_json(market_json)?;
    let pairs = standard_option_greeks_with_context(
        &instrument,
        &market,
        as_of,
        model.unwrap_or("default"),
    )?;
    let mut out = Map::new();
    for (metric, value) in pairs {
        out.insert(metric.to_string(), Value::from(value));
    }
    to_js_value(&Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_json_routes_validate_instrument_before_market_json() {
        assert!(price_payload(
            "{}",
            "not-market-json",
            "not-a-date",
            Some("not-a-model".to_string()),
            None,
            None,
            None,
        )
        .is_err());
        assert!(metric_value(
            "{}",
            "not-market-json",
            "not-a-date",
            Some("not-a-model".to_string()),
            "not-a-metric",
        )
        .is_err());
        assert!(
            option_greeks_object("{}", "not-market-json", "not-a-date", Some("not-a-model"),)
                .is_err()
        );
    }
}

macro_rules! fx_class {
    ($rust_name:ident, $js_name:literal, $type_tag:literal) => {
        #[doc = concat!("FX instrument `", $js_name, "`: holds a validated JSON spec.")]
        #[wasm_bindgen(js_name = $js_name)]
        pub struct $rust_name {
            json: String,
        }

        #[wasm_bindgen(js_class = $js_name)]
        impl $rust_name {
            /// Create the instrument from a JS spec object.
            /// @param spec - Bare JavaScript spec object for this exact instrument type.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if `spec` cannot be converted from
            /// JavaScript, is not a bare object for this FX instrument type, fails
            /// instrument validation, or cannot be serialized as a canonical envelope.
            #[wasm_bindgen(constructor)]
            pub fn new(spec: JsValue) -> Result<$rust_name, JsValue> {
                Ok(Self {
                    json: from_spec($type_tag, spec)?,
                })
            }

            /// Deserialize the instrument from its canonical v1 envelope.
            /// @param json - A `finstack_quant.instrument/1` envelope for this exact instrument type.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if `json` is malformed, is not a
            /// canonical envelope for this exact FX instrument type, fails
            /// instrument validation, or cannot be canonically serialized.
            #[wasm_bindgen(js_name = fromJson)]
            pub fn from_json(json: &str) -> Result<$rust_name, JsValue> {
                Ok(Self {
                    json: from_json_payload($type_tag, json)?,
                })
            }

            /// Serialize the instrument spec to pretty JSON.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the stored canonical instrument
            /// envelope cannot be parsed or rendered as pretty JSON.
            #[wasm_bindgen(js_name = toJson)]
            pub fn to_json(&self) -> Result<String, JsValue> {
                pretty_json(&self.json)
            }

            /// Instrument identifier (mirrors the Python wrappers' `id` property).
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the stored canonical instrument
            /// envelope cannot be re-parsed (should not happen for a validated
            /// instance).
            #[wasm_bindgen(getter)]
            pub fn id(&self) -> Result<String, JsValue> {
                instrument_id_from_json(&self.json)
            }

            /// Price the instrument against a market JSON snapshot.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @param metrics - Optional canonical metric IDs such as `"delta"`,
            /// `"vega"`, `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or
            /// `undefined` for a valuation-only result.
            /// @param pricing_options - Optional JSON metric-pricing overrides
            /// merged into the envelope before validation. Omit, `null`, or
            /// `undefined` to use the envelope as-is.
            /// @param market_history - Optional serialized market-history JSON
            /// required by historical risk metrics such as historical VaR.
            /// @returns Structured `ValuationResult` for the selected model.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if an instrument, market, pricing-
            /// option, or market-history payload is invalid; `metrics` is not a
            /// string array; `asOf`, `model`, or a metric identifier is invalid;
            /// required market data is missing; pricing or a metric fails; or the
            /// valuation cannot be converted to JavaScript.
            pub fn price(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
                metrics: Option<JsValue>,
                pricing_options: Option<String>,
                market_history: Option<String>,
            ) -> Result<JsValue, JsValue> {
                price_payload(
                    &self.json,
                    market_json,
                    as_of,
                    model,
                    metrics,
                    pricing_options,
                    market_history,
                )
            }
        }
    };
}

macro_rules! fx_option_class {
    ($rust_name:ident, $js_name:literal, $type_tag:literal) => {
        fx_class!($rust_name, $js_name, $type_tag);

        #[wasm_bindgen(js_class = $js_name)]
        impl $rust_name {
            /// Spot delta of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Spot delta: change in value per unit spot.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; or delta is not produced by the selected model.
            pub fn delta(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "delta")
            }

            /// Spot gamma of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Spot gamma: change in delta per unit spot.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; or gamma is not produced by the selected model.
            pub fn gamma(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "gamma")
            }

            /// Vega of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Vega: change in value per 1.0 absolute move in implied volatility.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; or vega is not produced by the selected model.
            pub fn vega(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "vega")
            }

            /// Theta of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Theta: change in value per year of calendar time.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; or theta is not produced by the selected model.
            pub fn theta(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "theta")
            }

            /// Domestic rate rho of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Domestic rho: change in value per 1.0 absolute move in the domestic rate.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; or domestic rho is not produced by the selected model.
            pub fn rho(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "rho")
            }

            /// Foreign rate rho of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Foreign rho: change in value per 1.0 absolute move in the foreign rate.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; or foreign rho is not produced by the selected model.
            #[wasm_bindgen(js_name = foreignRho)]
            pub fn foreign_rho(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "foreign_rho")
            }

            /// Vanna of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Vanna: cross sensitivity of delta to implied volatility.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; or vanna is not produced by the selected model.
            pub fn vanna(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "vanna")
            }

            /// Volga of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Volga: change in vega per 1.0 absolute move in implied volatility.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; or volga is not produced by the selected model.
            pub fn volga(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "volga")
            }

            /// Compute standard FX option Greeks as a JavaScript object.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Map of greek name to value, such as `delta`, `gamma`, and `vega`.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; a returned Greek is non-finite; or the result cannot
            /// be converted to a JavaScript value.
            pub fn greeks(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<JsValue, JsValue> {
                option_greeks_object(&self.json, market_json, as_of, model.as_deref())
            }
        }
    };
}

macro_rules! fx_option_subset_class {
    ($rust_name:ident, $js_name:literal, $type_tag:literal, [$(($method:ident, $metric:literal)),+ $(,)?]) => {
        fx_class!($rust_name, $js_name, $type_tag);

        #[wasm_bindgen(js_class = $js_name)]
        impl $rust_name {
            $(
                /// Compute this supported option sensitivity.
                pub fn $method(
                    &self,
                    market_json: &str,
                    as_of: &str,
                    model: Option<String>,
                ) -> Result<f64, JsValue> {
                    metric_value(&self.json, market_json, as_of, model, $metric)
                }
            )+

            /// Compute all Greeks supported by this instrument as a JavaScript object.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; a returned Greek is non-finite; or the result cannot
            /// be converted to a JavaScript value.
            pub fn greeks(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<JsValue, JsValue> {
                option_greeks_object(&self.json, market_json, as_of, model.as_deref())
            }
        }
    };
}

fx_class!(JsFxSpot, "FxSpot", "fx_spot");
fx_class!(JsFxForward, "FxForward", "fx_forward");
fx_class!(JsFxSwap, "FxSwap", "fx_swap");
fx_class!(JsNdf, "Ndf", "ndf");
fx_option_class!(JsFxOption, "FxOption", "fx_option");
fx_option_subset_class!(
    JsFxDigitalOption,
    "FxDigitalOption",
    "fx_digital_option",
    [
        (delta, "delta"),
        (gamma, "gamma"),
        (vega, "vega"),
        (theta, "theta"),
        (rho, "rho"),
    ]
);
fx_option_subset_class!(
    JsFxTouchOption,
    "FxTouchOption",
    "fx_touch_option",
    [
        (delta, "delta"),
        (gamma, "gamma"),
        (vega, "vega"),
        (rho, "rho"),
    ]
);
fx_option_subset_class!(
    JsFxBarrierOption,
    "FxBarrierOption",
    "fx_barrier_option",
    [
        (delta, "delta"),
        (gamma, "gamma"),
        (vega, "vega"),
        (rho, "rho"),
        (vanna, "vanna"),
        (volga, "volga"),
    ]
);
fx_class!(JsFxVarianceSwap, "FxVarianceSwap", "fx_variance_swap");
fx_option_class!(JsQuantoOption, "QuantoOption", "quanto_option");
