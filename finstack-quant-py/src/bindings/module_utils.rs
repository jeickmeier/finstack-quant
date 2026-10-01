//! Shared utilities for registering Python submodules.
//!
//! The compiled extension is installed as `finstack_quant.finstack_quant`, but
//! its submodules are part of the public `finstack_quant` tree: every compiled
//! module is named by its public import path (`finstack_quant.models.credit.lgd`),
//! never by its location inside the extension.
//!
//! [`new_submodule`] creates a module with that name. The name is derived from
//! the parent's `__package__` (the root sets it to [`ROOT_PACKAGE`]) and set at
//! creation, so `__name__`, `__package__` and the `__module__` of every function
//! added afterwards are all the public path from the start.
//!
//! [`attach_submodule`] then attaches the module to its parent and, when the
//! compiled module is itself what `import finstack_quant.x.y` should return
//! ([`Exposure::Compiled`]), gives it a `__spec__` and registers it in
//! `sys.modules` under that path. A
//! compiled module re-exported by a pure-Python package or module at the same
//! path ([`Exposure::Python`]) is not registered: the Python file owns that
//! `sys.modules` key, and registering the compiled module there would replace
//! the Python module or keep it from ever running.

use pyo3::prelude::*;
use serde_json::Value;

/// Canonical qualified name of the public Python package root.
pub(crate) const ROOT_PACKAGE: &str = "finstack_quant";

/// How the public import path of a compiled module resolves.
pub(crate) enum Exposure {
    /// The compiled module is imported directly: it is registered in
    /// `sys.modules` under its public path.
    Compiled,
    /// A pure-Python package or module at the same public path re-exports the
    /// compiled module and owns its `sys.modules` key.
    Python,
}

/// Create the compiled submodule `name` of `parent`, named by its public
/// import path.
///
/// The path is `parent.__package__` followed by `.name`; both `__name__` and
/// `__package__` of the new module are set to it, so its own submodules and
/// the functions added to it inherit the public path.
///
/// # Arguments
///
/// * `parent` - Compiled module the new module will be attached to; its
///   `__package__` must hold its public path (the root holds [`ROOT_PACKAGE`]).
/// * `name` - Attribute name of the new module under `parent`, without dots.
///
/// # Errors
///
/// Returns a `RuntimeError` when `parent.__package__` is not a string, and
/// propagates failures to create the module or set its attributes.
pub(crate) fn new_submodule<'py>(
    parent: &Bound<'py, PyModule>,
    name: &str,
) -> PyResult<Bound<'py, PyModule>> {
    let parent_path: String = parent.getattr("__package__")?.extract().map_err(|_| {
        pyo3::exceptions::PyRuntimeError::new_err(format!(
            "cannot register compiled submodule {name:?}: parent module has no __package__"
        ))
    })?;
    let path = format!("{parent_path}.{name}");
    let module = PyModule::new(parent.py(), &path)?;
    module.setattr("__package__", &path)?;
    Ok(module)
}

/// Attach `module` (created by [`new_submodule`]) to `parent` and, for
/// [`Exposure::Compiled`], register it in `sys.modules` under its public path.
///
/// A registered module also gets a `__spec__` (`ModuleSpec(path, None,
/// is_package=True)`) so `importlib.util.find_spec` resolves it; the import
/// system reads `__spec__`, not `__package__`. It is a package spec because
/// [`new_submodule`] sets `__package__` to the module's own path, and
/// `__spec__.parent` must agree with it. The loader is `None`: no loader can
/// re-create a compiled submodule on its own, so `importlib.reload` fails
/// rather than re-initialising the extension under the wrong name.
///
/// # Arguments
///
/// * `parent` - Module that receives `module` as an attribute named by the
///   last component of `module.__name__`.
/// * `module` - Compiled submodule to attach.
/// * `exposure` - Whether `import` of the public path returns this compiled
///   module ([`Exposure::Compiled`]) or a Python module that re-exports it
///   ([`Exposure::Python`]).
///
/// # Errors
///
/// Propagates failures to set the attribute, build the `__spec__`, or update
/// `sys.modules`.
pub(crate) fn attach_submodule(
    parent: &Bound<'_, PyModule>,
    module: &Bound<'_, PyModule>,
    exposure: Exposure,
) -> PyResult<()> {
    parent.add_submodule(module)?;
    if let Exposure::Compiled = exposure {
        let py = parent.py();
        let path = module.name()?;
        let kwargs = pyo3::types::PyDict::new(py);
        kwargs.set_item("is_package", true)?;
        let spec = PyModule::import(py, "importlib.machinery")?
            .getattr("ModuleSpec")?
            .call((&path, py.None()), Some(&kwargs))?;
        module.setattr("__spec__", spec)?;
        let sys = PyModule::import(py, "sys")?;
        sys.getattr("modules")?.set_item(path, module)?;
    }
    Ok(())
}

/// Convert a Python object (e.g. dict or string) to a `serde_json::Value`.
///
/// A Python `str` is first parsed as JSON; when that fails it is treated as
/// a **bare string value** (`serde_json::Value::String`). This is what
/// externally-tagged serde enums expect for unit variants — e.g. the
/// documented `attribute_pnl(..., method="parallel")` form, which previously
/// raised `ValueError: invalid method JSON` (quant review M11).
pub(crate) fn py_to_json_value<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<Value> {
    if let Ok(json) = obj.extract::<String>() {
        return Ok(serde_json::from_str(&json).unwrap_or(Value::String(json)));
    }

    let json_mod = py.import("json")?;
    let json: String = json_mod
        .call_method1("dumps", (obj,))
        .and_then(|value| value.extract())
        .map_err(|e| crate::errors::value_error(format!("invalid {label}: {e}")))?;
    serde_json::from_str(&json)
        .map_err(|e| crate::errors::value_error(format!("invalid {label} JSON: {e}")))
}

/// Serialize a Python object to a compact JSON string.
///
/// Accepts dicts/lists (via ``json.dumps``) or pre-serialized JSON strings
/// (validated, not double-encoded).
pub(crate) fn py_to_json_string<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<String> {
    let value = py_to_json_value(py, obj, label)?;
    serde_json::to_string(&value)
        .map_err(|e| crate::errors::value_error(format!("failed to serialize {label}: {e}")))
}

/// Serialize a Python object to JSON via `json.dumps`, then deserialize into `T`.
///
/// Releases the GIL for the serde step. Prefer this over re-declaring a local
/// copy per binding module: the conversion and its error shape are the same
/// everywhere, and duplicates drift.
pub(crate) fn py_to_serde<'py, T: serde::de::DeserializeOwned + Send>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<T> {
    let json_mod = py.import("json")?;
    // Typed wrappers (anything carrying ``to_dict``) serialize through their
    // canonical dict, standalone or nested inside lists and dicts.
    let to_dict = py.eval(c"lambda o: o.to_dict()", None, None)?;
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("default", to_dict)?;
    let json_str: String = json_mod
        .call_method("dumps", (obj,), Some(&kwargs))?
        .extract()?;
    py.detach(move || serde_json::from_str(&json_str))
        .map_err(|e| crate::errors::serde_json_to_py(e, &format!("invalid {label}")))
}

/// Parse an ISO-4217 code into a [`Currency`], mapping failures to `ValueError`.
pub(crate) fn parse_currency(code: &str) -> PyResult<finstack_quant_core::currency::Currency> {
    code.parse().map_err(crate::errors::display_to_py)
}
