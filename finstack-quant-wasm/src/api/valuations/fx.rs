//! Direct WASM wrappers for FX valuation instruments.
//!
//! # Monte-Carlo determinism
//!
//! The `price` method accepts Monte-Carlo models for
//! path-dependent FX products (e.g. barrier / touch options). As with the
//! generic `priceInstrument` bindings, no explicit RNG-seed parameter is
//! exposed: the seed is an instrument-level concern. When an instrument's
//! `instrument_pricing_overrides.model_config.mc_seed_scenario` is `None`, the core MC pricers
//! derive a stable seed deterministically from the instrument ID, so repricing
//! the same instrument JSON is bit-reproducible. Callers needing a distinct
//! deterministic stream set that label inside the instrument JSON.

use super::pricing::{
    metric_value_with_context, parse_market_json, parse_pricing_instrument_json,
    standard_option_greeks_with_context,
};
use crate::utils::input::{from_js_json, js_opt_string, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::{InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::pricer::{instrument_from_spec, parse_typed_instrument_json};
use indexmap::IndexMap;
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// Build the concrete instrument from a bare JS spec object through the Rust
/// validating constructor.
fn from_spec<T>(type_tag: &str, spec: JsValue) -> Result<T, JsValue>
where
    T: TryFrom<InstrumentJson, Error = finstack_quant_core::Error>,
{
    let spec: Value = from_js_json(&spec, "spec")?;
    let instrument = instrument_from_spec(type_tag, spec).map_err(to_js_err)?;
    T::try_from(instrument).map_err(to_js_err)
}

/// Serialize a concrete instrument as its compact canonical v1 envelope.
fn envelope_json(instrument: impl Into<InstrumentJson>) -> Result<String, JsValue> {
    serde_json::to_string(&InstrumentEnvelope::new(instrument.into())).map_err(to_js_err)
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

/// Shared body for the `greeks` method emitted by the FX-option macro.
///
/// Prices the Rust-ordered subset of `STANDARD_OPTION_GREEKS` that applies to
/// the instrument and returns a JS object whose keys keep that order.
/// Non-finite Greeks are rejected by Rust rather than serialized.
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
    let ordered: IndexMap<&'static str, f64> = pairs.into_iter().collect();
    to_js_value(&ordered)
}

macro_rules! fx_class {
    ($rust_name:ident, $js_name:literal, $type_tag:literal, $rust_ty:ty) => {
        #[doc = concat!("Typed WASM wrapper for the Rust FX instrument `", $js_name, "`.")]
        #[wasm_bindgen(js_name = $js_name)]
        #[derive(Clone)]
        pub struct $rust_name {
            pub(crate) inner: $rust_ty,
        }

        impl $rust_name {
            /// The instrument as its compact canonical v1 envelope.
            fn envelope_json(&self) -> Result<String, JsValue> {
                envelope_json(self.inner.clone())
            }
        }

        #[wasm_bindgen(js_class = $js_name)]
        impl $rust_name {
            /// Create the instrument from a JS spec object.
            /// @param spec - Bare JavaScript spec object for this exact instrument type.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if `spec` cannot be converted from
            /// JavaScript, is not a bare object for this FX instrument type, or
            /// fails instrument validation.
            #[wasm_bindgen(constructor)]
            pub fn new(spec: JsValue) -> Result<$rust_name, JsValue> {
                Ok(Self {
                    inner: from_spec($type_tag, spec)?,
                })
            }

            /// Deserialize the instrument from its canonical v1 envelope.
            /// @param json - A `finstack_quant.instrument/1` envelope for this exact instrument type.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if `json` is malformed, is not a
            /// canonical envelope for this exact FX instrument type, or fails
            /// instrument validation.
            #[wasm_bindgen(js_name = fromJson)]
            pub fn from_json(json: JsValue) -> Result<$rust_name, JsValue> {
                let json: &str = &json_text(&json, "json")?;
                Ok(Self {
                    inner: parse_typed_instrument_json::<$rust_ty>(json).map_err(to_js_err)?,
                })
            }

            /// Serialize to the canonical `finstack_quant.instrument/1` envelope.
            ///
            /// The output is compact JSON, byte-identical to the Python
            /// `to_json()` of the same instrument.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument cannot be serialized.
            #[wasm_bindgen(js_name = toJson)]
            pub fn to_json(&self) -> Result<String, JsValue> {
                self.envelope_json()
            }

            /// Instrument identifier (mirrors the Python wrappers' `id` property).
            #[wasm_bindgen(getter)]
            pub fn id(&self) -> String {
                self.inner.id.to_string()
            }

            /// Price the instrument against a market JSON snapshot.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @param metrics - Optional canonical metric IDs such as `"delta"`,
            /// `"vega"`, `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or
            /// `undefined` for a valuation-only result.
            /// @param metric_pricing_overrides - Optional JSON metric-pricing overrides
            /// merged into the envelope before validation. Omit, `null`, or
            /// `undefined` to use the envelope as-is.
            /// @param market_history - Optional serialized market-history JSON
            /// required by historical risk metrics such as historical VaR.
            /// @returns Structured `ValuationResult` for the selected model.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if an instrument, market,
            /// metric-pricing-override, or market-history payload is invalid; `metrics` is not a
            /// string array; `asOf`, `model`, or a metric identifier is invalid;
            /// required market data is missing; pricing or a metric fails; or the
            /// valuation cannot be converted to JavaScript.
            pub fn price(
                &self,
                market_json: JsValue,
                as_of: JsValue,
                model: Option<JsValue>,
                metrics: Option<JsValue>,
                metric_pricing_overrides: Option<JsValue>,
                market_history: Option<JsValue>,
            ) -> Result<JsValue, JsValue> {
                super::pricing::PriceRequest::from_js(
                    &market_json,
                    &as_of,
                    model.as_ref(),
                    metrics.as_ref(),
                    metric_pricing_overrides.as_ref(),
                    market_history.as_ref(),
                )?
                .price(&self.envelope_json()?)
            }
        }
    };
}

/// Emit an FX option class: the `fx_class!` surface plus one method per
/// applicable Greek and `greeks()`.
///
/// Each Greek entry is `(method, "jsName", "metric_id", "summary doc", "@returns doc")`. The
/// method list is pinned against the Rust metric registry by
/// `tests::option_greek_methods_match_the_metric_registry`.
macro_rules! fx_option_class {
    (
        $rust_name:ident, $js_name:literal, $type_tag:literal, $rust_ty:ty, $instrument_type:ident,
        [$(($method:ident, $js_method:literal, $metric:literal, $summary:literal, $returns:literal)),+ $(,)?]
    ) => {
        fx_class!($rust_name, $js_name, $type_tag, $rust_ty);

        impl $rust_name {
            /// Metric IDs of the Greek methods this class exposes, in
            /// `STANDARD_OPTION_GREEKS` order.
            #[cfg(test)]
            const GREEK_METHODS: &'static [&'static str] = &[$($metric),+];
            /// Rust instrument type whose registry decides which Greeks apply.
            #[cfg(test)]
            const INSTRUMENT_TYPE: finstack_quant_valuations::pricer::InstrumentType =
                finstack_quant_valuations::pricer::InstrumentType::$instrument_type;
        }

        #[wasm_bindgen(js_class = $js_name)]
        impl $rust_name {
            $(
                #[doc = $summary]
                /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
                /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
                /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
                #[doc = $returns]
                ///
                /// # Errors
                ///
                /// Throws a JavaScript exception if the instrument or market JSON,
                /// `asOf`, or `model` is invalid; required market data is missing;
                /// pricing fails; or the Greek is not produced by the selected model.
                #[wasm_bindgen(js_name = $js_method)]
                pub fn $method(
                    &self,
                    market_json: JsValue,
                    as_of: JsValue,
                    model: Option<JsValue>,
                ) -> Result<f64, JsValue> {
                    let market_json: &str = &json_text(&market_json, "marketJson")?;
                    let as_of: &str = &js_string(&as_of, "asOf")?;
                    let model = js_opt_string(model.as_ref(), "model")?;
                    metric_value(&self.envelope_json()?, market_json, as_of, model, $metric)
                }
            )+

            /// Compute every Greek that applies to this instrument as a JavaScript object.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Map of Greek name to value, with keys in the Rust
            /// `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`, …).
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; a returned Greek is non-finite; or the result cannot
            /// be converted to a JavaScript value.
            pub fn greeks(
                &self,
                market_json: JsValue,
                as_of: JsValue,
                model: Option<JsValue>,
            ) -> Result<JsValue, JsValue> {
                let market_json: &str = &json_text(&market_json, "marketJson")?;
                let as_of: &str = &js_string(&as_of, "asOf")?;
                let model = js_opt_string(model.as_ref(), "model")?;
                option_greeks_object(&self.envelope_json()?, market_json, as_of, model.as_deref())
            }
        }
    };
}

use finstack_quant_valuations::instruments as fxi;

fx_class!(JsFxSpot, "FxSpot", "fx_spot", fxi::FxSpot);
fx_class!(JsFxForward, "FxForward", "fx_forward", fxi::FxForward);
fx_class!(JsFxSwap, "FxSwap", "fx_swap", fxi::FxSwap);
fx_class!(JsNdf, "Ndf", "ndf", fxi::Ndf);
fx_class!(
    JsFxVarianceSwap,
    "FxVarianceSwap",
    "fx_variance_swap",
    fxi::FxVarianceSwap
);

macro_rules! fx_option_with_all_greeks {
    ($rust_name:ident, $js_name:literal, $type_tag:literal, $rust_ty:ty, $instrument_type:ident) => {
        fx_option_class!(
            $rust_name, $js_name, $type_tag, $rust_ty, $instrument_type,
            [
                (delta, "delta", "delta", "Spot delta of the option.", "@returns Spot delta: change in value per unit spot."),
                (gamma, "gamma", "gamma", "Spot gamma of the option.", "@returns Spot gamma: change in delta per unit spot."),
                (vega, "vega", "vega", "Vega of the option.", "@returns Vega: change in value per 1.0 absolute move in implied volatility."),
                (theta, "theta", "theta", "Theta of the option.", "@returns Theta: change in value per year of calendar time."),
                (rho, "rho", "rho", "Domestic rate rho of the option.", "@returns Domestic rho: change in value per 1.0 absolute move in the domestic rate."),
                (foreign_rho, "foreignRho", "foreign_rho", "Foreign rate rho of the option.", "@returns Foreign rho: change in value per 1.0 absolute move in the foreign rate."),
                (vanna, "vanna", "vanna", "Vanna of the option.", "@returns Vanna: cross sensitivity of delta to implied volatility."),
                (volga, "volga", "volga", "Volga of the option.", "@returns Volga: change in vega per 1.0 absolute move in implied volatility."),
            ]
        );
    };
}

fx_option_with_all_greeks!(JsFxOption, "FxOption", "fx_option", fxi::FxOption, FxOption);
fx_option_with_all_greeks!(
    JsQuantoOption,
    "QuantoOption",
    "quanto_option",
    fxi::QuantoOption,
    QuantoOption
);
fx_option_class!(
    JsFxDigitalOption,
    "FxDigitalOption",
    "fx_digital_option",
    fxi::FxDigitalOption,
    FxDigitalOption,
    [
        (
            delta,
            "delta",
            "delta",
            "Spot delta of the option.",
            "@returns Spot delta: change in value per unit spot."
        ),
        (
            gamma,
            "gamma",
            "gamma",
            "Spot gamma of the option.",
            "@returns Spot gamma: change in delta per unit spot."
        ),
        (
            vega,
            "vega",
            "vega",
            "Vega of the option.",
            "@returns Vega: change in value per 1.0 absolute move in implied volatility."
        ),
        (
            theta,
            "theta",
            "theta",
            "Theta of the option.",
            "@returns Theta: change in value per year of calendar time."
        ),
        (
            rho,
            "rho",
            "rho",
            "Domestic rate rho of the option.",
            "@returns Domestic rho: change in value per 1.0 absolute move in the domestic rate."
        ),
    ]
);
fx_option_class!(
    JsFxTouchOption,
    "FxTouchOption",
    "fx_touch_option",
    fxi::FxTouchOption,
    FxTouchOption,
    [
        (
            delta,
            "delta",
            "delta",
            "Spot delta of the option.",
            "@returns Spot delta: change in value per unit spot."
        ),
        (
            gamma,
            "gamma",
            "gamma",
            "Spot gamma of the option.",
            "@returns Spot gamma: change in delta per unit spot."
        ),
        (
            vega,
            "vega",
            "vega",
            "Vega of the option.",
            "@returns Vega: change in value per 1.0 absolute move in implied volatility."
        ),
        (
            theta,
            "theta",
            "theta",
            "Theta of the option.",
            "@returns Theta: P&L over the theta horizon (default one day, capped at expiry) with spot held at its `asOf` level, so a roll into the monitoring window observes that spot; not annualized."
        ),
        (
            rho,
            "rho",
            "rho",
            "Domestic rate rho of the option.",
            "@returns Domestic rho: change in value per 1.0 absolute move in the domestic rate."
        ),
    ]
);
fx_option_class!(
    JsFxBarrierOption,
    "FxBarrierOption",
    "fx_barrier_option",
    fxi::FxBarrierOption,
    FxBarrierOption,
    [
        (
            delta,
            "delta",
            "delta",
            "Spot delta of the option.",
            "@returns Spot delta: change in value per unit spot."
        ),
        (
            gamma,
            "gamma",
            "gamma",
            "Spot gamma of the option.",
            "@returns Spot gamma: change in delta per unit spot."
        ),
        (
            vega,
            "vega",
            "vega",
            "Vega of the option.",
            "@returns Vega: change in value per 1.0 absolute move in implied volatility."
        ),
        (
            theta,
            "theta",
            "theta",
            "Theta of the option.",
            "@returns Theta: P&L over the theta horizon (default one day, capped at expiry) with spot held at its `asOf` level, so a roll into the monitoring window observes that spot; not annualized."
        ),
        (
            rho,
            "rho",
            "rho",
            "Domestic rate rho of the option.",
            "@returns Domestic rho: change in value per 1.0 absolute move in the domestic rate."
        ),
        (
            vanna,
            "vanna",
            "vanna",
            "Vanna of the option.",
            "@returns Vanna: cross sensitivity of delta to implied volatility."
        ),
        (
            volga,
            "volga",
            "volga",
            "Volga of the option.",
            "@returns Volga: change in vega per 1.0 absolute move in implied volatility."
        ),
    ]
);

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations::metrics::{standard_registry, MetricId};
    use finstack_quant_valuations::pricer::{InstrumentType, STANDARD_OPTION_GREEKS};

    #[test]
    fn public_json_routes_validate_instrument_before_market_json() {
        assert!(super::super::pricing::tests::not_a_market_request()
            .price("{}")
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

    /// The Greeks the registry computes for `instrument_type`, in
    /// `STANDARD_OPTION_GREEKS` order.
    fn registry_greeks(instrument_type: InstrumentType) -> Vec<String> {
        let menu: Vec<MetricId> = STANDARD_OPTION_GREEKS
            .iter()
            .map(|metric| MetricId::parse_strict(metric).expect("standard greek"))
            .collect();
        standard_registry()
            .applicable_subset(&menu, instrument_type)
            .iter()
            .map(|id| id.as_str().to_string())
            .collect()
    }

    #[test]
    fn option_greek_methods_match_the_metric_registry() {
        let classes: [(&[&str], InstrumentType); 5] = [
            (JsFxOption::GREEK_METHODS, JsFxOption::INSTRUMENT_TYPE),
            (
                JsQuantoOption::GREEK_METHODS,
                JsQuantoOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxDigitalOption::GREEK_METHODS,
                JsFxDigitalOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxTouchOption::GREEK_METHODS,
                JsFxTouchOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxBarrierOption::GREEK_METHODS,
                JsFxBarrierOption::INSTRUMENT_TYPE,
            ),
        ];
        for (methods, instrument_type) in classes {
            assert_eq!(
                methods.to_vec(),
                registry_greeks(instrument_type),
                "{instrument_type:?} Greek methods drifted from the metric registry"
            );
        }
    }
}
