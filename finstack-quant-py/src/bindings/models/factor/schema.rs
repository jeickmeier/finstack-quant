//! Python bindings for `finstack_quant_factor::schema`.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Docstring for the `finstack_quant.models.factor.schema` Python namespace.
const MODULE_DOC: &str = r#"Compiled-in JSON Schemas for the factor-model wire format.

Examples
--------
>>> from finstack_quant.models.factor import schema
>>> schema.get("credit_factor_model")["title"]
'CreditFactorModel'
"#;

schema_registry_functions!(
    finstack_quant_factor::schema::ARTIFACTS,
    "finstack_quant.models.factor.schema"
);

/// Register the `finstack_quant.models.factor.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;
    add_registry_functions(&m)?;

    let exports = ["get", "index", "validate"];

    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    // Unlike its sibling subpackages this module has no pure-Python shim, so the
    // compiled module itself owns `finstack_quant.models.factor.schema`.
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
