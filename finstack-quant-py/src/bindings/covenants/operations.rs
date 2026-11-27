//! Covenant JSON validators and engine evaluation Python entry points.

use super::engine;
use pyo3::prelude::*;
use pyo3::types::PyDict;

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

/// Evaluate a covenant engine JSON document against a metric mapping.
///
/// # Arguments
///
/// * `engine_json` - Serialized covenant engine (`CovenantEngine.to_json()`
///   or a hand-written document; only `specs` is required)
/// * `metrics` - `dict[str, float]` mapping metric id to
///   value; ratios in turns, amounts in the engine's reporting currency. A
///   dict reaches Rust as numbers (`evaluate_engine_with_source`), so a
///   non-finite value fails only when a covenant reads it
/// * `as_of` - Evaluation date, either a date-like object (`datetime.date`,
///   `pandas.Timestamp`) or an ISO 8601 string
///
/// # Returns
///
/// A dict mapping the covenant instance key (label) to a typed
/// `CovenantReport`, in spec order.
///
/// # Errors
///
/// Raises `KeyError` when a required metric is missing and `ValueError` when
/// the engine document or a metric value is invalid.
#[pyfunction]
#[pyo3(text_signature = "(engine_json, metrics, as_of)")]
pub(super) fn evaluate_engine<'py>(
    py: Python<'py>,
    engine_json: &str,
    metrics: &Bound<'py, PyAny>,
    as_of: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let reports = {
        let source = finstack_quant_covenants::HashMapMetricSource::from_pairs(
            engine::extract_metric_dict(metrics)?,
        );
        py.detach(|| {
            finstack_quant_covenants::evaluate_engine_with_source(engine_json, &source, &as_of)
        })
    }
    .map_err(crate::errors::core_to_py)?;
    engine::reports_to_pydict(py, reports)
}
