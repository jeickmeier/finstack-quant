//! Python bindings for `finstack_quant_margin::schema`.
//!
//! Schemas are rendered from the crate's registry on demand, so a schema read
//! from Python always describes the exact wire format the installed wheel
//! accepts and the extension module carries no copy of the checked-in JSON.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Docstring for the `finstack_quant.margin.schema` Python namespace.
const MODULE_DOC: &str = "Compiled-in JSON Schema for the margin wire format.\n\nOne artifact with three closed root shapes: an OTC margin spec, a CSA spec,\nor a margin call.\n\nUse `index()` to see what this crate publishes, `get(selector)` to fetch one\nschema, and `validate(selector, payload)` to check a payload and get back the\nJSON Pointer of anything that failed.\n\nExamples\n--------\n>>> import json\n>>> from finstack_quant.margin import schema\n>>> json.loads(schema.get(\"margin.schema.json\"))[\"$schema\"]\n'https://json-schema.org/draft/2020-12/schema'\n";

schema_registry_functions!(finstack_quant_margin::schema::ARTIFACTS);

/// Register the `finstack_quant.margin.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;
    add_registry_functions(&m)?;

    let all = PyList::new(py, ["get", "index", "validate"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
