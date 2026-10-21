//! Macros shared by every typed instrument wrapper and builder.
//!
//! `instrument_pricing_methods!` stamps the common `price` / `metric` /
//! `market_dependencies` / `default_model` / `attributes` / `to_dict` surface
//! onto a wrapper, `instrument_envelope_methods!` the `builder` /
//! `__reduce__` / `from_json` / `to_json` / `id` surface plus the builder's
//! `build` / `__repr__` / pricing-override setters, and `builder_set!` is the
//! body of one consuming builder setter. Each macro expands to
//! `#[pymethods]` blocks (the crate enables `multiple-pymethods`), so the
//! per-instrument constructors, accessors and typed helpers stay in a
//! hand-written block beside the invocations.

/// Stamp the pricing surface shared by every typed instrument wrapper.
///
/// Expands to a `#[pymethods]` block with `price`, `metric`,
/// `market_dependencies`, `default_model`, `attributes` and `to_dict`. The
/// wrapper must expose `pub(crate) inner` (the Rust instrument).
///
/// The optional `model_doc = [ ... ]` list holds extra docstring lines
/// (each a `str` literal, already indented as a NumPy continuation line, i.e.
/// five leading spaces) appended to the `model` parameter of both `price` and
/// `metric`; use it when the instrument's model keys need explaining.
macro_rules! instrument_pricing_methods {
    ($ty:ident) => {
        $crate::bindings::valuations::typed_macros::instrument_pricing_methods!(
            $ty,
            model_doc = []
        );
    };
    ($ty:ident, model_doc = [ $($model_doc:literal),* $(,)? ]) => {
        #[pymethods]
        impl $ty {
            /// Price this instrument and return a typed ``ValuationResult``.
            ///
            /// Delegates to the same canonical Rust pricer entry point as
            /// ``price_instrument(self, market, as_of, model)``.
            ///
            /// Parameters
            /// ----------
            /// market : MarketContext | str
            ///     A ``MarketContext`` object or serialized market-context JSON.
            /// as_of : datetime.date | str
            ///     Valuation date, either a date-like object or an ISO 8601 string.
            /// model : str, optional
            ///     Model key (default ``"default"`` — the instrument-native model).
            $(#[doc = $model_doc])*
            /// metrics : list[str], optional
            ///     Metric identifiers to compute (e.g. ``["dv01", "theta"]``).
            ///     Empty or omitted means valuation only.
            /// metric_pricing_overrides : MetricPricingOverrides | dict | str | None
            ///     Metric-time overrides merged into
            ///     ``instrument.spec.metric_pricing_overrides`` before pricing.
            /// market_history : MarketHistory | dict | str | None
            ///     Historical ``MarketHistory`` scenarios required by ``hvar`` and
            ///     ``expected_shortfall`` metrics.
            ///
            /// Returns
            /// -------
            /// ValuationResult
            ///     Typed valuation envelope carrying value, currency, and metrics.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the market JSON, ``as_of``, or ``model`` is invalid, or the
            ///     selected pricer rejects the instrument.
            /// KeyError
            ///     If a curve, surface, or price the instrument depends on is
            ///     missing from ``market``.
            /// RuntimeError
            ///     If the pricer or a requested metric fails numerically.
            #[pyo3(signature = (market, as_of, model="default", metrics=None, metric_pricing_overrides=None, market_history=None))]
            #[pyo3(text_signature = "($self, market, as_of, model='default', metrics=None, metric_pricing_overrides=None, market_history=None)")]
            #[allow(clippy::too_many_arguments)]
            fn price(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &Bound<'_, PyAny>,
                model: &str,
                metrics: Option<Vec<String>>,
                metric_pricing_overrides: Option<&Bound<'_, PyAny>>,
                market_history: Option<&Bound<'_, PyAny>>,
            ) -> PyResult<$crate::bindings::valuations::PyValuationResult> {
                $crate::bindings::valuations::instruments::price_typed(
                    py,
                    Box::new(self.inner.clone()),
                    market,
                    as_of,
                    model,
                    metrics,
                    metric_pricing_overrides,
                    market_history,
                )
            }

            /// Compute one scalar metric for this instrument.
            ///
            /// Mirrors Rust ``pricer::metric_value``: the instrument is priced
            /// under ``model`` and the single metric ``metric_id`` is returned as
            /// a float.
            ///
            /// Parameters
            /// ----------
            /// market : MarketContext | str
            ///     A ``MarketContext`` object or serialized market-context JSON.
            /// as_of : datetime.date | str
            ///     Valuation date, either a date-like object or an ISO 8601 string.
            /// metric_id : str
            ///     Fully qualified metric identifier, e.g. ``"dv01"``,
            ///     ``"cs01"``, ``"delta"``.
            /// model : str, optional
            ///     Model key (default ``"default"`` — the instrument-native model).
            $(#[doc = $model_doc])*
            ///
            /// Returns
            /// -------
            /// float
            ///     The metric value in the metric's documented unit.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If ``metric_id`` is unknown, ``as_of`` or ``model`` is invalid,
            ///     or the metric is not defined for this instrument.
            /// KeyError
            ///     If required market data is missing from ``market``.
            /// RuntimeError
            ///     If the metric computation fails numerically.
            #[pyo3(signature = (market, as_of, metric_id, model="default"))]
            #[pyo3(text_signature = "($self, market, as_of, metric_id, model='default')")]
            fn metric(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &Bound<'_, PyAny>,
                metric_id: &str,
                model: &str,
            ) -> PyResult<f64> {
                $crate::bindings::valuations::instruments::metric_typed(
                    py,
                    Box::new(self.inner.clone()),
                    market,
                    as_of,
                    metric_id,
                    model,
                )
            }

            /// Market objects this instrument needs for pricing.
            ///
            /// Mirrors Rust ``Instrument::market_dependencies``.
            ///
            /// Returns
            /// -------
            /// dict[str, object]
            ///     Serde view of ``MarketDependencies``: ``curves`` (discount /
            ///     forward / credit / inflation curve ids), ``credit_index_ids``,
            ///     ``market_scalar_ids``, ``volatility_dependencies``,
            ///     ``fx_pairs`` and ``series_ids``.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the instrument cannot enumerate its dependencies.
            #[pyo3(text_signature = "($self)")]
            fn market_dependencies<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                let deps = finstack_quant_valuations::instruments::Instrument::market_dependencies(
                    &self.inner,
                )
                .map_err($crate::errors::core_to_py)?;
                $crate::bindings::pandas_utils::serde_to_py(py, &deps)
            }

            /// Model key the pricer uses when ``model="default"``.
            ///
            /// Returns
            /// -------
            /// str
            ///     Canonical model key, e.g. ``"hazard_rate"`` or ``"black76"``.
            #[getter]
            fn default_model(&self) -> String {
                finstack_quant_valuations::instruments::Instrument::default_model(&self.inner)
                    .to_string()
            }

            /// Free-form instrument attributes (tags and metadata).
            ///
            /// Returns
            /// -------
            /// Attributes
            ///     Copy of the instrument's attribute bag.
            #[getter]
            fn attributes(&self) -> $crate::bindings::core::types::PyAttributes {
                $crate::bindings::valuations::convert::attributes_to_py(
                    finstack_quant_valuations::instruments::Instrument::attributes(&self.inner),
                )
            }

            /// Instrument specification as a plain dict.
            ///
            /// Returns
            /// -------
            /// dict[str, object]
            ///     The canonical ``spec`` payload (the same fields ``to_json``
            ///     wraps in the ``finstack_quant.instrument/1`` envelope).
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the instrument cannot be serialized.
            #[pyo3(text_signature = "($self)")]
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                $crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
            }
        }
    };
}
pub(crate) use instrument_pricing_methods;

