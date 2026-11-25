//! Macros that emit the typed-instrument members.
//!
//! Each macro expands to `#[wasm_bindgen]` items whose bodies are one
//! conversion and one Rust call. The documentation for every member is
//! written at the invocation and forwarded with `#[doc]`, so wasm-bindgen
//! copies it into the generated TypeScript declarations.

/// A typed instrument class: the wrapper struct, `fromJson`, `toJson` and `id`.
macro_rules! instrument_class {
    ($(#[$doc:meta])* $ty:ident, $js:literal, $rust:ty) => {
        #[finstack_quant_wasm_macros::plain_docs]
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
            pub fn from_json(
                #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
            ) -> Result<$ty, JsValue> {
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
                finstack_quant_valuations::instruments::Instrument::id(&self.inner).to_string()
            }
        }
    };
}

/// The shared surface of a typed instrument class: the static `builder()`
/// entry point, `toDict`, `marketDependencies`, and `price` / `metric`.
///
/// `$dict` is the TypeScript type of `toDict()` (the class's generated spec
/// type).
macro_rules! instrument_surface {
    ($ty:ident, $js:literal, $builder:ident, $dict:literal) => {
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

            /// The bare instrument spec as a plain object (no envelope).
            ///
            /// Mirrors Python `to_dict()`: the serde form of the Rust struct,
            /// i.e. the `instrument.spec` payload of `toJson()`.
            /// @returns Plain-object instrument spec.
            /// @throws Error - Throws if the instrument cannot be serialized.
            #[wasm_bindgen(js_name = toDict, unchecked_return_type = $dict)]
            pub fn to_dict(&self) -> Result<JsValue, JsValue> {
                crate::utils::to_js_value(&self.inner)
            }

            /// Market data this instrument needs to price (mirrors Rust `Instrument::market_dependencies`).
            /// @returns `MarketDependencies` plain object listing discount, forward and credit curves, spot ids, volatility surfaces, FX pairs and fixing series.
            /// @throws Error - Throws with kind `validation` if the instrument cannot enumerate its dependencies.
            #[wasm_bindgen(js_name = marketDependencies)]
            #[wasm_bindgen(unchecked_return_type = "generated.valuations.MarketDependencies")]
            pub fn market_dependencies(&self) -> Result<JsValue, JsValue> {
                $crate::api::valuations::typed::market_dependencies(&self.inner)
            }
        }
        instrument_pricing!($ty, $js);
    };
}

/// Expose `toBuilder()` cloning a typed instrument back into its fluent builder.
macro_rules! to_builder_method {
    ($(#[$doc:meta])* $ty:ident, $js:literal, $bty:ident) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(#[$doc])*
            #[wasm_bindgen(js_name = toBuilder)]
            pub fn to_builder(&self) -> $bty {
                $bty {
                    inner: $crate::api::valuations::typed::Staged::new(self.inner.to_builder()),
                }
            }
        }
    };
}

