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

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_models::fourier::cos::{
    bs_cos_price as rust_bs_cos_price, merton_jump_cos_price as rust_merton_jump_cos_price,
    vg_cos_price as rust_vg_cos_price, CosMarketParams,
};
use finstack_quant_models::types::OptionType;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models._fourier", "bs_cos_price")]
#[pyfunction]
#[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, is_call, n_terms=None))]
#[allow(clippy::too_many_arguments)]
fn bs_cos_price(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> PyResult<f64> {
    py.detach(move || {
        rust_bs_cos_price(
            CosMarketParams {
                spot,
                strike,
                rate,
                div_yield,
                expiry,
                option_type: OptionType::from(is_call),
                n_terms,
            },
            vol,
        )
        .map_err(core_to_py)
    })
}

#[doc = pydoc!("finstack_quant.models._fourier", "vg_cos_price")]
#[pyfunction]
#[pyo3(signature = (spot, strike, rate, div_yield, sigma, theta, nu, expiry, is_call, n_terms=None))]
#[allow(clippy::too_many_arguments)]
fn vg_cos_price(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    sigma: f64,
    theta: f64,
    nu: f64,
    expiry: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> PyResult<f64> {
    py.detach(move || {
        rust_vg_cos_price(
            CosMarketParams {
                spot,
                strike,
                rate,
                div_yield,
                expiry,
                option_type: OptionType::from(is_call),
                n_terms,
            },
            sigma,
            theta,
            nu,
        )
        .map_err(core_to_py)
    })
}

#[doc = pydoc!("finstack_quant.models._fourier", "merton_jump_cos_price")]
#[pyfunction]
#[pyo3(signature = (spot, strike, rate, div_yield, sigma, mu_jump, sigma_jump, lambda_, expiry, is_call, n_terms=None))]
#[allow(clippy::too_many_arguments)]
fn merton_jump_cos_price(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    sigma: f64,
    mu_jump: f64,
    sigma_jump: f64,
    lambda_: f64,
    expiry: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> PyResult<f64> {
    py.detach(move || {
        rust_merton_jump_cos_price(
            CosMarketParams {
                spot,
                strike,
                rate,
                div_yield,
                expiry,
                option_type: OptionType::from(is_call),
                n_terms,
            },
            sigma,
            mu_jump,
            sigma_jump,
            lambda_,
        )
        .map_err(core_to_py)
    })
}

/// Register Fourier pricing functions on the models submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(bs_cos_price, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(vg_cos_price, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(merton_jump_cos_price, m)?)?;
    Ok(())
}
