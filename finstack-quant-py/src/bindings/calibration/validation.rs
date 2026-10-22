//! Standalone no-arbitrage checks on a volatility surface.
//!
//! Each function converts its arguments and calls the matching
//! `finstack_quant_calibration::validation` function; a failed check raises
//! ``ValueError`` with the Rust message.

use super::config::PyValidationConfig;
use crate::bindings::core::market_data::curves::PyVolSurface;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use pyo3::prelude::*;

use finstack_quant_calibration::validation as rust_validation;

#[doc = pydoc!("finstack_quant.calibration", "validate_surface")]
#[pyfunction]
#[pyo3(text_signature = "(surface, config, forwards)")]
pub(crate) fn validate_surface(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
    forwards: Vec<f64>,
) -> PyResult<()> {
    rust_validation::validate_surface(&surface.inner, &config.inner, &forwards).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.calibration", "validate_calendar_spread")]
#[pyfunction]
#[pyo3(text_signature = "(surface, config, forwards)")]
pub(crate) fn validate_calendar_spread(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
    forwards: Vec<f64>,
) -> PyResult<()> {
    rust_validation::validate_calendar_spread(&surface.inner, &config.inner, &forwards)
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.calibration", "validate_butterfly_spread")]
#[pyfunction]
#[pyo3(text_signature = "(surface, config, forwards)")]
pub(crate) fn validate_butterfly_spread(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
    forwards: Vec<f64>,
) -> PyResult<()> {
    rust_validation::validate_butterfly_spread(&surface.inner, &config.inner, &forwards)
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.calibration", "validate_vol_bounds")]
#[pyfunction]
#[pyo3(text_signature = "(surface, config)")]
pub(crate) fn validate_vol_bounds(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
) -> PyResult<()> {
    rust_validation::validate_vol_bounds(&surface.inner, &config.inner).map_err(core_to_py)
}
