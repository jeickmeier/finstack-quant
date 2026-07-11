//! Python bindings for `finstack_quant_core::math`.

mod consecutive;
mod linalg;
mod special_functions;
mod stats;
mod summation;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `math` submodule on the parent `core` module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "math")?;
    m.setattr(
        "__doc__",
        "Numerical helpers: linear algebra, statistics, special functions, summation.",
    )?;

    let qual = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &m,
        "math",
        "finstack_quant.core",
    )?;

    consecutive::register(py, &m)?;
    linalg::register(py, &m)?;
    stats::register(py, &m)?;
    special_functions::register(py, &m)?;
    summation::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "count_consecutive",
            "linalg",
            "stats",
            "special_functions",
            "summation",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;

    Ok(())
}
