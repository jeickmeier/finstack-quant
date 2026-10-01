//! Python bindings for `finstack_quant_cashflows::schema`.
//!
//! The cashflows crate does not publish one schema per accessor. It publishes
//! a single `resources()` call returning `(canonical URI, jsonschema::Resource)`
//! pairs for every embedded cashflow schema, which the valuations
//! validator feeds to its `$ref` resolver. The binding below preserves that
//! shape as a `dict[str, str]` keyed by the same canonical URI.
//!
//! `package_cashflow_schema` is deliberately not bound: it is `#[doc(hidden)]`,
//! it mutates a schema in place for the schema-generator binary, and it is not
//! a schema accessor.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};
use serde_json::Value;

use finstack_quant_cashflows::schema as canonical;

use crate::errors::{core_to_py, serde_json_to_py};

/// Docstring for the `finstack_quant.cashflows.schema` Python namespace.
const MODULE_DOC: &str = r#"Compiled-in JSON Schemas for the cashflow wire format.

Every cashflow schema (the amortization, coupon, default, fee, prepayment,
recovery and schedule specifications, plus the schedule, build-spec, accrual
and aggregation contracts) is embedded in the extension module, so they always
match the installed version and cannot drift from the wheel that ships them.
These are the documents that resolve the
``https://finstack_quant.dev/schemas/cashflow/1/...`` references appearing
inside instrument schemas.

Examples
--------
>>> import json
>>> from finstack_quant.cashflows import schema
>>> resources = schema.resources()
>>> base = "https://finstack_quant.dev/schemas/cashflow/1/"
>>> json.loads(resources[base + "schedule_params.schema.json"])["title"]
'ScheduleParams'
"#;

/// Render a canonical schema value as pretty-printed JSON text.
fn schema_json(value: &Value) -> PyResult<String> {
    serde_json::to_string_pretty(value)
        .map_err(|err| serde_json_to_py(err, "failed to serialize schema"))
}

/// Return every embedded cashflow schema, keyed by canonical URI.
///
/// The keys are the published ``$id`` URIs used by ``$ref`` targets in
/// instrument and cashflow payload schemas, so the mapping can be handed
/// directly to a resolver-aware validator.
///
/// Returns
/// -------
/// dict of str to str
///     Mapping from canonical schema URI to pretty-printed JSON Schema text,
///     one entry per embedded cashflow schema.
///
/// Raises
/// ------
/// ValueError
///     If a compiled-in schema is malformed or cannot be serialized
///     back to JSON text.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows import schema
/// >>> sorted(uri.rsplit("/", 1)[-1] for uri in schema.resources())[:2]
/// ['accrual_config.schema.json', 'amortization_spec.schema.json']
#[pyfunction]
#[pyo3(text_signature = "()")]
fn resources<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let mapping = PyDict::new(py);
    for (uri, resource) in canonical::resources().map_err(core_to_py)? {
        mapping.set_item(uri, schema_json(resource.contents())?)?;
    }
    Ok(mapping)
}

schema_registry_functions!(
    finstack_quant_cashflows::schema::ARTIFACTS,
    "finstack_quant.cashflows.schema"
);

/// Register the `finstack_quant.cashflows.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;
    add_registry_functions(&m)?;

    m.add_function(wrap_pyfunction!(resources, &m)?)?;

    let exports = ["get", "index", "resources", "validate"];
    for name in exports {
        m.getattr(name)?
            .setattr("__module__", "finstack_quant.cashflows.schema")?;
    }

    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "schema",
        "finstack_quant.cashflows",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
