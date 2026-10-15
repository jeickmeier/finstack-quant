//! Python bindings for `finstack_quant_core::math`.

mod linalg;
mod special_functions;
mod stats;
mod summation;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Length of the longest run of strictly positive values.
#[pyfunction]
pub fn longest_positive_run(values: Vec<f64>) -> usize {
    finstack_quant_core::math::longest_positive_run(&values)
}

/// Register the `math` submodule on the parent `core` module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "math")?;
    m.setattr(
        "__doc__",
        "Numerical helpers: linear algebra, statistics, special functions, summation.",
    )?;

    m.add_function(wrap_pyfunction!(longest_positive_run, &m)?)?;
    linalg::register(py, &m)?;
    stats::register(py, &m)?;
    special_functions::register(py, &m)?;
    summation::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "linalg",
            "longest_positive_run",
            "special_functions",
            "stats",
            "summation",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
