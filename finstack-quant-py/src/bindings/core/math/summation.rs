//! Python bindings for `finstack_quant_core::math::summation`.

use finstack_quant_core::math::summation;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Kahan compensated summation — reduces floating-point rounding errors.
///
/// Best for sequences where all values have the same sign.
/// For mixed-sign values, prefer :func:`neumaier_sum`.
#[pyfunction]
#[pyo3(text_signature = "(values)")]
fn kahan_sum(values: &Bound<'_, PyAny>) -> PyResult<f64> {
    let values = crate::bindings::dataframe_utils::extract_f64_vec(values, "values")?;
    summation::kahan_sum(values).map_err(crate::errors::core_to_py)
}

/// Neumaier compensated summation — handles mixed-sign values better than Kahan.
///
/// Recommended for financial calculations with mixed-sign cashflows.
#[pyfunction]
#[pyo3(text_signature = "(values)")]
fn neumaier_sum(values: &Bound<'_, PyAny>) -> PyResult<f64> {
    let values = crate::bindings::dataframe_utils::extract_f64_vec(values, "values")?;
    summation::neumaier_sum(values).map_err(crate::errors::core_to_py)
}

/// Build the `finstack_quant.core.math.summation` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "summation")?;
    m.setattr(
        "__doc__",
        "Numerically stable summation: Kahan and Neumaier compensated sums.",
    )?;

    m.add_function(wrap_pyfunction!(kahan_sum, &m)?)?;
    m.add_function(wrap_pyfunction!(neumaier_sum, &m)?)?;

    let all = PyList::new(py, ["kahan_sum", "neumaier_sum"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
