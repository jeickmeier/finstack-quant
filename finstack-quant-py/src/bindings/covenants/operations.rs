//! Covenant JSON validator Python entry points.

use pyo3::prelude::*;

/// Validate and canonicalize a covenant spec JSON string.
///
/// # Arguments
///
/// * `spec_json` - JSON-encoded `CovenantSpec`
///
/// # Returns
///
/// Canonical JSON string (object keys sorted recursively) after validation.
///
/// # Errors
///
/// Raises `ValueError` when the spec fails schema or semantic validation.
#[pyfunction]
#[pyo3(text_signature = "(spec_json)")]
pub(super) fn validate_covenant_spec_json(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::validate_covenant_spec_json(spec_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Validate and canonicalize a covenant report JSON string.
///
/// Returns canonical JSON with object keys sorted recursively. Raises
/// `ValueError` when the report JSON is malformed.
#[pyfunction]
#[pyo3(text_signature = "(report_json)")]
pub(super) fn validate_covenant_report_json(py: Python<'_>, report_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::validate_covenant_report_json(report_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Validate and canonicalize a covenant engine JSON string.
///
/// Only `specs` is required in the document; `breach_history`, `windows` and
/// `waivers` default to empty. Returns canonical JSON with object keys sorted
/// recursively. Raises `ValueError` when the engine fails schema or semantic
/// validation.
#[pyfunction]
#[pyo3(text_signature = "(engine_json)")]
pub(super) fn validate_covenant_engine_json(py: Python<'_>, engine_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_covenants::validate_covenant_engine_json(engine_json)
            .map_err(crate::errors::core_to_py)
    })
}
