//! Python bindings for `finstack_quant::schema`, the whole-workspace registry.
//!
//! The eleven per-domain `finstack_quant.<domain>.schema` namespaces each list
//! only their own crate's contracts, which means a caller wanting to advertise
//! every contract has to already know the domain list. This namespace is the
//! one that does not: it merges all eleven registries and labels every row with
//! the domain that owns it.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.schema", "index")]
#[pyfunction]
#[pyo3(text_signature = "()")]
fn index() -> PyResult<String> {
    let value = finstack_quant::schema::index().map_err(core_to_py)?;
    serde_json::to_string_pretty(&value).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("serialize index: {error}"))
    })
}

#[doc = pydoc!("finstack_quant.schema", "get")]
#[pyfunction]
#[pyo3(signature = (selector, profile = "canonical"), text_signature = "(selector, profile='canonical')")]
fn get(selector: &str, profile: &str) -> PyResult<String> {
    let artifact = finstack_quant::schema::find(selector).map_err(core_to_py)?;
    let value = finstack_quant::schema::render_profile(artifact, profile).map_err(core_to_py)?;
    serde_json::to_string_pretty(&value).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("serialize schema: {error}"))
    })
}

#[doc = pydoc!("finstack_quant.schema", "validate")]
#[pyfunction]
#[pyo3(text_signature = "(selector, payload)")]
fn validate(selector: &str, payload: &str) -> PyResult<String> {
    let artifact = finstack_quant::schema::find(selector).map_err(core_to_py)?;
    let parsed: serde_json::Value = serde_json::from_str(payload).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("payload is not JSON: {error}"))
    })?;
    let failures = finstack_quant::schema::validate(artifact, &parsed).map_err(core_to_py)?;
    serde_json::to_string_pretty(&failures).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("serialize report: {error}"))
    })
}

#[doc = pydoc!("finstack_quant.schema", "domains")]
#[pyfunction]
#[pyo3(text_signature = "()")]
fn domains() -> Vec<&'static str> {
    finstack_quant::schema::domains()
}

/// Register the `finstack_quant.schema` Python namespace.
///
/// # Errors
///
/// Returns a `PyErr` if any function cannot be registered.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "schema")?;
    m.setattr("__doc__", pydoc!("finstack_quant.schema", "__module__"))?;

    m.add_function(wrap_pyfunction!(index, &m)?)?;
    m.add_function(wrap_pyfunction!(get, &m)?)?;
    m.add_function(wrap_pyfunction!(validate, &m)?)?;
    m.add_function(wrap_pyfunction!(domains, &m)?)?;

    let exports = ["domains", "get", "index", "validate"];
    m.setattr("__all__", PyList::new(py, exports)?)?;

    // `finstack_quant/schema.py` re-exports this module and owns its import path.
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
