//! Shared utilities for registering Python submodules.
//!
//! Every binding submodule needs to:
//!
//! 1. Call `parent.add_submodule(&m)?` so attribute access works.
//! 2. Set `m.__package__` to the fully-qualified dotted path.
//! 3. Insert `m` into `sys.modules` under the qualified name so `import
//!    finstack_quant.x.y` resolves correctly (matters for re-export shims, the
//!    importlib machinery, and tools like `inspect.getmodule`).
//!
//! Callers choose whether the parent's qualified path comes from `__package__`
//! or `__name__`. That keeps historical behavior explicit while avoiding two
//! near-identical registration helpers.

use pyo3::prelude::*;
use serde_json::Value;

/// Canonical qualified name of the public Python package root.
pub(crate) const ROOT_PACKAGE: &str = "finstack_quant";

/// Which parent attribute should be used to derive a child module's qualified
/// dotted path.
pub(crate) enum ParentNameSource {
    /// Derive from `parent.__package__`.
    Package,
    /// Derive from `parent.__name__`.
    Name,
}

/// Register `submodule` under `parent`, deriving the qualified path from the
/// selected parent attribute and falling back to `parent_default` when the
/// attribute is missing or unreadable.
pub(crate) fn register_submodule(
    py: Python<'_>,
    parent: &Bound<'_, PyModule>,
    submodule: &Bound<'_, PyModule>,
    submod_name: &str,
    parent_default: &str,
    source: ParentNameSource,
) -> PyResult<()> {
    let qual = submodule_name(parent, submod_name, parent_default, source);
    submodule.setattr("__package__", &qual)?;
    register_submodule_at(py, parent, submodule, &qual)
}

/// Set `submodule.__package__` before registering its children, returning the
/// qualified module path that should later be used for `sys.modules`.
///
/// Some modules need their qualified package name before they can call nested
/// `register` functions, because those children derive their own paths from
/// the parent's `__package__`.
pub(crate) fn set_submodule_package_by_package(
    parent: &Bound<'_, PyModule>,
    submodule: &Bound<'_, PyModule>,
    submod_name: &str,
    parent_default_pkg: &str,
) -> PyResult<String> {
    set_submodule_package(
        parent,
        submodule,
        submod_name,
        parent_default_pkg,
        ParentNameSource::Package,
    )
}

/// Set `submodule.__package__` using the selected parent-name source.
pub(crate) fn set_submodule_package(
    parent: &Bound<'_, PyModule>,
    submodule: &Bound<'_, PyModule>,
    submod_name: &str,
    parent_default: &str,
    source: ParentNameSource,
) -> PyResult<String> {
    let qual = submodule_name(parent, submod_name, parent_default, source);
    submodule.setattr("__package__", &qual)?;
    Ok(qual)
}

/// Attach `submodule` to `parent` and register it in `sys.modules` at `qual`.
pub(crate) fn register_submodule_at(
    py: Python<'_>,
    parent: &Bound<'_, PyModule>,
    submodule: &Bound<'_, PyModule>,
    qual: &str,
) -> PyResult<()> {
    parent.add_submodule(submodule)?;
    submodule.setattr("__package__", qual)?;
    let sys = PyModule::import(py, "sys")?;
    sys.getattr("modules")?.set_item(qual, submodule)?;
    Ok(())
}

fn submodule_name(
    parent: &Bound<'_, PyModule>,
    submod_name: &str,
    parent_default: &str,
    source: ParentNameSource,
) -> String {
    let parent_name = parent_qualified_name(parent, parent_default, source);
    format!("{parent_name}.{submod_name}")
}

/// Derive a module's qualified parent path from the selected Python attribute.
pub(crate) fn parent_qualified_name(
    parent: &Bound<'_, PyModule>,
    parent_default: &str,
    source: ParentNameSource,
) -> String {
    let attr_name = match source {
        ParentNameSource::Package => "__package__",
        ParentNameSource::Name => "__name__",
    };
    parent
        .getattr(attr_name)
        .ok()
        .and_then(|v| v.extract::<String>().ok())
        .unwrap_or_else(|| parent_default.to_string())
}

/// Convert a Python object (e.g. dict or string) to a `serde_json::Value`.
///
/// A Python `str` is first parsed as JSON; when that fails it is treated as
/// a **bare string value** (`serde_json::Value::String`). This is what
/// externally-tagged serde enums expect for unit variants — e.g. the
/// documented `attribute_pnl(..., method="Parallel")` form, which previously
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
