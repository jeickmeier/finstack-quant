//! Shared `#[pymethods]` generators for serde-backed wrapper types.
//!
//! The crate enables PyO3's `multiple-pymethods`, so a block generated here
//! sits alongside the wrapper's hand-written `#[pymethods]` block.

/// Generate `to_json` / `from_json` / `__reduce__` for a wrapper holding
/// `pub(crate) inner: T` where `T: Serialize + DeserializeOwned`.
///
/// `wire_methods!(PyType, RustType, "Name")` deserializes straight into the
/// wrapper. Append `validate` when the Rust type's invariants are checked by
/// an inherent `validate()` returning a `finstack_quant_core` error rather
/// than by its `Deserialize` impl.
macro_rules! wire_methods {
    ($py_type:ident, $rust_type:ty, $name:literal) => {
        $crate::bindings::macros::wire_methods!(@impl $py_type, $rust_type, $name, |inner| inner);
    };
    ($py_type:ident, $rust_type:ty, $name:literal, validate) => {
        $crate::bindings::macros::wire_methods!(@impl $py_type, $rust_type, $name, |inner| {
            inner.validate().map_err($crate::errors::core_to_py)?;
            inner
        });
    };
    (@impl $py_type:ident, $rust_type:ty, $name:literal, |$inner:ident| $checked:expr) => {
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
                $crate::bindings::json_bridge::serialize_json_with_context(
                    &self.inner,
                    concat!("failed to serialize ", $name),
                )
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
                let $inner: $rust_type =
                    $crate::bindings::json_bridge::deserialize_json_with_context(
                        json,
                        concat!("invalid ", $name, " JSON"),
                    )?;
                Ok(Self { inner: $checked })
            }

            /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``)
            /// through the same strict serde round-trip as ``to_json`` / ``from_json``.
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

/// Generate `_repr_html_` delegating to the wrapper's `to_dataframe`.
macro_rules! impl_repr_html_via_dataframe {
    ($py_ty:ident) => {
        #[::pyo3::pymethods]
        impl $py_ty {
            /// Render as an HTML table in Jupyter notebooks.
            ///
            /// Delegates to the frame from ``to_dataframe``, so pandas' own
            /// row/column truncation applies; returns ``None`` if the frame
            /// cannot be built so IPython falls back to ``__repr__``.
            fn _repr_html_(&self, py: ::pyo3::Python<'_>) -> Option<String> {
                use ::pyo3::types::PyAnyMethods;
                let frame = self.to_dataframe(py).ok()?;
                frame.call_method0("_repr_html_").ok()?.extract().ok()
            }
        }
    };
}
pub(crate) use impl_repr_html_via_dataframe;
