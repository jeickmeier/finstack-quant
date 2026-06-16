//! Typed Python builders for [`finstack_quant_scenarios::OperationSpec`] and supporting enums.
//!
//! This module replaces the raw-JSON authoring path with typed classmethod
//! constructors. Each classmethod constructs the Rust enum variant directly and
//! delegates serialization to serde, so the JSON wire format is guaranteed to
//! match what [`finstack_quant_scenarios::ScenarioSpec`] deserializes.
//!
//! # Round-trip strategy
//!
//! - Python builders take Python-native arguments (strings, lists, floats).
//! - The classmethod converts those into the Rust enum/struct types directly.
//! - `to_json` and `from_json` use `serde_json` on the underlying Rust types,
//!   so the wire contract follows the serde attributes on the Rust types
//!   (notably `#[serde(tag = "kind", rename_all = "snake_case")]` on
//!   `OperationSpec` and `rename = "forward"` / `rename = "par_cds"` on the
//!   `CurveKind` variants).

mod helpers;
mod kinds;
mod rate_binding;
mod spec;

pub use kinds::{PyCompounding, PyCurveKind, PyTenorMatchMode, PyTimeRollMode, PyVolSurfaceKind};
pub use rate_binding::PyRateBindingSpec;
pub use spec::PyOperationSpec;

use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `OperationSpec` and supporting enums on the scenarios submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCurveKind>()?;
    m.add_class::<PyVolSurfaceKind>()?;
    m.add_class::<PyTenorMatchMode>()?;
    m.add_class::<PyTimeRollMode>()?;
    m.add_class::<PyCompounding>()?;
    m.add_class::<PyRateBindingSpec>()?;
    m.add_class::<PyOperationSpec>()?;
    Ok(())
}
