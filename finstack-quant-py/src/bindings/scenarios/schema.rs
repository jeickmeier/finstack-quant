//! Python bindings for `finstack_quant_scenarios::schema`.
//!
//! Schemas are rendered from the crate's registry on demand, so a schema read
//! from Python always describes the exact wire format the installed wheel
//! accepts and the extension module carries no copy of the checked-in JSON.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Docstring for the `finstack_quant.scenarios.schema` Python namespace.
const MODULE_DOC: &str = "Compiled-in JSON Schema for the scenario wire format.\n\nOne artifact: the ordered shock and roll operations applied to market,\nstatement and valuation targets.\n\nUse `index()` to see what this crate publishes, `get(selector)` to fetch one\nschema, and `validate(selector, payload)` to check a payload and get back the\nJSON Pointer of anything that failed.\n\nExamples\n--------\n>>> import json\n>>> from finstack_quant.scenarios import schema\n>>> json.loads(schema.get(\"scenario.schema.json\"))[\"$schema\"]\n'https://json-schema.org/draft/2020-12/schema'\n";

schema_registry_functions!(finstack_quant_scenarios::schema::ARTIFACTS);

/// Register the `finstack_quant.scenarios.schema` Python namespace.
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
