//! Python bindings for `finstack_quant::schema`, the whole-workspace registry.
//!
//! The ten per-domain `finstack_quant.<domain>.schema` namespaces each list
//! only their own crate's contracts, which means a caller wanting to advertise
//! every contract has to already know the domain list. This namespace is the
//! one that does not: it merges all ten registries and labels every row with
//! the domain that owns it.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::errors::core_to_py;

/// Docstring for the `finstack_quant.schema` Python namespace.
const MODULE_DOC: &str = r#"Every JSON Schema the workspace publishes, across all ten domains.

The per-domain namespaces (``finstack_quant.valuations.schema`` and friends)
list one crate each. This namespace merges all of them, so a service exposing
these contracts does not have to hard-code the domain list.

Each :func:`index` row carries ``domain`` alongside ``path``, ``$id``,
``title``, ``summary``, ``bytes`` and ``kind``.

Examples
--------
>>> import json
>>> from finstack_quant import schema
>>> index = json.loads(schema.index())
>>> index["schema_index_version"]
1
>>> sorted({row["domain"] for row in index["artifacts"]})[:3]
['attribution', 'calibration', 'cashflows']
"#;

/// List every JSON Schema the workspace publishes, across all ten domains.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON with an ``artifacts`` array. Each row carries
///     ``domain`` (the owning crate namespace), ``path``, ``$id``, ``title``,
///     ``summary``, ``bytes`` and ``kind`` (``input`` for documents you author,
///     ``output`` for documents the library emits, ``component`` for shared
///     definitions). Rows are sorted by ``domain`` then ``path``.
///
/// Raises
/// ------
/// ValueError
///     If an artifact cannot be rendered.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant import schema
/// >>> index = json.loads(schema.index())
/// >>> len(index["artifacts"]) > 100
/// True
#[pyfunction]
#[pyo3(text_signature = "()")]
fn index() -> PyResult<String> {
    let value = finstack_quant::schema::index().map_err(core_to_py)?;
    serde_json::to_string_pretty(&value).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("serialize index: {error}"))
    })
}

/// Fetch one JSON Schema by path, ``$id``, or filename, from any domain.
///
/// Parameters
/// ----------
/// selector : str
///     A ``path`` or ``$id`` from :func:`index`, or just the trailing filename
///     such as ``"bond.schema.json"``. Filename matching is anchored to a path
///     separator, so ``"bond.schema.json"`` never resolves to
///     ``convertible_bond.schema.json``.
/// profile : str, optional
///     ``"canonical"`` (default) returns the published contract, which is what
///     :func:`validate` checks against. ``"llm"`` returns the projection:
///     self-contained, unit enums flattened, Rust prose stripped, and shaped
///     for structured-output subsets. The projection is deliberately **not** a
///     validator.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON Schema text.
///
/// Raises
/// ------
/// KeyError
///     If no artifact matches ``selector``.
/// ValueError
///     If ``profile`` is unknown, or the artifact cannot be rendered.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant import schema
/// >>> json.loads(schema.get("bond.schema.json"))["title"]
/// 'bond'
#[pyfunction]
#[pyo3(signature = (selector, profile = "canonical"), text_signature = "(selector, profile='canonical')")]
fn get(selector: &str, profile: &str) -> PyResult<String> {
    let artifact = finstack_quant::schema::find(selector)
        .map_err(|error| pyo3::exceptions::PyKeyError::new_err(error.to_string()))?;
    let value = finstack_quant::schema::render_profile(artifact, profile)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    serde_json::to_string_pretty(&value).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("serialize schema: {error}"))
    })
}

/// Validate a JSON payload against one published schema, from any domain.
///
/// Union failures are reported at the offending field rather than at the
/// enclosing ``oneOf``, and unit-enum mismatches list the accepted spellings.
///
/// Parameters
/// ----------
/// selector : str
///     A ``path`` or ``$id`` from :func:`index`, or just the trailing filename.
/// payload : str
///     JSON text to check.
///
/// Returns
/// -------
/// str
///     Pretty-printed JSON array of failures, each with ``pointer`` (a JSON
///     Pointer into the payload) and ``message``. An empty array means the
///     payload validates.
///
/// Raises
/// ------
/// KeyError
///     If no artifact matches ``selector``.
/// ValueError
///     If ``payload`` is not valid JSON, or the schema cannot be built.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant import schema
/// >>> json.loads(schema.validate("bond.schema.json", "{}")) != []
/// True
#[pyfunction]
#[pyo3(text_signature = "(selector, payload)")]
fn validate(selector: &str, payload: &str) -> PyResult<String> {
    let artifact = finstack_quant::schema::find(selector)
        .map_err(|error| pyo3::exceptions::PyKeyError::new_err(error.to_string()))?;
    let parsed: serde_json::Value = serde_json::from_str(payload).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("payload is not JSON: {error}"))
    })?;
    let failures = finstack_quant::schema::validate(artifact, &parsed).map_err(core_to_py)?;
    let report = finstack_quant::schema::failures_to_value(&failures);
    serde_json::to_string_pretty(&report).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("serialize report: {error}"))
    })
}

/// List the domain namespaces that publish schemas.
///
/// Returns
/// -------
/// list of str
///     Sorted domain names, each of which is also a ``domain`` value in
///     :func:`index` and a ``finstack_quant.<domain>.schema`` namespace.
///
/// Raises
/// ------
/// ValueError
///     If the registry cannot be read.
///
/// Examples
/// --------
/// >>> from finstack_quant import schema
/// >>> "valuations" in schema.domains()
/// True
#[pyfunction]
#[pyo3(text_signature = "()")]
fn domains() -> PyResult<Vec<String>> {
    let mut names: Vec<String> = finstack_quant::schema::artifacts_with_domain()
        .into_iter()
        .map(|(domain, _)| domain.to_string())
        .collect();
    names.sort_unstable();
    names.dedup();
    Ok(names)
}

/// Register the `finstack_quant.schema` Python namespace.
///
/// # Errors
///
/// Returns a `PyErr` if any function cannot be registered.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "schema")?;
    m.setattr("__doc__", MODULE_DOC)?;

    m.add_function(wrap_pyfunction!(index, &m)?)?;
    m.add_function(wrap_pyfunction!(get, &m)?)?;
    m.add_function(wrap_pyfunction!(validate, &m)?)?;
    m.add_function(wrap_pyfunction!(domains, &m)?)?;

    let exports = ["domains", "get", "index", "validate"];
    for name in exports {
        m.getattr(name)?
            .setattr("__module__", "finstack_quant.schema")?;
    }
    m.setattr("__all__", PyList::new(py, exports)?)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "schema",
        "finstack_quant",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
