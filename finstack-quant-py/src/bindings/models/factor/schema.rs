//! Python bindings for `finstack_quant_models::factor::schema`.
//!
//! The canonical Rust accessors embed each checked-in schema with
//! `include_str!` and cache it in a `OnceLock`, so a schema read from Python
//! always describes the exact wire format the installed wheel accepts.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use serde_json::Value;

use finstack_quant_models::factor::schema as canonical;

use crate::errors::{core_to_py, serde_json_to_py};

/// Docstring for the `finstack_quant.models.factor.schema` Python namespace.
const MODULE_DOC: &str = r#"Compiled-in JSON Schemas for the factor-model wire format.

The generic factor-model configuration and the three credit artifacts
(hierarchy model, calibration configuration, calibration inputs) are embedded
in the extension module, so they always match the installed version and cannot
drift from the wheel that ships them.

Each accessor returns JSON *text*; parse it with ``json.loads`` or hand it
straight to a validator such as ``jsonschema.Draft202012Validator``.

Examples
--------
>>> import json
>>> from finstack_quant.models.factor import schema
>>> json.loads(schema.credit_factor_model_schema())["title"]
'CreditFactorModel'
"#;

/// Render a canonical schema value as pretty-printed JSON text.
fn schema_json(value: &Value) -> PyResult<String> {
    serde_json::to_string_pretty(value)
        .map_err(|err| serde_json_to_py(err, "failed to serialize schema"))
}

/// Return the JSON Schema for a serialized ``CreditCalibrationConfig``.
///
/// This is the contract for the deterministic credit factor-model calibrator:
/// estimation windows, shrinkage settings, and solver controls.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the credit calibration
///     configuration.
///
/// Raises
/// ------
/// RuntimeError
///     If the compiled-in schema is malformed JSON.
/// ValueError
///     If the parsed schema cannot be serialized back to JSON text.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.models.factor import schema
/// >>> json.loads(schema.credit_calibration_config_schema())["title"]
/// 'CreditCalibrationConfig'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn credit_calibration_config_schema() -> PyResult<String> {
    schema_json(canonical::credit_calibration_config_schema().map_err(core_to_py)?)
}

/// Return the JSON Schema for a serialized ``CreditCalibrationInputs``.
///
/// This is the contract for one calibration run's inputs: typed issuer
/// histories, tags, generic factor series, the anchor date, and any overrides.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the credit calibration inputs.
///
/// Raises
/// ------
/// RuntimeError
///     If the compiled-in schema is malformed JSON.
/// ValueError
///     If the parsed schema cannot be serialized back to JSON text.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.models.factor import schema
/// >>> json.loads(schema.credit_calibration_inputs_schema())["title"]
/// 'CreditCalibrationInputs'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn credit_calibration_inputs_schema() -> PyResult<String> {
    schema_json(canonical::credit_calibration_inputs_schema().map_err(core_to_py)?)
}

/// Return the JSON Schema for a serialized ``CreditFactorModel``.
///
/// This is the fully self-contained credit factor hierarchy artifact accepted
/// by ``finstack_quant.models.factor.credit.CreditFactorModel.from_json``.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the credit factor hierarchy model.
///
/// Raises
/// ------
/// RuntimeError
///     If the compiled-in schema is malformed JSON.
/// ValueError
///     If the parsed schema cannot be serialized back to JSON text.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.models.factor import schema
/// >>> json.loads(schema.credit_factor_model_schema())["title"]
/// 'CreditFactorModel'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn credit_factor_model_schema() -> PyResult<String> {
    schema_json(canonical::credit_factor_model_schema().map_err(core_to_py)?)
}

/// Return the JSON Schema for a versioned factor-model configuration.
///
/// This is the contract for the generic (non-credit) factor-model envelope:
/// typed factors, covariance settings, matching rules, pricing, and risk
/// configuration.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the factor-model configuration
///     envelope.
///
/// Raises
/// ------
/// RuntimeError
///     If the compiled-in schema is malformed JSON.
/// ValueError
///     If the parsed schema cannot be serialized back to JSON text.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.models.factor import schema
/// >>> json.loads(schema.factor_model_config_schema())["title"]
/// 'Finstack Quant Factor Model Configuration'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn factor_model_config_schema() -> PyResult<String> {
    schema_json(canonical::factor_model_config_schema().map_err(core_to_py)?)
}

schema_registry_functions!(finstack_quant_models::factor::schema::ARTIFACTS);

/// Register the `finstack_quant.models.factor.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;
    add_registry_functions(&m)?;

    m.add_function(wrap_pyfunction!(credit_calibration_config_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(credit_calibration_inputs_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(credit_factor_model_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(factor_model_config_schema, &m)?)?;

    let exports = [
        "credit_calibration_config_schema",
        "credit_calibration_inputs_schema",
        "credit_factor_model_schema",
        "factor_model_config_schema",
        "get",
        "index",
        "validate",
    ];

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
