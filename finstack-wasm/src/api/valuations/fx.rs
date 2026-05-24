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

use crate::utils::to_js_err;
use finstack_valuations::pricer::{
    canonical_instrument_json, canonical_instrument_json_from_str,
    metric_value_from_instrument_json, present_standard_option_greeks_from_instrument_json,
    pretty_instrument_json, price_instrument_json,
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
    let market: finstack_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let result = price_instrument_json(json, &market, as_of, model.as_deref().unwrap_or("default"))
        .map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
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
    let market: finstack_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let metrics: Vec<String> = serde_wasm_bindgen::from_value(metrics).map_err(to_js_err)?;
    let result = finstack_valuations::pricer::price_instrument_json_with_metrics_and_history(
        json,
        &market,
        as_of,
        model.as_deref().unwrap_or("default"),
        &metrics,
        pricing_options.as_deref(),
        market_history.as_deref(),
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
}

fn metric_value(
    json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<String>,
    metric: &str,
) -> Result<f64, JsValue> {
    let market: finstack_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    metric_value_from_instrument_json(
        json,
        &market,
        as_of,
        model.as_deref().unwrap_or("default"),
        metric,
    )
    .map_err(to_js_err)
}

macro_rules! fx_class {
    ($rust_name:ident, $js_name:literal, $type_tag:literal) => {
        #[wasm_bindgen(js_name = $js_name)]
        pub struct $rust_name {
            json: String,
        }

        #[wasm_bindgen(js_class = $js_name)]
        impl $rust_name {
            #[wasm_bindgen(constructor)]
            pub fn new(spec: JsValue) -> Result<$rust_name, JsValue> {
                Ok(Self {
                    json: from_spec($type_tag, spec)?,
                })
            }

            #[wasm_bindgen(js_name = fromJson)]
            pub fn from_json(json: &str) -> Result<$rust_name, JsValue> {
                Ok(Self {
                    json: from_json_payload($type_tag, json)?,
                })
            }

            #[wasm_bindgen(js_name = toJson)]
            pub fn to_json(&self) -> Result<String, JsValue> {
                pretty_json(&self.json)
            }

            pub fn price(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<String, JsValue> {
                price_payload(&self.json, market_json, as_of, model)
            }

            #[wasm_bindgen(js_name = priceWithMetrics)]
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
            pub fn delta(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "delta")
            }

            pub fn gamma(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "gamma")
            }

            pub fn vega(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "vega")
            }

            pub fn theta(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "theta")
            }

            pub fn rho(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "rho")
            }

            #[wasm_bindgen(js_name = foreignRho)]
            pub fn foreign_rho(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "foreign_rho")
            }

            pub fn vanna(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "vanna")
            }

            pub fn volga(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<f64, JsValue> {
                metric_value(&self.json, market_json, as_of, model, "volga")
            }

            pub fn greeks(
                &self,
                market_json: &str,
                as_of: &str,
                model: Option<String>,
            ) -> Result<JsValue, JsValue> {
                let market: finstack_core::market_data::context::MarketContext =
                    serde_json::from_str(market_json).map_err(to_js_err)?;
                let pairs = present_standard_option_greeks_from_instrument_json(
                    &self.json,
                    &market,
                    as_of,
                    model.as_deref().unwrap_or("default"),
                )
                .map_err(to_js_err)?;
                let mut out = Map::new();
                for (metric, value) in pairs {
                    out.insert(metric.to_string(), Value::from(value));
                }
                serde_wasm_bindgen::to_value(&Value::Object(out)).map_err(to_js_err)
            }
        }
    };
}

fx_class!(WasmFxSpot, "FxSpot", "fx_spot");
fx_class!(WasmFxForward, "FxForward", "fx_forward");
fx_class!(WasmFxSwap, "FxSwap", "fx_swap");
fx_class!(WasmNdf, "Ndf", "ndf");
fx_option_class!(WasmFxOption, "FxOption", "fx_option");
fx_option_class!(WasmFxDigitalOption, "FxDigitalOption", "fx_digital_option");
fx_option_class!(WasmFxTouchOption, "FxTouchOption", "fx_touch_option");
fx_option_class!(WasmFxBarrierOption, "FxBarrierOption", "fx_barrier_option");
fx_class!(WasmFxVarianceSwap, "FxVarianceSwap", "fx_variance_swap");
fx_option_class!(WasmQuantoOption, "QuantoOption", "quanto_option");
