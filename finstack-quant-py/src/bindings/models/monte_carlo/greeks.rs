//! Python bindings for Monte Carlo Greek estimators.
//!
//! Thin wrappers around the canonical GBM European finite-difference
//! convenience entry points in `finstack_quant_models::monte_carlo::greeks::gbm_european`.

use super::engine::extract_optional_currency;
use super::results::PyEstimate;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_models::monte_carlo::greeks::gbm_european::{self, GbmEuropeanFdSpec};
use finstack_quant_models::types::OptionType;
use pyo3::prelude::*;

#[allow(clippy::too_many_arguments)]
fn spec_from_args(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<GbmEuropeanFdSpec> {
    Ok(GbmEuropeanFdSpec {
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        num_steps,
        bump_size,
        option_type: OptionType::from(is_call),
        currency: extract_optional_currency(currency)?,
    })
}

#[doc = pydoc!("finstack_quant.models.monte_carlo._greeks", "finite_diff_delta")]
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    spot, strike, rate, div_yield, vol, expiry, is_call,
    num_paths=None, seed=None, num_steps=None,
    bump_size=None, currency=None,
))]
fn finite_diff_delta(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyEstimate> {
    let spec = spec_from_args(
        spot, strike, rate, div_yield, vol, expiry, is_call, num_paths, seed, num_steps, bump_size,
        currency,
    )?;
    py.detach(|| gbm_european::finite_diff_delta(spec))
        .map(PyEstimate::from_inner)
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.monte_carlo._greeks", "finite_diff_gamma")]
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    spot, strike, rate, div_yield, vol, expiry, is_call,
    num_paths=None, seed=None, num_steps=None,
    bump_size=None, currency=None,
))]
fn finite_diff_gamma(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyEstimate> {
    let spec = spec_from_args(
        spot, strike, rate, div_yield, vol, expiry, is_call, num_paths, seed, num_steps, bump_size,
        currency,
    )?;
    py.detach(|| gbm_european::finite_diff_gamma(spec))
        .map(PyEstimate::from_inner)
        .map_err(core_to_py)
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(finite_diff_delta, m)?)?;
    m.add_function(wrap_pyfunction!(finite_diff_gamma, m)?)?;
    Ok(())
}
