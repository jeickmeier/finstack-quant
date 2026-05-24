//! Python bindings for `finstack_core::math::summation`.

use finstack_core::math::summation;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Kahan compensated summation — reduces floating-point rounding errors.
///
/// Best for sequences where all values have the same sign.
/// For mixed-sign values, prefer :func:`neumaier_sum`.
#[pyfunction]
#[pyo3(text_signature = "(values)")]
fn kahan_sum(values: Vec<f64>) -> f64 {
    summation::kahan_sum(values)
}

/// Neumaier compensated summation — handles mixed-sign values better than Kahan.
///
/// Recommended for financial calculations with mixed-sign cashflows.
#[pyfunction]
#[pyo3(text_signature = "(values)")]
fn neumaier_sum(values: Vec<f64>) -> f64 {
    summation::neumaier_sum(values)
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

/// Build the `finstack.core.math.summation` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "summation")?;
    m.setattr(
        "__doc__",
        "Numerically stable summation: Kahan and Neumaier compensated sums.",
    )?;

    m.add_function(wrap_pyfunction!(kahan_sum, &m)?)?;
    m.add_function(wrap_pyfunction!(neumaier_sum, &m)?)?;

    let all = PyList::new(py, ["kahan_sum", "neumaier_sum"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "summation",
        "finstack.core.math",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
