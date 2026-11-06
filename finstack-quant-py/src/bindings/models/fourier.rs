//! Fourier pricing method bindings (COS).
//!
//! Exposes model-agnostic Fourier pricers for European options under the
//! Black-Scholes, Variance Gamma, and Merton jump-diffusion models. Each
//! binding wraps a characteristic function (from `finstack-quant-core`) with
//! the Fang-Oosterlee (2008) COS method from `finstack-quant-valuations`.
//!
//! A Lewis (2001) single-integral binding (`bs_lewis_price`) was
//! removed because the underlying implementation was known-divergent
//! off-ATM. Use `bs_cos_price` for all Black-Scholes Fourier pricing.

use crate::bindings::module_utils::spec_from_py;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_models::fourier::cos::{
    CosMarketParams, bs_cos_price as rust_bs_cos_price,
    merton_jump_cos_price as rust_merton_jump_cos_price, vg_cos_price as rust_vg_cos_price,
};
use pyo3::prelude::*;

/// Parse the `market` argument (dict or JSON text) as a [`CosMarketParams`].
fn cos_market(market: &Bound<'_, PyAny>) -> PyResult<CosMarketParams> {
    spec_from_py(market.py(), market, "CosMarketParams")
}

#[doc = pydoc!("finstack_quant.models._fourier", "bs_cos_price")]
#[pyfunction]
#[pyo3(signature = (market, vol))]
fn bs_cos_price(py: Python<'_>, market: &Bound<'_, PyAny>, vol: f64) -> PyResult<f64> {
    let market = cos_market(market)?;
    py.detach(move || rust_bs_cos_price(market, vol).map_err(core_to_py))
}

#[doc = pydoc!("finstack_quant.models._fourier", "vg_cos_price")]
#[pyfunction]
#[pyo3(signature = (market, sigma, theta, nu))]
fn vg_cos_price(
    py: Python<'_>,
    market: &Bound<'_, PyAny>,
    sigma: f64,
    theta: f64,
    nu: f64,
) -> PyResult<f64> {
    let market = cos_market(market)?;
    py.detach(move || rust_vg_cos_price(market, sigma, theta, nu).map_err(core_to_py))
}

#[doc = pydoc!("finstack_quant.models._fourier", "merton_jump_cos_price")]
#[pyfunction]
#[pyo3(signature = (market, sigma, mu_jump, sigma_jump, lambda_))]
fn merton_jump_cos_price(
    py: Python<'_>,
    market: &Bound<'_, PyAny>,
    sigma: f64,
    mu_jump: f64,
    sigma_jump: f64,
    lambda_: f64,
) -> PyResult<f64> {
    let market = cos_market(market)?;
    py.detach(move || {
        rust_merton_jump_cos_price(market, sigma, mu_jump, sigma_jump, lambda_).map_err(core_to_py)
    })
}

/// Register Fourier pricing functions on the models submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(bs_cos_price, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(vg_cos_price, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(merton_jump_cos_price, m)?)?;
    Ok(())
}
