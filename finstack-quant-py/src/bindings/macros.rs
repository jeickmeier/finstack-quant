//! Shared `#[pymethods]` generators for serde-backed wrapper types.
//!
//! The crate enables PyO3's `multiple-pymethods`, so a block generated here
//! sits alongside the wrapper's hand-written `#[pymethods]` block.

/// Generate `to_json` / `from_json` / `__reduce__` for a wrapper holding
/// `pub(crate) inner: T` where `T: Serialize + DeserializeOwned`.
///
/// - `wire_methods!(PyType, RustType, "Name")` deserializes straight into the
///   wrapper.
/// - Append `validate` when the Rust type's invariants are checked by an
///   inherent `validate()` returning a `finstack_quant_core` error rather than
///   by its `Deserialize` impl.
/// - Pass `module = "finstack_quant.domain"` to take the `to_json`, `from_json`
///   and `__reduce__` docstrings from that module's stubs.
/// - Append `to_dict` to also generate `to_dict`, returning the canonical
///   serde shape as a plain `dict`.
/// - Append `arc` when the wrapper holds `inner: Arc<T>`. Such types (curves,
///   surfaces) do not derive `PartialEq` in Rust, so this arm also emits
///   `__eq__` comparing the canonical JSON wire form.
/// - Append `json_only` for a [`label_enum!`] wrapper: it emits `to_json` /
///   `from_json` without `__reduce__`, because label enums pickle through
///   their class constant.
/// - Append `rust_json` when the Rust type owns its wire form through inherent
///   `from_json(&str)` / `to_json()` methods (both returning
///   `finstack_quant_core::Result`), so both directions run the Rust checks.
macro_rules! wire_methods {
    ($py_type:ident, $rust_type:ty, $name:literal, module = $module:literal) => {
        $crate::bindings::macros::wire_methods!(@documented_json $py_type, $rust_type, $name, [], |inner| inner,
            $crate::bindings::pydoc::pydoc!($module, concat!($name, ".to_json")),
            $crate::bindings::pydoc::pydoc!($module, concat!($name, ".from_json")));
        $crate::bindings::macros::wire_methods!(@reduce $py_type, $crate::bindings::pydoc::pydoc!($module, concat!($name, ".__reduce__")));
    };

    ($py_type:ident, $rust_type:ty, $name:literal, module = $module:literal, to_dict) => {
        $crate::bindings::macros::wire_methods!($py_type, $rust_type, $name, module = $module);

        #[::pyo3::pymethods]
        impl $py_type {
            #[doc = $crate::bindings::pydoc::pydoc!($module, concat!($name, ".to_dict"))]
            #[pyo3(text_signature = "($self)")]
            fn to_dict<'py>(
                &self,
                py: ::pyo3::Python<'py>,
            ) -> ::pyo3::PyResult<::pyo3::Bound<'py, ::pyo3::PyAny>> {
                $crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
            }
        }
    };

    ($py_type:ident, $rust_type:ty, $name:literal) => {
        $crate::bindings::macros::wire_methods!(@json $py_type, $rust_type, $name, [], |inner| inner);
        $crate::bindings::macros::wire_methods!(@reduce $py_type);
    };
    ($py_type:ident, $rust_type:ty, $name:literal, to_dict) => {
        $crate::bindings::macros::wire_methods!($py_type, $rust_type, $name);

        #[::pyo3::pymethods]
        impl $py_type {
            /// Return every field as a plain ``dict`` (canonical serde shape).
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the value cannot be serialized.
            #[pyo3(text_signature = "($self)")]
            fn to_dict<'py>(
                &self,
                py: ::pyo3::Python<'py>,
            ) -> ::pyo3::PyResult<::pyo3::Bound<'py, ::pyo3::PyAny>> {
                $crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
            }
        }
    };
    ($py_type:ident, $rust_type:ty, $name:literal, module = $module:literal, validate) => {
        $crate::bindings::macros::wire_methods!(@documented_json $py_type, $rust_type, $name, [], |inner| {
            inner.validate().map_err($crate::errors::core_to_py)?;
            inner
        },
            $crate::bindings::pydoc::pydoc!($module, concat!($name, ".to_json")),
            $crate::bindings::pydoc::pydoc!($module, concat!($name, ".from_json")));
        $crate::bindings::macros::wire_methods!(@reduce $py_type, $crate::bindings::pydoc::pydoc!($module, concat!($name, ".__reduce__")));
    };
    ($py_type:ident, $rust_type:ty, $name:literal, validate) => {
        $crate::bindings::macros::wire_methods!(@json $py_type, $rust_type, $name, [], |inner| {
            inner.validate().map_err($crate::errors::core_to_py)?;
            inner
        });
        $crate::bindings::macros::wire_methods!(@reduce $py_type);
    };
    ($py_type:ident, $rust_type:ty, $name:literal, arc) => {
        $crate::bindings::macros::wire_methods!(@json $py_type, $rust_type, $name, [*], |inner| {
            ::std::sync::Arc::new(inner)
        });
        $crate::bindings::macros::wire_methods!(@reduce $py_type);

        #[::pyo3::pymethods]
        impl $py_type {
            /// Structural equality via the canonical JSON wire form.
            fn __eq__(&self, other: &::pyo3::Bound<'_, ::pyo3::PyAny>) -> ::pyo3::PyResult<bool> {
                use ::pyo3::types::PyAnyMethods;
                let Ok(other) = other.extract::<::pyo3::PyRef<'_, Self>>() else {
                    return Ok(false);
                };
                Ok(self.to_json()? == other.to_json()?)
            }
        }
    };
    ($py_type:ident, $rust_type:ty, $name:literal, json_only) => {
        $crate::bindings::macros::wire_methods!(@json $py_type, $rust_type, $name, [], |inner| inner);
    };
    ($py_type:ident, $rust_type:ty, $name:literal, rust_json) => {
        #[::pyo3::pymethods]
        impl $py_type {
            #[doc = concat!(
                "Serialize this ", $name, " to its canonical JSON wire form.\n\n",
                "Returns\n-------\nstr\n    Compact JSON that ``from_json`` accepts.\n\n",
                "Raises\n------\nValueError\n    If the value cannot be serialized."
            )]
            #[allow(clippy::wrong_self_convention)]
            #[pyo3(text_signature = "($self)")]
            fn to_json(&self) -> ::pyo3::PyResult<String> {
                self.inner.to_json().map_err($crate::errors::core_to_py)
            }

            #[doc = concat!(
                "Deserialize a ", $name, " from its canonical JSON wire form.\n\n",
                "Parameters\n----------\njson : str\n    JSON produced by ``to_json`` (strict field names).\n\n",
                "Returns\n-------\n", $name, "\n    The reconstructed value.\n\n",
                "Raises\n------\nValueError\n    If the JSON is malformed, carries unknown fields, or fails validation."
            )]
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> ::pyo3::PyResult<Self> {
                Ok(Self {
                    inner: <$rust_type>::from_json(json).map_err($crate::errors::core_to_py)?,
                })
            }
        }
        $crate::bindings::macros::wire_methods!(@reduce $py_type);
    };
    (@json $py_type:ident, $rust_type:ty, $name:literal, [$($deref:tt)?], |$inner:ident| $checked:expr) => {
        $crate::bindings::macros::wire_methods!(@documented_json $py_type, $rust_type, $name, [$($deref)?], |$inner| $checked,
            concat!(
                "Serialize this ", $name, " to its canonical JSON wire form.\n\n",
                "Returns\n-------\nstr\n    Compact JSON that ``from_json`` accepts.\n\n",
                "Raises\n------\nValueError\n    If the value cannot be serialized."
            ),
            concat!(
                "Deserialize a ", $name, " from its canonical JSON wire form.\n\n",
                "Parameters\n----------\njson : str\n    JSON produced by ``to_json`` (strict field names).\n\n",
                "Returns\n-------\n", $name, "\n    The reconstructed value.\n\n",
                "Raises\n------\nValueError\n    If the JSON is malformed, carries unknown fields, or fails validation."
            ));
    };
    (@documented_json $py_type:ident, $rust_type:ty, $name:literal, [$($deref:tt)?], |$inner:ident| $checked:expr, $to_doc:expr, $from_doc:expr) => {
        #[::pyo3::pymethods]
        impl $py_type {
            #[doc = $to_doc]
            #[allow(clippy::wrong_self_convention)]
            #[pyo3(text_signature = "($self)")]
            fn to_json(&self) -> ::pyo3::PyResult<String> {
                $crate::bindings::json_bridge::serialize_json_with_context(
                    &$($deref)? self.inner,
                    concat!("failed to serialize ", $name),
                )
            }

            #[doc = $from_doc]
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> ::pyo3::PyResult<Self> {
                let $inner: $rust_type =
                    $crate::bindings::json_bridge::deserialize_json_with_context(
                        json,
                        concat!("invalid ", $name, " JSON"),
                    )?;
                Ok(Self { inner: $checked })
            }
        }
    };
    (@reduce $py_type:ident) => {
        $crate::bindings::macros::wire_methods!(@reduce $py_type, concat!(
            "Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``)\n",
            "through the same strict serde round-trip as ``to_json`` / ``from_json``."
        ));
    };
    (@reduce $py_type:ident, $doc:expr) => {
        #[::pyo3::pymethods]
        impl $py_type {
            #[doc = $doc]
            fn __reduce__<'py>(
                &self,
                py: ::pyo3::Python<'py>,
            ) -> ::pyo3::PyResult<(::pyo3::Bound<'py, ::pyo3::PyAny>, (String,))> {
                use ::pyo3::types::PyAnyMethods;
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                $crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }
        }
    };
}
pub(crate) use wire_methods;

