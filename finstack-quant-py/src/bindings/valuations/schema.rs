//! Python bindings for `finstack_quant_valuations::schema`.
//!
//! Every schema exposed here is compiled into the extension module by the
//! canonical Rust accessors (`include_str!` + `OnceLock`), so a schema read
//! from Python always describes the exact wire format the installed wheel
//! accepts. There is no loose data file that can drift from the binary.

use crate::bindings::schema_registry::schema_registry_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use serde_json::Value;

use finstack_quant_valuations::schema as canonical;

use crate::errors::{core_to_py, serde_json_to_py};

/// Docstring for the `finstack_quant.valuations.schema` Python namespace.
const MODULE_DOC: &str = r#"Compiled-in JSON Schemas for the valuations wire format.

The instrument envelope, the per-type instrument schemas, and the valuation
result schema are embedded in the extension module, so they always match the
installed version and cannot drift from the wheel that ships them.

Each accessor returns JSON *text*; parse it with ``json.loads`` or hand it
straight to a validator such as ``jsonschema.Draft202012Validator``.

Examples
--------
>>> import json
>>> from finstack_quant.valuations import schema
>>> json.loads(schema.instrument_envelope_schema())["title"]
'Finstack Quant Instrument'
"#;

/// Render a canonical schema value as pretty-printed JSON text.
fn schema_json(value: &Value) -> PyResult<String> {
    serde_json::to_string_pretty(value)
        .map_err(|err| serde_json_to_py(err, "failed to serialize schema"))
}

/// Decode a caller-supplied payload before handing it to canonical validation.
fn parse_instance(instrument_json: &str) -> PyResult<Value> {
    serde_json::from_str(instrument_json)
        .map_err(|err| serde_json_to_py(err, "invalid instrument JSON"))
}

/// Validate `instance` against one rendered schema, raising on any failure.
///
/// Routes through [`finstack_quant::schema::validate_document`] rather than the
/// crate-local validator so the two mistakes a generated payload makes most
/// often — a decimal sent as a JSON number, and a wrong enum spelling — are
/// reported at the offending field with the accepted values enumerated, instead
/// of as one `oneOf` failure against the enclosing subtree.
fn ensure_valid_against(schema: &Value, instance: &Value, context: &str) -> PyResult<()> {
    let failures =
        finstack_quant::schema::validate_document(schema, instance).map_err(core_to_py)?;
    if failures.is_empty() {
        return Ok(());
    }
    let rendered: Vec<String> = failures
        .iter()
        .map(|failure| {
            if failure.pointer.is_empty() {
                failure.message.clone()
            } else {
                format!("{}: {}", failure.pointer, failure.message)
            }
        })
        .collect();
    Err(pyo3::exceptions::PyValueError::new_err(format!(
        "{context} validation failed with {} error(s):\n  {}",
        rendered.len(),
        rendered.join("\n  ")
    )))
}

/// Return the JSON Schema for the canonical instrument envelope.
///
/// The envelope is the ``finstack_quant.instrument/1`` wrapper carrying a
/// ``type`` discriminator alongside the matching typed ``spec`` payload. Use
/// it to validate, document, or generate forms for any instrument payload
/// without knowing its concrete type up front.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the instrument envelope. The
///     document is large (roughly one megabyte); read it once and cache the
///     parsed result rather than calling this inside a loop.
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
/// >>> from finstack_quant.valuations import schema
/// >>> json.loads(schema.instrument_envelope_schema())["title"]
/// 'Finstack Quant Instrument'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn instrument_envelope_schema() -> PyResult<String> {
    schema_json(canonical::instrument_envelope_schema().map_err(core_to_py)?)
}

/// Return every canonical instrument discriminator, in registry order.
///
/// The tagged-JSON registry is the single source of truth for decoding and
/// schema generation, so this is the authoritative set of values accepted in
/// an envelope's ``instrument.type`` field.
///
/// Returns
/// -------
/// list of str
///     Registered instrument type tags, for example ``"bond"`` and
///     ``"interest_rate_swap"``. Every entry resolves through
///     :func:`instrument_schema`.
///
/// Raises
/// ------
/// ValueError
///     If the compiled-in instrument registry cannot be read.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations import schema
/// >>> "bond" in schema.instrument_types()
/// True
#[pyfunction]
#[pyo3(text_signature = "()")]
fn instrument_types() -> PyResult<Vec<String>> {
    canonical::instrument_types().map_err(core_to_py)
}

/// Return the dedicated JSON Schema for one instrument type.
///
/// Parameters
/// ----------
/// instrument_type : str
///     Canonical registry discriminator such as ``"bond"`` or
///     ``"interest_rate_swap"``. Call :func:`instrument_types` for the
///     complete set of valid values.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for that instrument type, including at
///     least one worked ``examples`` entry.
///
/// Raises
/// ------
/// KeyError
///     If ``instrument_type`` is not a registered discriminator. Call
///     :func:`instrument_types` for the valid set.
/// ValueError
///     If the compiled-in schema for that type is malformed.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.valuations import schema
/// >>> json.loads(schema.instrument_schema("bond"))["title"]
/// 'bond'
#[pyfunction]
#[pyo3(text_signature = "(instrument_type)")]
fn instrument_schema(instrument_type: &str) -> PyResult<String> {
    schema_json(&canonical::instrument_schema(instrument_type).map_err(core_to_py)?)
}

