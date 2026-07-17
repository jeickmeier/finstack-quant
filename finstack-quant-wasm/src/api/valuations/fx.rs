//! Direct WASM wrappers for FX valuation instruments.
//!
//! # Monte-Carlo determinism
//!
//! The `price` / `priceWithMetrics` methods accept Monte-Carlo models for
//! path-dependent FX products (e.g. barrier / touch options). As with the
//! generic `priceInstrument` bindings, no explicit RNG-seed parameter is
//! exposed: the seed is an instrument-level concern. When an instrument's
//! `pricing_overrides.metrics.mc_seed_scenario` is `None`, the core MC pricers
//! derive a stable seed deterministically from the instrument ID, so repricing
//! the same instrument JSON is bit-reproducible. Callers needing a distinct
//! deterministic stream set `mc_seed_scenario` inside the instrument JSON.

use super::pricing::{
    metric_value_with_context, parse_market_json, price_instrument_with_context,
    price_instrument_with_metrics_context, standard_option_greeks_with_context,
    validate_pricing_instrument_json,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::pricer::{
    canonical_instrument_json, canonical_instrument_json_from_str, pretty_instrument_json,
};
use serde_json::{Map, Value};
use wasm_bindgen::prelude::*;

fn value_from_spec(spec: JsValue) -> Result<Value, JsValue> {
    if let Some(json) = spec.as_string() {
        serde_json::from_str(&json).map_err(to_js_err)
    } else {
        serde_wasm_bindgen::from_value(spec).map_err(to_js_err)
    }
}

fn from_spec(type_tag: &str, spec: JsValue) -> Result<String, JsValue> {
    canonical_instrument_json(type_tag, value_from_spec(spec)?).map_err(to_js_err)
}

fn from_json_payload(type_tag: &str, json: &str) -> Result<String, JsValue> {
    canonical_instrument_json_from_str(type_tag, json).map_err(to_js_err)
}

fn pretty_json(json: &str) -> Result<String, JsValue> {
    pretty_instrument_json(json).map_err(to_js_err)
}

fn price_payload(
    json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<String>,
) -> Result<String, JsValue> {
    validate_pricing_instrument_json(json, None)?;
    let market = parse_market_json(market_json)?;
    price_instrument_with_context(json, &market, as_of, model.as_deref().unwrap_or("default"))
}

fn price_payload_with_metrics(
    json: &str,
    market_json: &str,
    as_of: &str,
    metrics: JsValue,
    model: Option<String>,
    pricing_options: Option<String>,
    market_history: Option<String>,
) -> Result<String, JsValue> {
    validate_pricing_instrument_json(json, pricing_options.as_deref())?;
    let market = parse_market_json(market_json)?;
    let metrics: Vec<String> = serde_wasm_bindgen::from_value(metrics).map_err(to_js_err)?;
    price_instrument_with_metrics_context(
        json,
        &market,
        as_of,
        model.as_deref().unwrap_or("default"),
        metrics,
        pricing_options.as_deref(),
        market_history.as_deref(),
    )
}

fn metric_value(
    json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<String>,
    metric: &str,
) -> Result<f64, JsValue> {
    validate_pricing_instrument_json(json, None)?;
    let market = parse_market_json(market_json)?;
    metric_value_with_context(
        json,
        &market,
        as_of,
        model.as_deref().unwrap_or("default"),
        metric,
    )
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
            #[wasm_bindgen(constructor)]
            /// Create the instrument from a JS spec object.
            /// @param spec - JavaScript object or JSON payload defining the canonical instrument or calculation specification.
            pub fn new(spec: JsValue) -> Result<$rust_name, JsValue> {
                Ok(Self {
                    json: from_spec($type_tag, spec)?,
                })
            }

            #[wasm_bindgen(js_name = fromJson)]
            /// Deserialize the instrument from a JSON spec string.
            /// @param json - Canonical JSON string defining the object to deserialize or normalize.
            pub fn from_json(json: &str) -> Result<$rust_name, JsValue> {
                Ok(Self {
                    json: from_json_payload($type_tag, json)?,
                })
            }

            #[wasm_bindgen(js_name = toJson)]
            /// Serialize the instrument spec to pretty JSON.
            pub fn to_json(&self) -> Result<String, JsValue> {
                pretty_json(&self.json)
            }

            /// Price the instrument against a market JSON snapshot.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            pub fn price(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<String, JsValue> {
                price_payload(&self.json, market_json, as_of, model)
            }

            #[wasm_bindgen(js_name = priceWithMetrics)]
            /// Price the instrument and compute the requested metrics.
            ///
            /// WASM keeps optional arguments trailing for JavaScript callers,
            /// so the order is `(marketJson, asOf, metrics, model?, ...)`.
            /// Python uses `(market, as_of, model="default", metrics=...)`.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param metrics - Array of canonical metric identifiers to calculate with the instrument price.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @param pricing_options - Optional JSON pricing overrides accepted by the canonical instrument validator.
            /// @param market_history - Optional serialized historical market snapshots required by historical pricing models.
            pub fn price_with_metrics(
                &self,
                market_json: &str,
                as_of: &str,
                metrics: JsValue,
                model: Option<String>,
                pricing_options: Option<String>,
                market_history: Option<String>,
            ) -> Result<String, JsValue> {
                price_payload_with_metrics(
                    &self.json,
                    market_json,
                    as_of,
                    metrics,
                    model,
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
            pub fn rho(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "rho")
            }

            #[wasm_bindgen(js_name = foreignRho)]
            /// Foreign rate rho of the option.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
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
            pub fn greeks(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<JsValue, JsValue> {
                validate_pricing_instrument_json(&self.json, None)?;
                let market = parse_market_json(market_json)?;
                let pairs = standard_option_greeks_with_context(
                    &self.json,
                    &market,
                    as_of,
                    model.as_deref().unwrap_or("default"),
                )?;
                let mut out = Map::new();
                for (metric, value) in pairs {
                    out.insert(metric.to_string(), Value::from(value));
                }
                to_js_value(&Value::Object(out))
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
            pub fn greeks(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<JsValue, JsValue> {
                validate_pricing_instrument_json(&self.json, None)?;
                let market = parse_market_json(market_json)?;
                let pairs = standard_option_greeks_with_context(
                    &self.json,
                    &market,
                    as_of,
                    model.as_deref().unwrap_or("default"),
                )?;
                let mut out = Map::new();
                for (metric, value) in pairs {
                    out.insert(metric.to_string(), Value::from(value));
                }
                to_js_value(&Value::Object(out))
            }
        }
    };
}

fx_class!(WasmFxSpot, "FxSpot", "fx_spot");
fx_class!(WasmFxForward, "FxForward", "fx_forward");
fx_class!(WasmFxSwap, "FxSwap", "fx_swap");
fx_class!(WasmNdf, "Ndf", "ndf");
fx_option_class!(WasmFxOption, "FxOption", "fx_option");
fx_option_subset_class!(
    WasmFxDigitalOption,
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
    WasmFxTouchOption,
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
    WasmFxBarrierOption,
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
fx_class!(WasmFxVarianceSwap, "FxVarianceSwap", "fx_variance_swap");
fx_option_class!(WasmQuantoOption, "QuantoOption", "quanto_option");