/// Generate the class surface of an enum-like wrapper holding
/// `pub(crate) inner: T` for a `Copy + PartialEq` Rust enum `T`.
///
/// Emits one class constant per `CONST => Variant` pair, `__str__` (the
/// label), `__repr__` (`Name.CONST`) and `__reduce__`, which pickles through
/// the class constant (`getattr(Name, "CONST")`). Insert `from_name` before
/// the variant list to also emit the `from_name` classmethod parsing a label
/// through `T: FromStr`.
///
/// The label is `T`'s `Display` form unless `label = |v| expr` (with `v:
/// &T`) follows `from_name`. Equality and hashing come from the
/// `#[pyclass(eq, hash)]` attributes; type-specific methods live in the
/// wrapper's own `#[pymethods]` block.
macro_rules! label_enum {
    ($py_type:ident, $rust_type:ident, $name:literal, { $($pairs:tt)+ }) => {
        $crate::bindings::macros::label_enum!(
            @impl $py_type, $rust_type, $name, |v| ::std::string::ToString::to_string(v),
            { $($pairs)+ }
        );
    };
    ($py_type:ident, $rust_type:ident, $name:literal, from_name, { $($pairs:tt)+ }) => {
        $crate::bindings::macros::label_enum!(
            @impl $py_type, $rust_type, $name, |v| ::std::string::ToString::to_string(v),
            { $($pairs)+ }
        );
        $crate::bindings::macros::label_enum!(@from_name $py_type, $rust_type, $name);
    };
    (
        $py_type:ident, $rust_type:ident, $name:literal, from_name,
        label = |$v:ident| $label:expr, { $($pairs:tt)+ }
    ) => {
        $crate::bindings::macros::label_enum!(
            @impl $py_type, $rust_type, $name, |$v| $label, { $($pairs)+ }
        );
        $crate::bindings::macros::label_enum!(@from_name $py_type, $rust_type, $name);
    };
    (
        @impl $py_type:ident, $rust_type:ident, $name:literal, |$v:ident| $label:expr,
        { $( $(#[$meta:meta])* $const:ident => $variant:ident ),+ $(,)? }
    ) => {
        impl $py_type {
            /// Name of the class constant holding this value.
            fn const_name(&self) -> &'static str {
                [$( (stringify!($const), $rust_type::$variant) ),+]
                    .into_iter()
                    .find(|(_, variant)| *variant == self.inner)
                    .map_or("?", |(name, _)| name)
            }

            /// Label returned by `__str__` (and parsed by `from_name`).
            fn label(&self) -> String {
                let $v = &self.inner;
                $label
            }
        }

        #[::pyo3::pymethods]
        impl $py_type {
            $(
                $(#[$meta])*
                #[classattr]
                const $const: $py_type = $py_type { inner: $rust_type::$variant };
            )+

            #[doc = concat!("Return ``repr(self)``: the class constant, e.g. ``", $name, ".NAME``.")]
            fn __repr__(&self) -> String {
                format!(concat!($name, ".{}"), self.const_name())
            }

            /// Return ``str(self)``: the canonical label.
            fn __str__(&self) -> String {
                self.label()
            }

            /// Support ``pickle`` by reconstructing the class constant.
            fn __reduce__<'py>(
                &self,
                py: ::pyo3::Python<'py>,
            ) -> ::pyo3::PyResult<(
                ::pyo3::Bound<'py, ::pyo3::PyAny>,
                (::pyo3::Bound<'py, ::pyo3::types::PyType>, &'static str),
            )> {
                use ::pyo3::types::PyAnyMethods;
                let getattr = py.import("builtins")?.getattr("getattr")?;
                Ok((getattr, (py.get_type::<Self>(), self.const_name())))
            }
        }
    };
    (@from_name $py_type:ident, $rust_type:ident, $name:literal) => {
        #[::pyo3::pymethods]
        impl $py_type {
            #[doc = concat!(
                "Parse a ", $name, " from its label; the class docstring lists the accepted spellings.\n\n",
                "Parameters\n----------\nname : str\n    Label to parse.\n\n",
                "Returns\n-------\n", $name, "\n    The matching value.\n\n",
                "Raises\n------\nValueError\n    If ``name`` is not a recognised label."
            )]
            #[classmethod]
            #[pyo3(text_signature = "(cls, name)")]
            fn from_name(
                _cls: &::pyo3::Bound<'_, ::pyo3::types::PyType>,
                name: &str,
            ) -> ::pyo3::PyResult<Self> {
                name.parse::<$rust_type>()
                    .map(|inner| Self { inner })
                    .map_err($crate::errors::core_to_py)
            }
        }
    };
}
pub(crate) use label_enum;

