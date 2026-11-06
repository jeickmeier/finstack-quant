//! Python bindings for Monte Carlo Greek estimators.
//!
//! Thin wrappers around the canonical GBM European finite-difference
//! convenience entry points in `finstack_quant_models::monte_carlo::greeks::gbm_european`.

use super::results::PyEstimate;
use crate::bindings::module_utils::spec_from_py;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_models::monte_carlo::greeks::gbm_european::{self, GbmEuropeanFdSpec};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.monte_carlo._greeks", "finite_diff_delta")]
#[pyfunction]
#[pyo3(signature = (spec))]
fn finite_diff_delta(py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<PyEstimate> {
    let spec: GbmEuropeanFdSpec = spec_from_py(py, spec, "GbmEuropeanFdSpec")?;
    py.detach(|| gbm_european::finite_diff_delta(spec))
        .map(PyEstimate::from_inner)
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.monte_carlo._greeks", "finite_diff_gamma")]
#[pyfunction]
#[pyo3(signature = (spec))]
fn finite_diff_gamma(py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<PyEstimate> {
    let spec: GbmEuropeanFdSpec = spec_from_py(py, spec, "GbmEuropeanFdSpec")?;
    py.detach(|| gbm_european::finite_diff_gamma(spec))
        .map(PyEstimate::from_inner)
        .map_err(core_to_py)
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(finite_diff_delta, m)?)?;
    m.add_function(wrap_pyfunction!(finite_diff_gamma, m)?)?;
    Ok(())
}
