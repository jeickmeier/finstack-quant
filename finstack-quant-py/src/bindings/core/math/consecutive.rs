//! Python binding for `finstack_quant_core::math::consecutive`.

use finstack_quant_core::math::consecutive;
use pyo3::prelude::*;

/// Count longest consecutive run of positive values.
#[pyfunction]
pub fn count_consecutive(values: Vec<f64>) -> usize {
    consecutive::count_consecutive(&values, |x| x > 0.0)
}

/// Register consecutive helpers on the parent math module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(count_consecutive, m)?)?;
    Ok(())
}