/// Generate an extractor accepting a wrapper instance or a label string.
///
/// `typed_or_label!(vis fn name -> Inner, Wrapper, "expected text", |label| parse)`
/// returns `Wrapper.inner` for a wrapper, `parse` (a `PyResult<Inner>`) for a
/// `str`, and raises `TypeError("expected <expected text>")` otherwise.
macro_rules! typed_or_label {
    (
        $vis:vis fn $fn_name:ident -> $inner:ty, $wrapper:ty, $expected:literal,
        |$label:ident| $parse:expr
    ) => {
        #[doc = concat!("Extract a value from a wrapper instance or its label; expects ", $expected, ".")]
        $vis fn $fn_name(obj: &::pyo3::Bound<'_, ::pyo3::PyAny>) -> ::pyo3::PyResult<$inner> {
            use ::pyo3::types::{PyAnyMethods, PyStringMethods};
            if let Ok(typed) = obj.extract::<::pyo3::PyRef<'_, $wrapper>>() {
                return Ok(typed.inner);
            }
            if let Ok(text) = obj.cast::<::pyo3::types::PyString>() {
                let $label: &str = &text.to_str()?;
                return $parse;
            }
            Err(::pyo3::exceptions::PyTypeError::new_err(concat!("expected ", $expected)))
        }
    };
}
pub(crate) use typed_or_label;

