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
/// wrapper must expose `pub(crate) inner` (the Rust instrument); `$name` is its
/// Python class name, which keys the per-class docstrings in the
/// `finstack_quant.valuations.instruments` stub.
macro_rules! instrument_pricing_methods {
    ($ty:ident, $name:literal) => {
        #[pymethods]
        impl $ty {
            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".price"))]
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
                $crate::bindings::valuations::pricing::price_typed(
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

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".metric"))]
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
                $crate::bindings::valuations::pricing::metric_typed(
                    py,
                    Box::new(self.inner.clone()),
                    market,
                    as_of,
                    metric_id,
                    model,
                )
            }

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".market_dependencies"))]
            #[pyo3(text_signature = "($self)")]
            fn market_dependencies<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                let deps = finstack_quant_valuations::instruments::Instrument::market_dependencies(
                    &self.inner,
                )
                .map_err($crate::errors::core_to_py)?;
                $crate::bindings::pandas_utils::serde_to_py(py, &deps)
            }

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".default_model"))]
            #[getter]
            fn default_model(&self) -> String {
                finstack_quant_valuations::instruments::Instrument::default_model(&self.inner)
                    .to_string()
            }

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".attributes"))]
            #[getter]
            fn attributes(&self) -> $crate::bindings::core::types::PyAttributes {
                $crate::bindings::valuations::convert::attributes_to_py(
                    finstack_quant_valuations::instruments::Instrument::attributes(&self.inner),
                )
            }

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".to_dict"))]
            #[pyo3(text_signature = "($self)")]
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                $crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
            }
        }
    };
}
pub(crate) use instrument_pricing_methods;

/// Stamp the envelope and builder surface of a typed instrument wrapper.
///
/// On the instrument wrapper `$ty`: `builder`, `__reduce__`, `from_json`,
/// `to_json` and `id`. On its builder wrapper `$builder`: `build`,
/// `__repr__` and the three
/// `*_pricing_overrides` setters (with the matching getters on `$ty`; see
/// `pricing_override_methods!`). Docstrings come from the
/// `finstack_quant.valuations.instruments` stub, keyed by the Python class names.
///
/// Arguments, in order:
///
/// * `$ty($name)` — the Python instrument wrapper and its Python class name; the
///   wrapper must expose `pub(crate) inner` (the Rust instrument, convertible
///   with `InstrumentJson::from`).
/// * `$variant` — the Rust type name, used in the `to_json` error context
///   (`"failed to serialize {variant} instrument envelope"`).
/// * `builder = $builder($builder_name, $shape)` — the Python builder wrapper
///   (`inner: Option<RustBuilder>`), its Python class name, and its shape:
///   `fields` for a builder that records `fields: Vec<(&'static str,
///   String)>` for `__repr__`, `no_fields` for one that only holds `inner`
///   (its repr is `"{builder_name}(consumed=...)"`).
/// * `seed = $seed` — an expression producing the seeded Rust builder.
macro_rules! instrument_envelope_methods {
    (
        $ty:ident($name:literal), $variant:ident,
        builder = $builder:ident($builder_name:literal, $shape:ident),
        seed = $seed:expr $(,)?
    ) => {
        $crate::bindings::valuations::typed_macros::instrument_envelope_methods!(
            @impl $ty, $name, $variant, $builder, $builder_name, $shape, $seed
        );
        $crate::bindings::valuations::typed_macros::pricing_override_methods!(
            $ty, $name, $builder, $builder_name, $shape
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
        @impl $ty:ident, $name:literal, $variant:ident, $builder:ident,
        $builder_name:literal, $shape:ident, $seed:expr
    ) => {
        #[pymethods]
        impl $ty {
            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".builder"))]
            #[staticmethod]
            #[pyo3(text_signature = "()")]
            fn builder() -> $builder {
                $crate::bindings::valuations::typed_macros::instrument_envelope_methods!(
                    @new_builder $shape, $builder, $seed
                )
            }

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".__reduce__"))]
            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                $crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".from_json"))]
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> PyResult<Self> {
                $crate::bindings::valuations::convert::parse_typed_instrument_json(json).map(|inner| Self { inner })
            }

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".to_json"))]
            #[pyo3(text_signature = "($self)")]
            fn to_json(&self) -> PyResult<String> {
                $crate::bindings::valuations::convert::serialize_typed_instrument_json(
                    finstack_quant_valuations::instruments::json_loader::InstrumentJson::from(
                        self.inner.clone(),
                    ),
                    stringify!($variant),
                )
            }

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".id"))]
            #[getter]
            fn id(&self) -> String {
                self.inner.id.to_string()
            }
        }

        #[pymethods]
        impl $builder {
            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($builder_name, ".build"))]
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
/// Arguments: the Python instrument wrapper (`inner` is the Rust instrument)
/// and its Python class name, the Python builder wrapper
/// (`inner: Option<RustBuilder>`) and its Python class name, and `fields` when
/// the builder records set fields for `__repr__` in a `fields` vector
/// (`no_fields` otherwise). Docstrings come from the instruments stub.
macro_rules! pricing_override_methods {
    (@record fields, $slf:ident, $name:literal) => {
        $slf.fields.push(($name, "{...}".to_string()));
    };
    (@record no_fields, $slf:ident, $name:literal) => {};
    ($instrument:ty, $name:literal, $builder:ty, $builder_name:literal, $record:ident) => {
        #[pyo3::pymethods]
        impl $instrument {
            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".instrument_pricing_overrides"))]
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

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".metric_pricing_overrides"))]
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

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($name, ".scenario_pricing_overrides"))]
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
            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($builder_name, ".instrument_pricing_overrides"))]
            #[pyo3(text_signature = "($self, value)")]
            fn instrument_pricing_overrides<'py>(
                mut slf: pyo3::PyRefMut<'py, Self>,
                py: pyo3::Python<'py>,
                value: &pyo3::Bound<'py, pyo3::PyAny>,
            ) -> pyo3::PyResult<pyo3::PyRefMut<'py, Self>> {
                let overrides: finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides =
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

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($builder_name, ".metric_pricing_overrides"))]
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

            #[doc = $crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", concat!($builder_name, ".scenario_pricing_overrides"))]
            #[pyo3(text_signature = "($self, value)")]
            fn scenario_pricing_overrides<'py>(
                mut slf: pyo3::PyRefMut<'py, Self>,
                py: pyo3::Python<'py>,
                value: &pyo3::Bound<'py, pyo3::PyAny>,
            ) -> pyo3::PyResult<pyo3::PyRefMut<'py, Self>> {
                let overrides: finstack_quant_valuations::instruments::pricing_overrides::ScenarioPricingOverrides =
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