/// Expose the complete selected-model Greek bundle (`greeks(...)`) on an option class.
#[rustfmt::skip]
macro_rules! option_greeks_bundle {
    ($rust_name:ident, $js_name:literal) => {
        #[wasm_bindgen(js_class = $js_name)]
        impl $rust_name {
            /// Compute every Greek that applies to this instrument as a JavaScript object.
            /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @param extras - Optional pricing configuration and overrides accepted by `price`.
            /// @returns Map of Greek name to value, with keys in the Rust
            /// `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`, …);
            /// only the Greeks that apply to the instrument are present.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; a returned Greek is non-finite; or the result cannot
            /// be converted to a JavaScript value.
            #[wasm_bindgen(unchecked_return_type = "Record<string, number>")]
            pub fn greeks(
                &self,
                #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
                market_json: JsValue,
                #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
                #[wasm_bindgen(unchecked_optional_param_type = "string | null")] model: Option<
                    JsValue,
                >,
                #[wasm_bindgen(unchecked_optional_param_type = "generated.valuations.PriceExtras | JsonInput | null")]
                extras: Option<JsValue>,
            ) -> Result<JsValue, JsValue> {
                let market_json = $crate::utils::input::json_text(&market_json, "marketJson")?;
                let as_of = $crate::utils::input::js_string(&as_of, "asOf")?;
                let model = $crate::utils::input::js_opt_string(model.as_ref(), "model")?;
                $crate::api::valuations::typed::option_greeks(
                    Box::new(self.inner.clone()),
                    &market_json,
                    &as_of,
                    model.as_deref(),
                    extras.as_ref(),
                )
            }
        }
    };
}
/// `price` and `metric` for a typed instrument class.
// rustfmt cannot lay out attributed parameters inside a macro body: it keeps
// re-indenting them on every run, so this macro keeps its hand layout.
#[rustfmt::skip]
macro_rules! instrument_pricing {
    ($ty:ident, $js:literal) => {
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            /// Price the instrument against a market snapshot.
            ///
            /// Same pipeline and arguments as `valuations.instruments.priceInstrument`.
            /// @param marketJson - Canonical market-context JSON (string or plain object) supplying curves, quotes, and FX data.
            /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @param metrics - Optional canonical metric IDs such as `"dv01"` or `"cs01"`. Omit, `null`, or `undefined` for a valuation-only result.
            /// @param extras - Optional `PriceExtras` (plain object or JSON text); omit, `null`, or `undefined` for none. Keys, each optional: `metric_pricing_overrides` - a `MetricPricingOverrides` JSON object patch merged into the envelope before validation (only supplied fields replace stored overrides, `bump_config` fields merge, an explicit `null` clears a nullable field; omitted keeps the stored configuration); `instrument_pricing_overrides` - object patch for instrument-owned quotes and model settings, applied only to this request; `market_history` - a `MarketHistory` object required by historical risk metrics such as `"hvar"` and `"expected_shortfall"` (omitted prices without history); `explain` - boolean, default `false`; `true` attaches a step trace as `result.explanation`.
            /// @returns Structured `ValuationResult` for the selected model.
            /// @throws Error - Throws with kind `validation` if a payload (including an unknown `extras` key), `asOf`, `model`, or a metric identifier is invalid; kind `not_found` if required market data is missing; kind `invalid_type` for a wrong argument type; and kind `computation` if pricing or a metric fails.
            #[wasm_bindgen(unchecked_return_type = "ValuationResult")]
            pub fn price(
                &self,
                #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
                market_json: JsValue,
                #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")]
                as_of: JsValue,
                #[wasm_bindgen(unchecked_optional_param_type = "string | null")]
                model: Option<JsValue>,
                #[wasm_bindgen(unchecked_optional_param_type = "string[] | null")]
                metrics: Option<JsValue>,
                #[wasm_bindgen(unchecked_optional_param_type = "generated.valuations.PriceExtras | JsonInput | null")]
                extras: Option<JsValue>,
            ) -> Result<JsValue, JsValue> {
                let request = $crate::api::valuations::pricing::PriceRequest::from_js(
                    &as_of,
                    model.as_ref(),
                    metrics.as_ref(),
                    extras.as_ref(),
                )?;
                let instrument = request.typed_instrument(Box::new(self.inner.clone()))?;
                request.price(
                    &instrument,
                    &$crate::api::valuations::pricing::parse_market(&market_json)?,
                )
            }

            /// Compute one metric of the instrument against a market snapshot.
            ///
            /// The same Rust metric path as `price(..., [metricId])`, returning just the value.
            /// @param marketJson - Canonical market-context JSON (string or plain object) supplying curves, quotes, and FX data.
            /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param metricId - Fully qualified metric identifier, e.g. `"dv01"` or `"par_rate"`.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @param extras - Optional pricing configuration, history and overrides accepted by `price`.
            /// @returns The metric value in the metric's documented unit.
            /// @throws Error - Throws with kind `validation` if the market JSON, `asOf`, `model` or `metricId` is invalid or the metric is not defined for this instrument; kind `not_found` if required market data is missing; and kind `computation` if the calculation fails.
            pub fn metric(
                &self,
                #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")] market_json: JsValue,
                #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
                #[wasm_bindgen(js_name = "metricId", unchecked_param_type = "string")] metric_id: JsValue,
                #[wasm_bindgen(unchecked_optional_param_type = "string | null")]
                model: Option<JsValue>,
                #[wasm_bindgen(unchecked_optional_param_type = "generated.valuations.PriceExtras | JsonInput | null")]
                extras: Option<JsValue>,
            ) -> Result<f64, JsValue> {
                let metric_id = crate::utils::input::js_string(&metric_id, "metricId")?;
                $crate::api::valuations::typed::metric_value(
                    Box::new(self.inner.clone()),
                    &market_json,
                    &as_of,
                    model.as_ref(),
                    &metric_id,
                    extras.as_ref(),
                )
            }
        }
    };
}

