//! Python bindings for `finstack_quant_valuations::schema`.
//!
//! Every schema exposed here is compiled into the extension module by the
//! canonical Rust accessors (`include_str!` + `OnceLock`), so a schema read
//! from Python always describes the exact wire format the installed wheel
//! accepts. There is no loose data file that can drift from the binary.

use crate::bindings::pydoc::pydoc;
use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use serde_json::Value;

use finstack_quant_valuations::schema as canonical;

use crate::errors::{core_to_py, serde_json_to_py};

/// Render a canonical schema value as pretty-printed JSON text.
fn schema_json(value: &Value) -> PyResult<String> {
    serde_json::to_string_pretty(value)
        .map_err(|err| serde_json_to_py(err, "failed to serialize schema"))
}

/// Decode a caller-supplied payload before handing it to canonical validation.
fn parse_instance(instrument_json: &str) -> PyResult<Value> {
    serde_json::from_str(instrument_json)
        .map_err(|err| serde_json_to_py(err, "invalid instrument JSON"))
}

#[doc = pydoc!("finstack_quant.valuations.schema", "instrument_envelope_schema")]
#[pyfunction]
#[pyo3(text_signature = "()")]
fn instrument_envelope_schema() -> PyResult<String> {
    schema_json(canonical::instrument_envelope_schema().map_err(core_to_py)?)
}

#[doc = pydoc!("finstack_quant.valuations.schema", "instrument_types")]
#[pyfunction]
#[pyo3(text_signature = "()")]
fn instrument_types() -> Vec<&'static str> {
    canonical::instrument_types().to_vec()
}

#[doc = pydoc!("finstack_quant.valuations.schema", "instrument_schema")]
#[pyfunction]
#[pyo3(text_signature = "(instrument_type)")]
fn instrument_schema(instrument_type: &str) -> PyResult<String> {
    schema_json(&canonical::instrument_schema(instrument_type).map_err(core_to_py)?)
}

#[doc = pydoc!("finstack_quant.valuations.schema", "valuation_result_schema")]
#[pyfunction]
#[pyo3(text_signature = "()")]
fn valuation_result_schema() -> PyResult<String> {
    schema_json(canonical::valuation_result_schema().map_err(core_to_py)?)
}

#[doc = pydoc!("finstack_quant.valuations.schema", "validate_instrument_envelope_json")]
#[pyfunction]
#[pyo3(text_signature = "(instrument_json)")]
fn validate_instrument_envelope_json(instrument_json: &Bound<'_, PyAny>) -> PyResult<String> {
    let instrument_json = crate::bindings::extract::envelope_json_text(instrument_json)?;
    let instance = parse_instance(&instrument_json)?;
    canonical::validate_instrument_envelope_json(&instance).map_err(core_to_py)?;
    canonical_json(&instance)
}

#[doc = pydoc!("finstack_quant.valuations.schema", "validate_instrument_type_json")]
#[pyfunction]
#[pyo3(text_signature = "(instrument_type, instrument_json)")]
fn validate_instrument_type_json(
    instrument_type: &str,
    instrument_json: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let instrument_json = crate::bindings::extract::envelope_json_text(instrument_json)?;
    let instance = parse_instance(&instrument_json)?;
    canonical::validate_instrument_type_json(instrument_type, &instance).map_err(core_to_py)?;
    canonical_json(&instance)
}

/// Re-serialize a validated instance as canonical compact JSON.
fn canonical_json(instance: &Value) -> PyResult<String> {
    serde_json::to_string(instance).map_err(|err| {
        crate::errors::value_error(format!("failed to canonicalize instrument JSON: {err}"))
    })
}

schema_registry_functions!(
    finstack_quant_valuations::schema::artifacts_slice(),
    "finstack_quant.valuations.schema"
);

/// Register the `finstack_quant.valuations.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "schema")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.valuations.schema", "__module__"),
    )?;
    add_registry_functions(&m)?;

    m.add_function(wrap_pyfunction!(instrument_envelope_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(instrument_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(instrument_types, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_instrument_envelope_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_instrument_type_json, &m)?)?;
    m.add_function(wrap_pyfunction!(valuation_result_schema, &m)?)?;

    let exports = [
        "get",
        "index",
        "instrument_envelope_schema",
        "instrument_schema",
        "instrument_types",
        "validate",
        "validate_instrument_envelope_json",
        "validate_instrument_type_json",
        "valuation_result_schema",
    ];

    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    // No pure-Python shim: the compiled module owns `finstack_quant.valuations.schema`.
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