/// Generate `_repr_html_` delegating to the wrapper's `to_dataframe`.
macro_rules! impl_repr_html_via_dataframe {
    ($py_ty:ident) => {
        $crate::bindings::macros::impl_repr_html_via_dataframe!(
            $py_ty,
            concat!(
                "Render as an HTML table in Jupyter notebooks.\n\n",
                "Delegates to the frame from ``to_dataframe``, so pandas' own\n",
                "row/column truncation applies; returns ``None`` if the frame\n",
                "cannot be built so IPython falls back to ``__repr__``."
            )
        );
    };
    ($py_ty:ident, $doc:expr) => {
        #[::pyo3::pymethods]
        impl $py_ty {
            #[doc = $doc]
            fn _repr_html_(&self, py: ::pyo3::Python<'_>) -> Option<String> {
                use ::pyo3::types::PyAnyMethods;
                let frame = self.to_dataframe(py).ok()?;
                frame.call_method0("_repr_html_").ok()?.extract().ok()
            }
        }
    };
}
pub(crate) use impl_repr_html_via_dataframe;

/// Generate read-only `#[getter]`s whose body is one expression.
///
/// `py_getters!(PyType |s| { /// docs \n name -> Ret = expr; ... })` expands
/// to a `#[pymethods]` block with one `#[getter] fn name(&self) -> Ret` per
/// entry. Inside `expr`, the binder (`s`) is the wrapper (`&self`), so a body
/// reads `s.inner.field`. Each entry's docstring is forwarded verbatim.
/// Getters that need `py`, a lifetime or more than one statement stay
/// hand-written.
macro_rules! py_getters {
    ($py_type:ident |$s:ident| { $( $(#[$doc:meta])* $name:ident -> $ret:ty = $body:expr; )+ }) => {
        #[::pyo3::pymethods]
        impl $py_type {
            $(
                $(#[$doc])*
                #[getter]
                fn $name(&self) -> $ret {
                    let $s = self;
                    $body
                }
            )+
        }
    };
}
pub(crate) use py_getters;
