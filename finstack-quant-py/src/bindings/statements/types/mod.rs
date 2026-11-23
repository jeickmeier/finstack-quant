//! Python wrappers for statement model types and enums.

mod forecast;
mod model;

pub use forecast::{PyForecastMethod, PyForecastSpec};
pub use model::{PyFinancialModelSpec, PyNodeId, PyNodeSpec, PyNodeType, PyNumericMode};
use pyo3::prelude::*;

/// Register type classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyForecastMethod>()?;
    m.add_class::<PyForecastSpec>()?;
    m.add_class::<PyNodeType>()?;
    m.add_class::<PyNodeId>()?;
    m.add_class::<PyNodeSpec>()?;
    m.add_class::<PyNumericMode>()?;
    m.add_class::<PyFinancialModelSpec>()?;
    Ok(())
}
