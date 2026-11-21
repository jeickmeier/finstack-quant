//! Shared JSON (de)serialization helpers for PyO3 bindings.
//!
//! Result and spec types round-trip through serde JSON in many
//! `#[pyclass]` wrappers. Centralizing the error mapping keeps binding code
//! to type conversion + a single Rust call.

use crate::errors::{display_to_py, serde_json_to_py};
use pyo3::prelude::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Deserialize a JSON string into a Rust type while releasing the GIL.
pub(crate) fn deserialize_json<T: DeserializeOwned + Send>(json: &str) -> PyResult<T> {
    let json = json.to_owned();
    Python::attach(|py| py.detach(move || serde_json::from_str(&json))).map_err(display_to_py)
}

/// Serialize a Rust type to a compact JSON string while releasing the GIL.
pub(crate) fn serialize_json<T: Serialize + Sync>(value: &T) -> PyResult<String> {
    Python::attach(|py| py.detach(|| serde_json::to_string(value))).map_err(display_to_py)
}

/// Deserialize a JSON string while releasing the GIL, prefixing a parse
/// failure with `context` (`"{context}: {serde error}"`).
pub(crate) fn deserialize_json_with_context<T: DeserializeOwned + Send>(
    json: &str,
    context: &str,
) -> PyResult<T> {
    Python::attach(|py| py.detach(|| serde_json::from_str(json)))
        .map_err(|err| serde_json_to_py(err, context))
}

/// Serialize a Rust type to compact JSON while releasing the GIL, prefixing a
/// failure with `context` (`"{context}: {serde error}"`).
pub(crate) fn serialize_json_with_context<T: Serialize + Sync>(
    value: &T,
    context: &str,
) -> PyResult<String> {
    Python::attach(|py| py.detach(|| serde_json::to_string(value)))
        .map_err(|err| serde_json_to_py(err, context))
}

/// Convert any `Serialize` value into a Python object via its canonical JSON
/// shape.
///
/// This is the shared escape hatch behind `to_dict()`-style accessors and
/// typed getters that surface nested serde structures (for example
/// [`finstack_quant_core::config::ResultsMeta`]) without hand-writing a
/// `PyDict` per field.
///
/// # Arguments
///
/// * `value` - Any serde-serializable value.
///
/// # Errors
///
/// Returns a `ValueError` if the value cannot be serialized, or propagates a
/// failure from Python's `json.loads`.
pub(crate) fn serde_to_py<'py, T>(py: Python<'py>, value: &T) -> PyResult<Bound<'py, PyAny>>
where
    T: Serialize,
{
    let json = serde_json::to_string(value).map_err(display_to_py)?;
    py.import("json")?.call_method1("loads", (json,))
}