/// Stamp fixed-metric Greek methods on a typed option wrapper.
///
/// Each entry `/// docs` + `name` expands to
/// `name(market, as_of, model="default") -> float`, which prices the wrapped
/// instrument and returns the metric whose id is `stringify!(name)` (the same
/// call as ``metric(market, as_of, "name", model)``). The docstring is
/// forwarded verbatim.
macro_rules! instrument_greek_methods {
    ($ty:ident { $( $(#[$doc:meta])* $name:ident ),+ $(,)? }) => {
        #[pymethods]
        impl $ty {
            $(
                $(#[$doc])*
                #[pyo3(signature = (market, as_of, model="default"))]
                fn $name(
                    &self,
                    py: Python<'_>,
                    market: &Bound<'_, PyAny>,
                    as_of: &Bound<'_, PyAny>,
                    model: &str,
                ) -> PyResult<f64> {
                    $crate::bindings::valuations::instruments::metric_typed(
                        py,
                        Box::new(self.inner.clone()),
                        market,
                        as_of,
                        stringify!($name),
                        model,
                    )
                }
            )+
        }
    };
}
pub(crate) use instrument_greek_methods;

/// Stamp the envelope and builder surface of a typed instrument wrapper.
///
/// On the instrument wrapper `$ty`: `builder`, `__reduce__`, `from_json`,
/// `to_json` and `id`. On its builder wrapper `$builder`: `build`,
/// `__repr__` and the three
/// `*_pricing_overrides` setters (with the matching getters on `$ty`; see
/// `pricing_override_methods!`).
///
/// Arguments, in order:
///
/// * `$ty` — the Python instrument wrapper; it must expose
///   `pub(crate) inner` (the Rust instrument, convertible with
///   `InstrumentJson::from`).
/// * `$variant` — the Rust type name, used in the `to_json` error context
///   (`"failed to serialize {variant} instrument envelope"`).
/// * `$type_tag` — the serde type tag (`"fx_forward"`), quoted in the
///   `from_json` docstring.
/// * `builder = $builder($builder_name, $shape)` — the Python builder wrapper
///   (`inner: Option<RustBuilder>`), its Python class name, and its shape:
///   `fields` for a builder that records `fields: Vec<(&'static str,
///   String)>` for `__repr__`, `no_fields` for one that only holds `inner`
///   (its repr is `"{builder_name}(consumed=...)"`).
/// * `seed = $seed` — an expression producing the seeded Rust builder.
/// * `builder_doc = [ ... ]` (optional) — docstring lines (each a `str`
///   literal with its own leading space, typically a NumPy ``Notes``
///   section) appended after the `builder` docstring's ``Returns`` section.
/// * the `build` docstring, written as `///` lines before the closing
///   `build` keyword, forwarded verbatim.
macro_rules! instrument_envelope_methods {
    (
        $ty:ident, $variant:ident, $type_tag:literal,
        builder = $builder:ident($builder_name:literal, $shape:ident),
        seed = $seed:expr,
        $(builder_doc = [ $($builder_doc:literal),* $(,)? ],)?
        $(#[$build_doc:meta])*
        build $(,)?
    ) => {
        $crate::bindings::valuations::typed_macros::instrument_envelope_methods!(
            @impl $ty, $variant, $type_tag, $builder, $builder_name, $shape, $seed,
            [$($($builder_doc),*)?],
            $(#[$build_doc])*
        );
        $crate::bindings::valuations::typed_macros::pricing_override_methods!(
            $ty, $builder, $builder_name, $shape
        );
    };
    (@new_builder fields, $builder:ident, $seed:expr) => {
        $builder {
            inner: Some($seed),
            fields: Vec::new(),
        }
    };
    (@new_builder no_fields, $builder:ident, $seed:expr) => {
        $builder { inner: Some($seed) }
    };
    (@repr fields, $builder_name:literal, $slf:ident) => {
        $crate::bindings::valuations::convert::builder_repr($builder_name, &$slf.fields)
    };
    (@repr no_fields, $builder_name:literal, $slf:ident) => {
        format!(
            concat!($builder_name, "(consumed={})"),
            $crate::bindings::valuations::convert::bool_repr($slf.inner.is_none())
        )
    };
    (
        @impl $ty:ident, $variant:ident, $type_tag:literal, $builder:ident,
        $builder_name:literal, $shape:ident, $seed:expr,
        [$($builder_doc:literal),*],
        $(#[$build_doc:meta])*
    ) => {
        #[pymethods]
        impl $ty {
            /// Create a fluent builder (mirrors the Rust ``builder()``).
            ///
            /// Builders are consumed by ``build()``; create a new builder per
            /// instrument.
            ///
            /// Returns
            /// -------
            /// builder
            ///     A builder with fluent, consuming setter methods.
            $(#[doc = $builder_doc])*
            #[staticmethod]
            #[pyo3(text_signature = "()")]
            fn builder() -> $builder {
                $crate::bindings::valuations::typed_macros::instrument_envelope_methods!(
                    @new_builder $shape, $builder, $seed
                )
            }

            /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
            ///
            /// Reconstruction goes through the same strict serde round-trip as
            /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
            /// format defines — there is no second state format that can drift.
            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                $crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }

            /// Deserialize a validated instrument from its canonical v1 envelope.
            ///
            /// Parameters
            /// ----------
            /// json : str
            #[doc = concat!(
                        "    A ``finstack_quant.instrument/1`` envelope carrying an exact \"",
                        $type_tag,
                        "\" payload. The UTF-8 input must not exceed 16 MiB. Bare payloads \
                 and cross-type coercion are rejected."
                    )]
            ///
            /// Returns
            /// -------
            /// instrument
            ///     The validated instrument.
            ///
            /// Raises
            /// ------
            /// ValueError
            #[doc = concat!(
                        "    If the input exceeds 16 MiB, is malformed, has an unsupported \
                 envelope schema, carries a type other than \"",
                        $type_tag,
                        "\", or fails validation."
                    )]
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> PyResult<Self> {
                $crate::bindings::valuations::instruments::parse_typed_instrument_json(json).map(|inner| Self { inner })
            }

            /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
            ///
            /// Returns
            /// -------
            /// str
            ///     Canonical instrument envelope accepted by ``price_instrument`` and
            ///     ``from_json``.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the value cannot be serialized to JSON.
            #[pyo3(text_signature = "($self)")]
            fn to_json(&self) -> PyResult<String> {
                $crate::bindings::valuations::instruments::serialize_typed_instrument_json(
                    finstack_quant_valuations::instruments::InstrumentJson::from(
                        self.inner.clone(),
                    ),
                    stringify!($variant),
                )
            }

            /// Instrument identifier.
            #[getter]
            fn id(&self) -> String {
                self.inner.id.to_string()
            }
        }

        #[pymethods]
        impl $builder {
            $(#[$build_doc])*
            #[pyo3(text_signature = "($self)")]
            fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<$ty> {
                let b = $crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
                let inner = b.build().map_err($crate::errors::core_to_py)?;
                Ok($ty { inner })
            }

            /// Return ``repr(self)``.
            fn __repr__(&self) -> String {
                $crate::bindings::valuations::typed_macros::instrument_envelope_methods!(
                    @repr $shape, $builder_name, self
                )
            }
        }
    };
}
pub(crate) use instrument_envelope_methods;

/// Bind the three Rust `*_pricing_overrides` fields of a typed instrument:
/// serde-dict getters on the instrument and consuming setters on its
/// `FinancialBuilder`. Invoked by `instrument_envelope_methods!`.
///
/// Arguments: the Python instrument wrapper (`inner` is the Rust instrument),
/// the Python builder wrapper (`inner: Option<RustBuilder>`), the builder's
/// Python class name, and `fields` when the builder records set fields for
/// `__repr__` in a `fields` vector (`no_fields` otherwise).
macro_rules! pricing_override_methods {
    (@record fields, $slf:ident, $name:literal) => {
        $slf.fields.push(($name, "{...}".to_string()));
    };
    (@record no_fields, $slf:ident, $name:literal) => {};
    ($instrument:ty, $builder:ty, $builder_name:literal, $record:ident) => {
        #[pyo3::pymethods]
        impl $instrument {
            /// Instrument-owned pricing inputs in serde form.
            ///
            /// Returns
            /// -------
            /// dict[str, object]
            ///     ``InstrumentPricingOverrides``: ``market_quotes`` (quoted
            ///     clean price, yield, spreads, implied volatility, premium) and
            ///     ``model_config`` (tree model and steps, Monte Carlo and
            ///     Hull-White parameters). Empty sections are omitted; ``{}``
            ///     when none is set.
            #[getter]
            fn instrument_pricing_overrides<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::PyAny>> {
                $crate::bindings::pandas_utils::serde_to_py(
                    py,
                    &self.inner.instrument_pricing_overrides,
                )
            }

            /// Metric-time pricing configuration in serde form.
            ///
            /// Returns
            /// -------
            /// dict[str, object]
            ///     ``MetricPricingOverrides`` (bump sizes, theta period, ...);
            ///     unset fields are ``None`` or omitted. A
            ///     ``metric_pricing_overrides`` argument to ``price`` is merged
            ///     over it per call.
            #[getter]
            fn metric_pricing_overrides<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::PyAny>> {
                $crate::bindings::pandas_utils::serde_to_py(
                    py,
                    &self.inner.metric_pricing_overrides,
                )
            }

            /// Scenario-only pricing adjustments in serde form.
            ///
            /// Returns
            /// -------
            /// dict[str, object]
            ///     ``ScenarioPricingOverrides`` (``scenario_price_shock_decimal``,
            ///     ``scenario_spread_shock_bp``); ``{}`` when none is set.
            #[getter]
            fn scenario_pricing_overrides<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::PyAny>> {
                $crate::bindings::pandas_utils::serde_to_py(
                    py,
                    &self.inner.scenario_pricing_overrides,
                )
            }
        }

        #[pyo3::pymethods]
        impl $builder {
            /// Set the instrument-owned pricing inputs.
            ///
            /// Parameters
            /// ----------
            /// value : dict[str, object] | str
            ///     Rust ``InstrumentPricingOverrides`` in serde form (dict or
            ///     JSON string): ``{"market_quotes": {...}, "model_config": {...}}``,
            ///     e.g. ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.
            ///
            /// Returns
            /// -------
            #[doc = concat!(" ", $builder_name)]
            ///     ``self``, for chaining.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the builder was already consumed by ``build()`` or
            ///     ``value`` does not deserialize as ``InstrumentPricingOverrides``.
            #[pyo3(text_signature = "($self, value)")]
            fn instrument_pricing_overrides<'py>(
                mut slf: pyo3::PyRefMut<'py, Self>,
                py: pyo3::Python<'py>,
                value: &pyo3::Bound<'py, pyo3::PyAny>,
            ) -> pyo3::PyResult<pyo3::PyRefMut<'py, Self>> {
                let overrides: finstack_quant_valuations::instruments::InstrumentPricingOverrides =
                    $crate::bindings::module_utils::spec_from_py(
                        py,
                        value,
                        "instrument_pricing_overrides",
                    )?;
                let builder = $crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
                slf.inner = Some(builder.instrument_pricing_overrides(overrides));
                $crate::bindings::valuations::typed_macros::pricing_override_methods!(
                    @record $record, slf, "instrument_pricing_overrides"
                );
                Ok(slf)
            }

            /// Set the metric-time pricing configuration stored on the instrument.
            ///
            /// Parameters
            /// ----------
            /// value : MetricPricingOverrides | dict[str, object] | str
            ///     Rust ``MetricPricingOverrides`` as the typed class, a dict or a
            ///     JSON string.
            ///
            /// Returns
            /// -------
            #[doc = concat!(" ", $builder_name)]
            ///     ``self``, for chaining.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the builder was already consumed by ``build()`` or
            ///     ``value`` does not deserialize as ``MetricPricingOverrides``.
            #[pyo3(text_signature = "($self, value)")]
            fn metric_pricing_overrides<'py>(
                mut slf: pyo3::PyRefMut<'py, Self>,
                py: pyo3::Python<'py>,
                value: &pyo3::Bound<'py, pyo3::PyAny>,
            ) -> pyo3::PyResult<pyo3::PyRefMut<'py, Self>> {
                let overrides =
                    $crate::bindings::valuations::pricing::metric_pricing_overrides_from_py(
                        py, value,
                    )?;
                let builder = $crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
                slf.inner = Some(builder.metric_pricing_overrides(overrides));
                $crate::bindings::valuations::typed_macros::pricing_override_methods!(
                    @record $record, slf, "metric_pricing_overrides"
                );
                Ok(slf)
            }

            /// Set the scenario-only pricing adjustments.
            ///
            /// Parameters
            /// ----------
            /// value : dict[str, object] | str
            ///     Rust ``ScenarioPricingOverrides`` in serde form (dict or JSON
            ///     string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.
            ///
            /// Returns
            /// -------
            #[doc = concat!(" ", $builder_name)]
            ///     ``self``, for chaining.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the builder was already consumed by ``build()`` or
            ///     ``value`` does not deserialize as ``ScenarioPricingOverrides``.
            #[pyo3(text_signature = "($self, value)")]
            fn scenario_pricing_overrides<'py>(
                mut slf: pyo3::PyRefMut<'py, Self>,
                py: pyo3::Python<'py>,
                value: &pyo3::Bound<'py, pyo3::PyAny>,
            ) -> pyo3::PyResult<pyo3::PyRefMut<'py, Self>> {
                let overrides: finstack_quant_valuations::instruments::ScenarioPricingOverrides =
                    $crate::bindings::module_utils::spec_from_py(
                        py,
                        value,
                        "scenario_pricing_overrides",
                    )?;
                let builder = $crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
                slf.inner = Some(builder.scenario_pricing_overrides(overrides));
                $crate::bindings::valuations::typed_macros::pricing_override_methods!(
                    @record $record, slf, "scenario_pricing_overrides"
                );
                Ok(slf)
            }
        }
    };
}
pub(crate) use pricing_override_methods;

/// Apply one consuming Rust setter inside a Python builder method.
///
/// Takes the wrapped Rust builder out of `$slf.inner` (``ValueError`` once
/// `build()` consumed it), stores `$apply(builder)` back and returns
/// `Ok($slf)` for chaining. The four-argument form also records
/// `(stringify!($field), $repr)` in `$slf.fields` for `__repr__`; the
/// two-argument form is for builders without a `fields` vector.
macro_rules! builder_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = $crate::bindings::valuations::convert::take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
    ($slf:ident, $apply:expr) => {{
        let b = $crate::bindings::valuations::convert::take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        Ok($slf)
    }};
}
pub(crate) use builder_set;
