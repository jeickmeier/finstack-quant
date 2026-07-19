//! Shared JSON (de)serialization helpers for portfolio PyO3 bindings.
//!
//! Portfolio result and spec types round-trip through serde JSON in many
//! `#[pyclass]` wrappers. Centralizing the error mapping keeps binding code
//! to type conversion + a single Rust call.

use crate::errors::display_to_py;
use pyo3::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Deserialize a JSON string into a portfolio type while releasing the GIL.
pub(crate) fn deserialize_json<T: DeserializeOwned + Send>(json: &str) -> PyResult<T> {
    let json = json.to_owned();
    Python::attach(|py| py.detach(move || serde_json::from_str(&json))).map_err(display_to_py)
}

/// Serialize a portfolio type to a compact JSON string while releasing the GIL.
pub(crate) fn serialize_json<T: Serialize + Sync>(value: &T) -> PyResult<String> {
    Python::attach(|py| py.detach(|| serde_json::to_string(value))).map_err(display_to_py)
}