/// Return the JSON Schema for a serialized ``ValuationResult``.
///
/// This is the shape of the pricing output envelope: present value, currency,
/// measures, and policy stamps.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text for the valuation result envelope.
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
/// >>> from finstack_quant.valuations import schema
/// >>> json.loads(schema.valuation_result_schema())["title"]
/// 'Valuation Result'
#[pyfunction]
#[pyo3(text_signature = "()")]
fn valuation_result_schema() -> PyResult<String> {
    schema_json(canonical::valuation_result_schema().map_err(core_to_py)?)
}

/// Validate an instrument envelope payload against the canonical schemas.
///
/// Both the envelope schema and the schema selected by ``instrument.type`` are
/// applied, exactly as the Rust loader does when it decodes tagged instrument
/// JSON.
///
/// Parameters
/// ----------
/// instrument_json : str
///     JSON text of a ``finstack_quant.instrument/1`` envelope. Pass
///     ``json.dumps(payload)`` when starting from a Python dictionary.
///
/// Returns
/// -------
/// str
///     Canonical compact JSON of the validated payload. A failure raises
///     rather than returning a falsy value, so the individual schema
///     violations are never discarded.
///
/// Raises
/// ------
/// ValueError
///     If ``instrument_json`` is not valid JSON, or if it violates the
///     envelope or the selected type schema. The message enumerates every
///     violation with its instance path.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.valuations import schema
/// >>> example = json.loads(schema.instrument_schema("bond"))["examples"][0]
/// >>> json.loads(schema.validate_instrument_envelope_json(json.dumps(example)))["schema"]
/// 'finstack_quant.instrument/1'
#[pyfunction]
#[pyo3(text_signature = "(instrument_json)")]
fn validate_instrument_envelope_json(instrument_json: &str) -> PyResult<String> {
    let instance = parse_instance(instrument_json)?;
    let envelope = canonical::instrument_envelope_schema().map_err(core_to_py)?;
    ensure_valid_against(envelope, &instance, "instrument envelope")?;

    let instrument_type = instance
        .pointer("/instrument/type")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "instrument envelope validation passed but instrument.type is missing".to_string(),
            )
        })?
        .to_string();
    validate_instrument_type_json(&instrument_type, instrument_json)
}

/// Validate a payload against one specific instrument type's schema.
///
/// Unlike :func:`validate_instrument_envelope_json`, the type is chosen by the
/// caller rather than read from the payload, which is what you want when
/// checking that a draft payload conforms to an intended instrument type.
///
/// Parameters
/// ----------
/// instrument_type : str
///     Canonical registry discriminator whose schema is used for validation.
///     Call :func:`instrument_types` for the complete set of valid values.
/// instrument_json : str
///     JSON text to validate against that type schema. Pass
///     ``json.dumps(payload)`` when starting from a Python dictionary.
///
/// Returns
/// -------
/// str
///     Canonical compact JSON of the validated payload. A failure raises
///     rather than returning a falsy value, so the individual schema
///     violations are never discarded.
///
/// Raises
/// ------
/// KeyError
///     If ``instrument_type`` is not a registered discriminator.
/// ValueError
///     If ``instrument_json`` is not valid JSON, or if it violates the
///     selected type schema.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.valuations import schema
/// >>> example = json.loads(schema.instrument_schema("bond"))["examples"][0]
/// >>> json.loads(schema.validate_instrument_type_json("bond", json.dumps(example)))["schema"]
/// 'finstack_quant.instrument/1'
#[pyfunction]
#[pyo3(text_signature = "(instrument_type, instrument_json)")]
fn validate_instrument_type_json(instrument_type: &str, instrument_json: &str) -> PyResult<String> {
    let instance = parse_instance(instrument_json)?;
    let schema = canonical::instrument_schema(instrument_type).map_err(core_to_py)?;
    ensure_valid_against(&schema, &instance, instrument_type)?;
    canonical_json(&instance)
}

/// Re-serialize a validated instance as canonical compact JSON.
fn canonical_json(instance: &Value) -> PyResult<String> {
    serde_json::to_string(instance).map_err(|err| {
        crate::errors::value_error(format!("failed to canonicalize instrument JSON: {err}"))
    })
}

schema_registry_functions!(
    finstack_quant_valuations::schema::artifacts_slice(),
    "finstack_quant.valuations.schema"
);

/// Register the `finstack_quant.valuations.schema` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;
    add_registry_functions(&m)?;

    m.add_function(wrap_pyfunction!(instrument_envelope_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(instrument_schema, &m)?)?;
    m.add_function(wrap_pyfunction!(instrument_types, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_instrument_envelope_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_instrument_type_json, &m)?)?;
    m.add_function(wrap_pyfunction!(valuation_result_schema, &m)?)?;

    let exports = [
        "get",
        "index",
        "instrument_envelope_schema",
        "instrument_schema",
        "instrument_types",
        "validate",
        "validate_instrument_envelope_json",
        "validate_instrument_type_json",
        "valuation_result_schema",
    ];
    for name in exports {
        m.getattr(name)?
            .setattr("__module__", "finstack_quant.valuations.schema")?;
    }

    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    // Explicit public path: unlike its sibling subpackages this module has no
    // pure-Python shim, so it owns `finstack_quant.valuations.schema` itself.
    // Deriving from the parent's `__package__` would put it on the extension's
    // private path, where `import finstack_quant.valuations.schema` cannot see it.
    crate::bindings::module_utils::register_submodule_at(
        py,
        parent,
        &m,
        "finstack_quant.valuations.schema",
    )?;

    Ok(())
}
