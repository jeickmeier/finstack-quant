//! Macros that emit the typed-instrument members.
//!
//! Each macro expands to `#[wasm_bindgen]` items whose bodies are one
//! conversion and one Rust call. The documentation for every member is
//! written at the invocation and forwarded with `#[doc]`, so wasm-bindgen
//! copies it into the generated TypeScript declarations.

/// A typed instrument class: the wrapper struct, `fromJson`, `toJson` and `id`.
macro_rules! instrument_class {
    ($(#[$doc:meta])* $ty:ident, $js:literal, $rust:ty) => {
        $(#[$doc])*
        #[wasm_bindgen(js_name = $js)]
        #[derive(Clone)]
        pub struct $ty {
            pub(crate) inner: $rust,
        }

        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            /// Deserialize the instrument from its canonical v1 envelope.
            ///
            /// Bare payloads are rejected; the loader's validation runs on the result.
            /// @param json - A `finstack_quant.instrument/1` envelope (JSON string or plain object) for this exact instrument type.
            /// @returns The validated instrument.
            /// @throws Error - Throws with kind `validation` if `json` is malformed, carries a different instrument type, or fails instrument validation.
            #[wasm_bindgen(js_name = fromJson)]
            pub fn from_json(json: JsValue) -> Result<$ty, JsValue> {
                let json = crate::utils::input::json_text(&json, "json")?;
                finstack_quant_valuations::pricer::parse_typed_instrument_json::<$rust>(&json)
                    .map(|inner| Self { inner })
                    .map_err(crate::utils::to_js_err)
            }

            /// Serialize to the canonical `finstack_quant.instrument/1` envelope.
            ///
            /// The output is compact JSON, byte-identical to the Python
            /// `to_json()` of the same instrument; pass it to
            /// `valuations.instruments.priceInstrument` or `fromJson`.
            /// @returns Canonical instrument envelope JSON.
            /// @throws Error - Throws if the instrument cannot be serialized.
            #[wasm_bindgen(js_name = toJson)]
            pub fn to_json(&self) -> Result<String, JsValue> {
                $crate::api::valuations::typed::envelope_json(self.inner.clone())
            }

            /// Instrument identifier.
            /// @returns Stable instrument identifier used in metric keys.
            #[wasm_bindgen(getter)]
            pub fn id(&self) -> String {
                self.inner.id.to_string()
            }
        }
    };
}

/// `toDict` for a typed instrument class.
macro_rules! instrument_to_dict {
    ($ty:ident, $js:literal) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            /// The bare instrument spec as a plain object (no envelope).
            ///
            /// Mirrors Python `to_dict()`: the serde form of the Rust struct,
            /// i.e. the `instrument.spec` payload of `toJson()`.
            /// @returns Plain-object instrument spec.
            /// @throws Error - Throws if the instrument cannot be serialized.
            #[wasm_bindgen(js_name = toDict)]
            pub fn to_dict(&self) -> Result<JsValue, JsValue> {
                crate::utils::to_js_value(&self.inner)
            }
        }
    };
}

/// `marketDependencies` for a typed instrument class.
macro_rules! instrument_market_dependencies {
    ($ty:ident, $js:literal) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            /// Market data this instrument needs to price (mirrors Rust `Instrument::market_dependencies`).
            /// @returns `MarketDependencies` plain object listing discount, forward and credit curves, spot ids, volatility surfaces, FX pairs and fixing series.
            /// @throws Error - Throws with kind `validation` if the instrument cannot enumerate its dependencies.
            #[wasm_bindgen(js_name = marketDependencies)]
            pub fn market_dependencies(&self) -> Result<JsValue, JsValue> {
                $crate::api::valuations::typed::market_dependencies(&self.inner)
            }
        }
    };
}

/// The static `builder()` entry point of a typed instrument class.
macro_rules! instrument_builder_entry {
    ($ty:ident, $js:literal, $builder:ident) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            /// Create a fluent builder (mirrors the Rust `builder()`).
            ///
            /// Each setter stores one field and returns the builder; `build()`
            /// validates and consumes it.
            /// @returns An empty builder.
            pub fn builder() -> $builder {
                $builder::new()
            }
        }
    };
}

