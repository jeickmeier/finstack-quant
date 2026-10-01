//! Python bindings for `finstack_quant_core::schema`.
//!
//! Schemas are rendered from the crate's registry on demand, so a schema read
//! from Python always describes the exact wire format the installed wheel
//! accepts and the extension module carries no copy of the checked-in JSON.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Docstring for the `finstack_quant.core.schema` Python namespace.
const MODULE_DOC: &str = "Compiled-in JSON Schemas for the core wire formats.\n\nThe registry covers the persisted market context, the columnar table\nenvelope, period plans, date schedules and their specifications, and\nscorecard rating scales.\n\nUse `index()` to see what this crate publishes, `get(selector)` to fetch one\nschema, and `validate(selector, payload)` to check a payload and get back the\nJSON Pointer of anything that failed.\n\nExamples\n--------\n>>> import json\n>>> from finstack_quant.core import schema\n>>> json.loads(schema.get(\"market_context_state.schema.json\"))[\"$schema\"]\n'https://json-schema.org/draft/2020-12/schema'\n";

schema_registry_functions!(
    finstack_quant_core::schema::ARTIFACTS,
    "finstack_quant.core.schema"
);

/// Register the `finstack_quant.core.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;
    add_registry_functions(&m)?;

    let all = PyList::new(py, ["get", "index", "validate"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "schema",
        "finstack_quant.core",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
