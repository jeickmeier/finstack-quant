//! Python bindings for `finstack_quant_calibration`'s schema registry.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Docstring for the `finstack_quant.calibration.schema` namespace.
const MODULE_DOC: &str = "Compiled-in JSON Schemas for calibration envelopes and raw market quotes.\n\nUse `index()` to list the calibration crate's contracts, `get(selector)` to fetch one, and `validate(selector, payload)` for pointer-precise validation.\n\nExamples\n--------\n>>> import json\n>>> from finstack_quant.calibration import schema\n>>> json.loads(schema.get(\"calibration.schema.json\"))[\"$schema\"]\n'https://json-schema.org/draft/2020-12/schema'\n";

schema_registry_functions!(finstack_quant_calibration::json_schema::artifacts());

/// Register the `finstack_quant.calibration.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;
    add_registry_functions(&m)?;
    m.setattr("__all__", PyList::new(py, ["get", "index", "validate"])?)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}
