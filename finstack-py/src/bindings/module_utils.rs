//! Shared utilities for registering Python submodules.
//!
//! Every binding submodule needs to:
//!
//! 1. Call `parent.add_submodule(&m)?` so attribute access works.
//! 2. Set `m.__package__` to the fully-qualified dotted path.
//! 3. Insert `m` into `sys.modules` under the qualified name so `import
//!    finstack.x.y` resolves correctly (matters for re-export shims, the
//!    importlib machinery, and tools like `inspect.getmodule`).
//!
//! Two flavors are provided. They differ only in how the parent's qualified
//! name is obtained — kept separate to preserve the historical behavior at
//! each call site without changing observable semantics.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::Value;

/// Register `submodule` under `parent`, deriving the qualified path from the
/// parent's `__package__` attribute and falling back to
/// `parent_default_pkg` when the attribute is missing or unreadable.
///
/// Used by submodules nested several layers deep (e.g. `finstack.core.math`,
/// `finstack.core.market_data`) where the parent already has a stable
/// `__package__` set by its own `register`.
pub(crate) fn register_submodule_by_package(
    py: Python<'_>,
    parent: &Bound<'_, PyModule>,
    submodule: &Bound<'_, PyModule>,
    submod_name: &str,
    parent_default_pkg: &str,
) -> PyResult<()> {
    let qual =
        set_submodule_package_by_package(parent, submodule, submod_name, parent_default_pkg)?;
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
    let qual = submodule_name_by_package(parent, submod_name, parent_default_pkg);
    submodule.setattr("__package__", &qual)?;
    Ok(qual)
}

/// Register `submodule` under `parent`, deriving the qualified path from the
/// parent's `__name__` attribute and falling back to `parent_default_name`
/// when the attribute is missing or unreadable.
///
/// Used by top-level domain modules (e.g. `finstack.analytics`,
/// `finstack.portfolio`) where parent is the root `finstack` module.
pub(crate) fn register_submodule_by_parent_name(
    py: Python<'_>,
    parent: &Bound<'_, PyModule>,
    submodule: &Bound<'_, PyModule>,
    submod_name: &str,
    parent_default_name: &str,
) -> PyResult<()> {
    let parent_name: String = parent
        .getattr("__name__")
        .ok()
        .and_then(|v| v.extract::<String>().ok())
        .unwrap_or_else(|| parent_default_name.to_string());
    let qual = format!("{parent_name}.{submod_name}");
    register_submodule_at(py, parent, submodule, &qual)
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

fn submodule_name_by_package(
    parent: &Bound<'_, PyModule>,
    submod_name: &str,
    parent_default_pkg: &str,
) -> String {
    let pkg: String = parent
        .getattr("__package__")
        .ok()
        .and_then(|v| v.extract::<String>().ok())
        .unwrap_or_else(|| parent_default_pkg.to_string());
    format!("{pkg}.{submod_name}")
}

/// Convert a Python object (e.g. dict or string) to a `serde_json::Value`.
pub(crate) fn py_to_json_value<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<Value> {
    if let Ok(json) = obj.extract::<String>() {
        return serde_json::from_str(&json)
            .map_err(|e| PyValueError::new_err(format!("invalid {label} JSON: {e}")));
    }

    let json_mod = py.import("json")?;
    let json: String = json_mod
        .call_method1("dumps", (obj,))
        .and_then(|value| value.extract())
        .map_err(|e| PyValueError::new_err(format!("invalid {label}: {e}")))?;
    serde_json::from_str(&json)
        .map_err(|e| PyValueError::new_err(format!("invalid {label} JSON: {e}")))
}
