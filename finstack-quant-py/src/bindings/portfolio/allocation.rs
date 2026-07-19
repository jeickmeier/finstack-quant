//! Python bindings for lightweight strategy allocation.

use crate::errors::portfolio_to_py;
use pyo3::prelude::*;

/// Allocate strategy weights from a JSON specification.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-serialized ``WeightAllocationSpec``.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``WeightAllocationResult``.
#[pyfunction]
fn allocate_weights(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    let spec_json = spec_json.to_owned();
    py.detach(move || finstack_quant_portfolio::allocate_weights(&spec_json))
        .map_err(portfolio_to_py)
}

/// Validate a strategy allocation JSON specification.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-serialized ``WeightAllocationSpec``.
#[pyfunction]
fn validate_allocation_json(py: Python<'_>, spec_json: &str) -> PyResult<()> {
    let spec_json = spec_json.to_owned();
    py.detach(move || finstack_quant_portfolio::validate_allocation_json(&spec_json))
        .map_err(portfolio_to_py)
}

/// Register strategy allocation functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(allocate_weights, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_allocation_json, m)?)?;
    Ok(())
}