/// `price` and `metric` for a typed instrument class.
macro_rules! instrument_pricing {
    ($ty:ident, $js:literal) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            /// Price the instrument against a market snapshot.
            ///
            /// Same pipeline and arguments as `valuations.instruments.priceInstrument`.
            /// @param market_json - Canonical market-context JSON (string or plain object) supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @param metrics - Optional canonical metric IDs such as `"dv01"` or `"cs01"`. Omit, `null`, or `undefined` for a valuation-only result.
            /// @param metric_pricing_overrides - Optional `MetricPricingOverrides` (JSON string or plain object) merged into the envelope before validation.
            /// @param market_history - Optional `MarketHistory` (JSON string or plain object) required by historical risk metrics such as historical VaR.
            /// @returns Structured `ValuationResult` for the selected model.
            /// @throws Error - Throws with kind `validation` if a payload, `asOf`, `model`, or a metric identifier is invalid; kind `not_found` if required market data is missing; kind `invalid_type` for a wrong argument type; and kind `computation` if pricing or a metric fails.
            pub fn price(
                &self,
                market_json: JsValue,
                as_of: JsValue,
                model: Option<JsValue>,
                metrics: Option<JsValue>,
                metric_pricing_overrides: Option<JsValue>,
                market_history: Option<JsValue>,
            ) -> Result<JsValue, JsValue> {
                $crate::api::valuations::pricing::PriceRequest::from_js(
                    &market_json,
                    &as_of,
                    model.as_ref(),
                    metrics.as_ref(),
                    metric_pricing_overrides.as_ref(),
                    market_history.as_ref(),
                )?
                .price(&self.to_json()?)
            }

            /// Compute one metric of the instrument against a market snapshot.
            ///
            /// The same Rust metric path as `price(..., [metricId])`, returning just the value.
            /// @param market_json - Canonical market-context JSON (string or plain object) supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param metric_id - Fully qualified metric identifier, e.g. `"dv01"` or `"par_rate"`.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns The metric value in the metric's documented unit.
            /// @throws Error - Throws with kind `validation` if the market JSON, `asOf`, `model` or `metricId` is invalid or the metric is not defined for this instrument; kind `not_found` if required market data is missing; and kind `computation` if the calculation fails.
            pub fn metric(
                &self,
                market_json: JsValue,
                as_of: JsValue,
                metric_id: JsValue,
                model: Option<JsValue>,
            ) -> Result<f64, JsValue> {
                let metric_id = crate::utils::input::js_string(&metric_id, "metricId")?;
                $crate::api::valuations::typed::metric_value(
                    &self.to_json()?,
                    &market_json,
                    &as_of,
                    model.as_ref(),
                    &metric_id,
                )
            }
        }
    };
}

/// Read-only property getters: `name as jsName => conversion(expression)`.
///
/// `$i` binds the wrapped Rust instrument inside each expression.
macro_rules! getters {
    ($ty:ident, $js:literal, |$i:ident| {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident => $conv:ident($value:expr) ),* $(,)?
    }) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(getter, js_name = $js_name)]
                pub fn $name(&self) -> Result<JsValue, JsValue> {
                    let $i = &self.inner;
                    $crate::api::valuations::typed::get::$conv(&$value)
                }
            )*
        }
    };
}

/// Zero-argument accessor methods: `name as jsName => conversion(expression)`.
macro_rules! accessors {
    ($ty:ident, $js:literal, |$i:ident| {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident => $conv:ident($value:expr) ),* $(,)?
    }) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name(&self) -> Result<JsValue, JsValue> {
                    let $i = &self.inner;
                    $crate::api::valuations::typed::get::$conv(&$value)
                }
            )*
        }
    };
}

/// Zero-argument static factories that return the class (`example()` and friends).
macro_rules! factories {
    ($ty:ident, $js:literal, $rust:ty, {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident ),* $(,)?
    }) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name() -> Result<$ty, JsValue> {
                    <$rust>::$name()
                        .map(|inner| Self { inner })
                        .map_err(crate::utils::to_js_err)
                }
            )*
        }
    };
}

