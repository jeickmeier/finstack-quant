//! Python bindings for `finstack_quant_statements::schema`.
//!
//! The canonical Rust accessors embed each checked-in schema with
//! `include_str!` and cache it in a `OnceLock`, so a schema read from Python
//! always describes the exact wire format the installed wheel accepts.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use serde_json::Value;

use finstack_quant_statements::schema as canonical;

use crate::errors::{serde_json_to_py, statements_to_py};

/// Docstring for the `finstack_quant.statements.schema` Python namespace.
const MODULE_DOC: &str = r#"Compiled-in JSON Schemas for the statements wire format.

The model specification, the evaluated statement result, and the EBITDA
normalization configuration schemas are embedded in the extension module, so
they always match the installed version and cannot drift from the wheel that
ships them.

Each accessor returns JSON *text*; parse it with ``json.loads`` or hand it
straight to a validator such as ``jsonschema.Draft202012Validator``.

Examples
--------
>>> import json
>>> from finstack_quant.statements import schema
>>> json.loads(schema.financial_model_spec_schema())["title"]
'FinancialModelSpec'
"#;

/// Render a canonical schema value as pretty-printed JSON text.
fn schema_json(value: &Value) -> PyResult<String> {
    serde_json::to_string_pretty(value)
        .map_err(|err| serde_json_to_py(err, "failed to serialize schema"))
}

/// Return the JSON Schema for a serialized ``FinancialModelSpec``.
///
/// This is the contract for a statements model definition: periods, nodes,
/// forecast specifications, and formulas.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the financial model specification.
///
/// Raises
/// ------
/// ValueError
///     If the compiled-in schema is malformed or cannot be serialized back to
///     JSON text.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.statements import schema
/// >>> json.loads(schema.financial_model_spec_schema())["title"]
/// 'FinancialModelSpec'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn financial_model_spec_schema() -> PyResult<String> {
    schema_json(canonical::financial_model_spec_schema().map_err(statements_to_py)?)
}

/// Return the JSON Schema for a serialized ``NormalizationConfig``.
///
/// This is the contract for EBITDA normalization: the adjustment items, their
/// sign conventions, and the add-back policy applied during normalization.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the normalization configuration.
///
/// Raises
/// ------
/// ValueError
///     If the compiled-in schema is malformed or cannot be serialized back to
///     JSON text.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.statements import schema
/// >>> json.loads(schema.normalization_config_schema())["title"]
/// 'NormalizationConfig'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn normalization_config_schema() -> PyResult<String> {
    schema_json(canonical::normalization_config_schema().map_err(statements_to_py)?)
}

/// Return the JSON Schema for a serialized ``StatementResult``.
///
/// This describes per-period node values and execution statistics: numeric
/// mode, node and period counts, optional elapsed time, parallel execution,
/// and evaluation warnings. Monetary node maps retain amounts and currencies.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the statement result envelope.
///
/// Raises
/// ------
/// ValueError
///     If the compiled-in schema is malformed or cannot be serialized back to
///     JSON text.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.statements import schema
/// >>> json.loads(schema.statement_result_schema())["title"]
/// 'StatementResult'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn statement_result_schema() -> PyResult<String> {
    schema_json(canonical::statement_result_schema().map_err(statements_to_py)?)
}

schema_registry_functions!(finstack_quant_statements::schema::ARTIFACTS);

/// Register the `finstack_quant.statements.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;
    add_registry_functions(&m)?;

    m.add_function(wrap_pyfunction!(financial_model_spec_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(normalization_config_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(statement_result_schema, &m)?)?;

    let exports = [
        "financial_model_spec_schema",
        "get",
        "index",
        "normalization_config_schema",
        "statement_result_schema",
        "validate",
    ];

    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
