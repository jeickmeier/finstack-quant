//! Python bindings for the canonical statements schema registry.

use crate::bindings::pydoc::pydoc;
use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

schema_registry_functions!(
    finstack_quant_statements::schema::ARTIFACTS,
    "finstack_quant.statements.schema"
);

/// Register the statements schema namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "schema")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.statements.schema", "__module__"),
    )?;
    add_registry_functions(&m)?;
    m.setattr("__all__", PyList::new(py, ["get", "index", "validate"])?)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}