/// Methods of the form `fn(&self, market, as_of) -> Result<f64>`.
macro_rules! market_metrics {
    ($ty:ident, $js:literal, {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident ),* $(,)?
    }) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name(&self, market_json: JsValue, as_of: JsValue) -> Result<f64, JsValue> {
                    let market = $crate::api::valuations::typed::market(&market_json)?;
                    let as_of = $crate::api::valuations::typed::as_of(&as_of)?;
                    self.inner
                        .$name(&market, as_of)
                        .map_err(crate::utils::to_js_err)
                }
            )*
        }
    };
}

/// One Greek method per `(name, jsName, "metric_id")` through the metric path.
macro_rules! greek_methods {
    ($ty:ident, $js:literal, {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident => $metric:literal ),* $(,)?
    }) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name(
                    &self,
                    market_json: JsValue,
                    as_of: JsValue,
                    model: Option<JsValue>,
                ) -> Result<f64, JsValue> {
                    $crate::api::valuations::typed::metric_value(
                        &self.to_json()?,
                        &market_json,
                        &as_of,
                        model.as_ref(),
                        $metric,
                    )
                }
            )*
        }
    };
}

/// A fluent builder class over a Rust `FinancialBuilder` builder.
///
/// `$target` is the WASM class returned by `build()`; `|$b| $finish` turns
/// the Rust builder into the validated Rust instrument.
macro_rules! builder_class {
    ($(#[$doc:meta])* $bty:ident, $bjs:literal, $inner:ty, $target:ident, |$b:ident| $finish:expr) => {
        $(#[$doc])*
        #[wasm_bindgen(js_name = $bjs)]
        pub struct $bty {
            inner: $crate::api::valuations::typed::Staged<$inner>,
        }

        impl $bty {
            /// An empty builder.
            pub(crate) fn new() -> Self {
                Self {
                    inner: $crate::api::valuations::typed::Staged::new(<$inner>::default()),
                }
            }
        }

        #[wasm_bindgen(js_class = $bjs)]
        impl $bty {
            /// Validate the staged fields and build the instrument.
            ///
            /// Runs the Rust `build()` validation and consumes the builder;
            /// create a new builder for the next instrument.
            /// @returns The validated instrument.
            /// @throws Error - Throws with kind `validation` if the builder was already consumed, a required field is missing (the message names the field), or validation fails.
            pub fn build(&self) -> Result<$target, JsValue> {
                let $b = self.inner.take()?;
                let inner = $finish.map_err(crate::utils::to_js_err)?;
                Ok($target { inner })
            }
        }
    };
}

/// Builder setters taking one JavaScript value: `name as jsName => conversion`.
///
/// `name` is the Rust builder setter; an entry may name a different Rust
/// setter with `=> conversion via rust_setter`.
macro_rules! setters {
    ($bty:ident, $bjs:literal, {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident => $conv:ident $(via $setter:ident)? ),* $(,)?
    }) => {
        #[wasm_bindgen(js_class = $bjs)]
        impl $bty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name(&self, value: JsValue) -> Result<$bty, JsValue> {
                    let value = $crate::api::valuations::typed::arg::$conv(&value, "value")?;
                    let inner = self.inner.apply(|b| setters!(@call b, $name, value $(, $setter)?))?;
                    Ok(Self { inner })
                }
            )*
        }
    };
    (@call $b:ident, $name:ident, $value:ident) => { $b.$name($value) };
    (@call $b:ident, $name:ident, $value:ident, $setter:ident) => { $b.$setter($value) };
}

/// Builder setters taking a WASM handle (`Money`, `Tenor`, `DayCount`, `Currency`).
macro_rules! handle_setters {
    ($bty:ident, $bjs:literal, {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident => $handle:ident $(via $setter:ident)? ),* $(,)?
    }) => {
        #[wasm_bindgen(js_class = $bjs)]
        impl $bty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name(&self, value: &$handle) -> Result<$bty, JsValue> {
                    let value = value.inner;
                    let inner = self.inner.apply(|b| setters!(@call b, $name, value $(, $setter)?))?;
                    Ok(Self { inner })
                }
            )*
        }
    };
}
