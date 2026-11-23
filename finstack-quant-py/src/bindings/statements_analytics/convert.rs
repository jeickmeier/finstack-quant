//! Internal serde-enum and repr formatting helpers for statements-analytics bindings.

use pyo3::prelude::*;

/// Convert between a Python enum wrapper and its Rust twin through their
/// shared serde representation.
///
/// Both sides derive serde with the same `rename_all`, so one round-trip
/// replaces a hand-written `to_rust` / `from_rust` match table per enum.
pub(crate) fn enum_convert<A, B>(value: &A) -> PyResult<B>
where
    A: serde::Serialize,
    B: serde::de::DeserializeOwned,
{
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .map_err(|e| crate::errors::serde_json_to_py(e, "enum conversion"))
}

/// Python-style rendering of a bool for `__repr__`.
pub(crate) fn py_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// Python-style rendering of an optional string for `__repr__`.
pub(crate) fn py_opt_str(value: Option<&str>) -> String {
    match value {
        Some(s) => format!("'{s}'"),
        None => "None".to_string(),
    }
}
