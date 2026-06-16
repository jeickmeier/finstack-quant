//! Shared JSON (de)serialization helpers for portfolio PyO3 bindings.
//!
//! Portfolio result and spec types round-trip through serde JSON in many
//! `#[pyclass]` wrappers. Centralizing the error mapping keeps binding code
//! to type conversion + a single Rust call.

use crate::errors::display_to_py;
use pyo3::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Deserialize a JSON string into a portfolio type.
pub(crate) fn deserialize_json<T: DeserializeOwned>(json: &str) -> PyResult<T> {
    serde_json::from_str(json).map_err(display_to_py)
}

/// Serialize a portfolio type to a compact JSON string.
pub(crate) fn serialize_json<T: Serialize>(value: &T) -> PyResult<String> {
    serde_json::to_string(value).map_err(display_to_py)
}