/// Read-only property getters: `name as jsName: "TsType" => conversion(expression)`.
///
/// `$i` binds the wrapped Rust instrument inside each expression; `TsType` is
/// the getter's TypeScript type.
macro_rules! getters {
    ($ty:ident, $js:literal, |$i:ident| {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident : $ts:literal => $conv:ident($value:expr_2021) ),* $(,)?
    }) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(getter, js_name = $js_name, unchecked_return_type = $ts)]
                pub fn $name(&self) -> Result<JsValue, JsValue> {
                    let $i = &self.inner;
                    $crate::api::valuations::typed::get::$conv(&$value)
                }
            )*
        }
    };
}

/// Zero-argument accessor methods: `name as jsName: "TsType" => conversion(expression)`.
macro_rules! accessors {
    ($ty:ident, $js:literal, |$i:ident| {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident : $ts:literal => $conv:ident($value:expr_2021) ),* $(,)?
    }) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name, unchecked_return_type = $ts)]
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
        #[finstack_quant_wasm_macros::plain_docs]
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
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name(
                    &self,
                    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")] market_json: JsValue,
                    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
                ) -> Result<f64, JsValue> {
                    let market = $crate::api::valuations::pricing::parse_market(&market_json)?;
                    let as_of = $crate::utils::wire::js_date(&as_of, "asOf")?;
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
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = $js)]
        impl $ty {
            $(
                $(#[$doc])*
                /// @param extras - Optional pricing configuration and overrides accepted by `price`.
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name(
                    &self,
                    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")] market_json: JsValue,
                    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
                    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] model: Option<JsValue>,
                    #[wasm_bindgen(unchecked_optional_param_type = "generated.valuations.PriceExtras | JsonInput | null")]
                extras: Option<JsValue>,
                ) -> Result<f64, JsValue> {
                    $crate::api::valuations::typed::metric_value(
                        Box::new(self.inner.clone()),
                        &market_json,
                        &as_of,
                        model.as_ref(),
                        $metric,
                        extras.as_ref(),
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
    // The builder struct and its empty constructor, staging `$empty`; a
    // builder whose `build()` is not a typed class writes `build` itself.
    (@staged $(#[$doc:meta])* $bty:ident, $bjs:literal, $inner:ty, $empty:expr_2021) => {
        #[finstack_quant_wasm_macros::plain_docs]
        $(#[$doc])*
        #[wasm_bindgen(js_name = $bjs)]
        pub struct $bty {
            inner: $crate::api::valuations::typed::Staged<$inner>,
        }

        impl $bty {
            /// An empty builder.
            pub(crate) fn new() -> Self {
                Self {
                    inner: $crate::api::valuations::typed::Staged::new($empty),
                }
            }
        }
    };
    ($(#[$doc:meta])* $bty:ident, $bjs:literal, $inner:ty, $target:ident, |$b:ident| $finish:expr_2021) => {
        builder_class!(@staged $(#[$doc])* $bty, $bjs, $inner, <$inner>::default());

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

/// Builder setters taking one JavaScript value: `name as jsName: "TsType" => conversion`.
///
/// `name` is the Rust builder setter; an entry may name a different Rust
/// setter with `=> conversion via rust_setter`. `TsType` is the TypeScript
/// type of the value.
macro_rules! setters {
    ($bty:ident, $bjs:literal, {
        $( $(#[$doc:meta])* $name:ident as $js_name:ident : $ts:literal => $conv:ident $(via $setter:ident)? ),* $(,)?
    }) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = $bjs)]
        impl $bty {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name(
                    &self,
                    #[wasm_bindgen(unchecked_param_type = $ts)] value: JsValue,
                ) -> Result<$bty, JsValue> {
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
        #[finstack_quant_wasm_macros::plain_docs]
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

/// The pricing-override and attribute getters shared by the typed instrument
/// classes.
///
/// The bare form emits both groups; `overrides` emits only the three
/// `*PricingOverrides` getters and `attributes` only `attributes`, for classes
/// whose other getter carries its own documentation or does not exist.
macro_rules! common_getters {
    ($ty:ident, $js:literal) => {
        common_getters!($ty, $js, overrides);
        common_getters!($ty, $js, attributes);
    };
    ($ty:ident, $js:literal, overrides) => {
        getters!($ty, $js, |i| {
            /// Instrument-owned pricing inputs (`InstrumentPricingOverrides`) in serde form: `market_quotes` (quoted clean price, yield, spreads, implied volatility, premium) and `model_config` (tree model and steps, Monte Carlo and Hull-White parameters).
            /// @returns The overrides plain object; `{}` when none is set.
            instrument_pricing_overrides as instrumentPricingOverrides: "generated.valuations.InstrumentPricingOverrides" => json(i.instrument_pricing_overrides),
            /// Metric-time pricing configuration (`MetricPricingOverrides`: bump sizes, theta period, ...) stored on the instrument; an `extras.metric_pricing_overrides` patch passed to `price` is merged over it per call.
            /// @returns The overrides plain object; unset fields are `null` or omitted.
            metric_pricing_overrides as metricPricingOverrides: "generated.valuations.MetricPricingOverrides" => json(i.metric_pricing_overrides),
            /// Scenario-only pricing adjustments (`ScenarioPricingOverrides`: `scenario_price_shock_decimal`, `scenario_spread_shock_bp`).
            /// @returns The overrides plain object; `{}` when none is set.
            scenario_pricing_overrides as scenarioPricingOverrides: "generated.valuations.ScenarioPricingOverrides" => json(i.scenario_pricing_overrides),
        });
    };
    ($ty:ident, $js:literal, attributes) => {
        getters!($ty, $js, |i| {
            /// Instrument attributes (tags and metadata) used for scenario selection.
            /// @returns The attribute bag; empty when none were set.
            attributes as attributes: "generated.valuations.Attributes" => json(i.attributes),
        });
    };
}

/// The pricing-override and attribute setters shared by the typed instrument
/// builders; the forms match `common_getters!`.
macro_rules! common_setters {
    ($bty:ident, $bjs:literal) => {
        common_setters!($bty, $bjs, overrides);
        common_setters!($bty, $bjs, attributes);
    };
    ($bty:ident, $bjs:literal, overrides) => {
        setters!($bty, $bjs, {
            /// Set the instrument-owned pricing inputs.
            /// @param value - Rust `InstrumentPricingOverrides` in serde form (plain object or JSON string): `{"market_quotes": {...}, "model_config": {...}}`, e.g. `{"market_quotes": {"quoted_clean_price_pct": 99.0}}`.
            /// @returns The builder, for chaining.
            /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
            instrument_pricing_overrides as instrumentPricingOverrides: "generated.valuations.InstrumentPricingOverrides" => from_js_json,
            /// Set the metric-time pricing configuration stored on the instrument.
            /// @param value - Rust `MetricPricingOverrides` in serde form (plain object or JSON string), e.g. `{"bump_config": {"rate_bump_bp": 0.5}}`.
            /// @returns The builder, for chaining.
            /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
            metric_pricing_overrides as metricPricingOverrides: "generated.valuations.MetricPricingOverrides" => from_js_json,
            /// Set the scenario-only pricing adjustments.
            /// @param value - Rust `ScenarioPricingOverrides` in serde form (plain object or JSON string), e.g. `{"scenario_price_shock_decimal": -0.02}`.
            /// @returns The builder, for chaining.
            /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
            scenario_pricing_overrides as scenarioPricingOverrides: "generated.valuations.ScenarioPricingOverrides" => from_js_json,
        });
    };
    ($bty:ident, $bjs:literal, attributes) => {
        setters!($bty, $bjs, {
            /// Set instrument attributes (tags and metadata).
            /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
            /// @returns The builder, for chaining.
            /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
            attributes as attributes: "generated.valuations.Attributes" => from_js_json,
        });
    };
}

/// A builder setter taking `(paymentDate, amount)`: an ISO-8601 date and a
/// `Money` handle, passed as one `(Date, Money)` pair to the Rust setter of
/// the same name.
macro_rules! dated_money_setter {
    ($bty:ident, $bjs:literal, $(#[$doc:meta])* $name:ident) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = $bjs)]
        impl $bty {
            $(#[$doc])*
            pub fn $name(
                &self,
                #[wasm_bindgen(js_name = "paymentDate", unchecked_param_type = "string")] payment_date: JsValue,
                amount: &crate::api::core::money::JsMoney,
            ) -> Result<$bty, JsValue> {
                let value = (
                    $crate::utils::wire::js_date(&payment_date, "paymentDate")?,
                    amount.inner,
                );
                let inner = self.inner.apply(|b| b.$name(value))?;
                Ok(Self { inner })
            }
        }
    };
}

/// Bind one `*FutureOption` WASM wrapper, its fluent builder, and `toBuilder`.
macro_rules! wasm_future_option {
    (
        $ty:ident, $js:literal, $bty:ident, $bjs:literal,
        $rust_ty:path, $rust_builder:path, $dict:literal
    ) => {
        instrument_class!(
            /// Exchange-listed option on a futures price.
            ///
            /// @example
            /// ```typescript
            /// import init, { valuations } from "finstack-quant-wasm";
            /// await init();
            #[doc = concat!("const future = valuations.instruments.", $js, ".example();")]
            /// console.log(future.terms.strike);
            /// future.free();
            /// ```
            $ty, $js, $rust_ty
        );
        instrument_surface!($ty, $js, $bty, $dict);
        factories!($ty, $js, $rust_ty, {
            /// Construct the native option-on-future example.
            /// @returns Example with an explicit pricing model and reference price units.
            /// @throws FinstackError if the example terms are invalid.
            example as example,
        });
        getters!($ty, $js, |i| {
            /// Effective expiry of the listed contract.
            /// @returns The ISO expiry, or null if the instrument defines none.
            expiry as expiry: "string | null" => opt_date(finstack_quant_valuations::instruments::Instrument::expiry(i)),
            /// Registered pricing model for the contract.
            /// @returns The registered default pricing model name.
            default_model as defaultModel: "string" => text(finstack_quant_valuations::instruments::Instrument::default_model(i)),
            /// Return the contract's position and lifecycle terms.
            /// @returns Independent copy of option and underlying marks, payoff and lifecycle terms.
            terms as terms: "generated.valuations.FutureOptionTerms" => json(i.terms),
        });
        common_getters!($ty, $js);

        builder_class!(
            /// Builder for a listed option on a futures contract.
            ///
            /// @example
            /// ```typescript
            /// import init, { valuations } from "finstack-quant-wasm";
            /// await init();
            #[doc = concat!("const initial = valuations.instruments.", $js, ".example();")]
            /// const builder = initial.toBuilder();
            /// const updated = builder.id("OPTION-REVIEW").build();
            /// console.log(updated.id);
            /// updated.free();
            /// initial.free();
            /// ```
            $bty, $bjs, $rust_builder, $ty, |b| b.build()
        );
        to_builder_method!(
            /// Seed an independent builder from the complete option contract.
            /// @returns Builder carrying exercise, settlement, mark and pricing inputs.
            $ty, $js, $bty
        );
        setters!($bty, $bjs, {
            /// Set the stable contract identifier.
            /// @param value - Identifier used for scenario selection and metrics.
            /// @returns The active builder.
            /// @throws FinstackError for invalid inputs or an inactive builder.
            id as id: "string" => id,
            /// Set the contract's terms.
            /// @param value - Option strike, futures mark, contract count, model and exercise and settlement terms.
            /// @returns The active builder.
            /// @throws FinstackError for invalid inputs or an inactive builder.
            terms as terms: "generated.valuations.FutureOptionTerms" => js_wire,
        });
        common_setters!($bty, $bjs);
    };
}
