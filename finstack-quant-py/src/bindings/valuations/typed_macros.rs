//! Macros shared by every typed instrument wrapper and builder.
//!
//! `instrument_pricing_methods!` stamps the common `price` / `metric` /
//! `market_dependencies` / `default_model` / `attributes` / `to_dict` surface
//! onto a wrapper, `instrument_envelope_methods!` the `builder` /
//! `__reduce__` / `from_json` / `to_json` / `id` surface, and `builder_set!`
//! is the body of one consuming builder setter. Each macro expands to a
//! `#[pymethods]` block (the crate enables `multiple-pymethods`), so the
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

/// Stamp `__reduce__` / `from_json` / `to_json` / `id` / `builder` on a typed
/// instrument wrapper.
///
/// `$variant` is the `InstrumentJson` variant, `$type_tag` the serde type tag
/// (`"fx_forward"`), `$builder` the Python builder wrapper and `$seed` an
/// expression producing the seeded Rust builder. The wrapper must expose
/// `pub(crate) inner` and `envelope_json()`.
///
/// The trailing selector names the builder wrapper's shape, as in
/// `pricing_override_methods!`: `fields` (the default) for a builder that
/// records `fields: Vec<(&'static str, String)>` for `__repr__`, `no_fields`
/// for one that only holds `inner`. The optional `builder_doc = [ ... ]` list
/// appends docstring lines (each a `str` literal with its own leading space,
/// typically a NumPy ``Notes`` section) after the `builder` docstring's
/// ``Returns`` section, for instrument-specific notes on the builder's
/// defaults.
macro_rules! instrument_envelope_methods {
    ($ty:ident, $variant:ident, $type_tag:literal, $builder:ident, $seed:expr) => {
        $crate::bindings::valuations::typed_macros::instrument_envelope_methods!(
            $ty,
            $variant,
            $type_tag,
            $builder,
            $seed,
            fields,
            builder_doc = []
        );
    };
    ($ty:ident, $variant:ident, $type_tag:literal, $builder:ident, $seed:expr, $shape:ident) => {
        $crate::bindings::valuations::typed_macros::instrument_envelope_methods!(
            $ty,
            $variant,
            $type_tag,
            $builder,
            $seed,
            $shape,
            builder_doc = []
        );
    };
    (
        $ty:ident, $variant:ident, $type_tag:literal, $builder:ident, $seed:expr,
        builder_doc = [ $($builder_doc:literal),* $(,)? ]
    ) => {
        $crate::bindings::valuations::typed_macros::instrument_envelope_methods!(
            $ty,
            $variant,
            $type_tag,
            $builder,
            $seed,
            fields,
            builder_doc = [$($builder_doc),*]
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
    (
        $ty:ident, $variant:ident, $type_tag:literal, $builder:ident, $seed:expr, $shape:ident,
        builder_doc = [ $($builder_doc:literal),* $(,)? ]
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
                self.envelope_json()
            }

            /// Instrument identifier.
            #[getter]
            fn id(&self) -> String {
                self.inner.id.to_string()
            }
        }
    };
}
pub(crate) use instrument_envelope_methods;

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
