//! Fourier pricing method bindings (COS).
//!
//! Exposes model-agnostic Fourier pricers for European options under the
//! Black-Scholes, Variance Gamma, and Merton jump-diffusion models. Each
//! binding wraps a characteristic function (from `finstack-core`) with
//! the Fang-Oosterlee (2008) COS method from `finstack-valuations`.
//!
//! A Lewis (2001) single-integral binding (`bs_lewis_price`) was
//! removed because the underlying implementation was known-divergent
//! off-ATM. Use `bs_cos_price` for all Black-Scholes Fourier pricing.

use crate::errors::display_to_py;
use finstack_core::math::characteristic_function::{BlackScholesCf, MertonJumpCf, VarianceGammaCf};
use finstack_valuations::pricer::cos::{CosConfig, CosPricer};
use pyo3::prelude::*;

/// Build a [`CosConfig`] for the COS pricer.
///
/// When `n_terms` is `None` the term count is taken from
/// [`CosConfig::default()`] — the single canonical source for the COS
/// expansion default, shared with the WASM binding
/// (`finstack-wasm/.../fourier.rs`). No COS default is hardcoded in the
/// binding layer, so the Python and WASM defaults cannot drift from the core.
fn cos_config(n_terms: Option<usize>) -> CosConfig {
    let default = CosConfig::default();
    CosConfig {
        num_terms: n_terms.unwrap_or(default.num_terms),
        ..default
    }
}

// ---------------------------------------------------------------------------
// Black-Scholes
// ---------------------------------------------------------------------------

/// Price a European option under the Black-Scholes model using the COS method.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price of the underlying.
/// strike : float
///     Option strike.
/// rate : float
///     Continuously-compounded risk-free rate (annualized).
/// dividend : float
///     Continuous dividend yield (annualized).
/// vol : float
///     Annualized volatility (sigma).
/// maturity : float
///     Time to maturity in years.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
/// n_terms : int, optional
///     Number of cosine terms in the expansion (default 128).
///
/// Returns
/// -------
/// float
///     Present-value option price in the underlying's currency units.
#[pyfunction]
#[pyo3(signature = (spot, strike, rate, dividend, vol, maturity, is_call, n_terms=None))]
#[allow(clippy::too_many_arguments)]
fn bs_cos_price(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    dividend: f64,
    vol: f64,
    maturity: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> PyResult<f64> {
    py.detach(move || {
        let cf = BlackScholesCf {
            r: rate,
            q: dividend,
            sigma: vol,
        };
        let config = cos_config(n_terms);
        let pricer = CosPricer::new(&cf, config);
        if is_call {
            pricer
                .price_call(spot, strike, rate, maturity)
                .map_err(display_to_py)
        } else {
            pricer
                .price_put(spot, strike, rate, maturity)
                .map_err(display_to_py)
        }
    })
}

// ---------------------------------------------------------------------------
// Variance Gamma
// ---------------------------------------------------------------------------

/// Price a European option under the Variance Gamma model using the COS method.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price.
/// strike : float
///     Option strike.
/// rate : float
///     Continuously-compounded risk-free rate.
/// dividend : float
///     Continuous dividend yield.
/// sigma : float
///     Volatility of the subordinated Brownian motion.
/// theta : float
///     Drift of the subordinated Brownian motion (negative values skew left).
/// nu : float
///     Variance rate of the Gamma subordinator (nu > 0).
/// maturity : float
///     Time to maturity in years.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
/// n_terms : int, optional
///     Number of cosine terms (default 128; heavier tails may need 256+).
///
/// Returns
/// -------
/// float
///     Present-value option price.
#[pyfunction]
#[pyo3(signature = (spot, strike, rate, dividend, sigma, theta, nu, maturity, is_call, n_terms=None))]
#[allow(clippy::too_many_arguments)]
fn vg_cos_price(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    dividend: f64,
    sigma: f64,
    theta: f64,
    nu: f64,
    maturity: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> PyResult<f64> {
    py.detach(move || {
        let cf = VarianceGammaCf {
            r: rate,
            q: dividend,
            sigma,
            nu,
            theta,
        };
        let config = cos_config(n_terms);
        let pricer = CosPricer::new(&cf, config);
        if is_call {
            pricer
                .price_call(spot, strike, rate, maturity)
                .map_err(display_to_py)
        } else {
            pricer
                .price_put(spot, strike, rate, maturity)
                .map_err(display_to_py)
        }
    })
}

// ---------------------------------------------------------------------------
// Merton jump-diffusion
// ---------------------------------------------------------------------------

/// Price a European option under Merton (1976) jump-diffusion using the COS method.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price.
/// strike : float
///     Option strike.
/// rate : float
///     Continuously-compounded risk-free rate.
/// dividend : float
///     Continuous dividend yield.
/// sigma : float
///     Diffusion volatility.
/// mu_jump : float
///     Mean of log-jump size (negative for left-skewed crash risk).
/// sigma_jump : float
///     Standard deviation of log-jump size.
/// lambda : float
///     Jump intensity (expected number of jumps per year).
/// maturity : float
///     Time to maturity in years.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
/// n_terms : int, optional
///     Number of cosine terms (default 128).
///
/// Returns
/// -------
/// float
///     Present-value option price.
#[pyfunction]
#[pyo3(signature = (spot, strike, rate, dividend, sigma, mu_jump, sigma_jump, lambda, maturity, is_call, n_terms=None))]
#[allow(clippy::too_many_arguments)]
fn merton_jump_cos_price(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    dividend: f64,
    sigma: f64,
    mu_jump: f64,
    sigma_jump: f64,
    lambda: f64,
    maturity: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> PyResult<f64> {
    py.detach(move || {
        let cf = MertonJumpCf {
            r: rate,
            q: dividend,
            sigma,
            lambda,
            mu_j: mu_jump,
            sigma_j: sigma_jump,
        };
        let config = cos_config(n_terms);
        let pricer = CosPricer::new(&cf, config);
        if is_call {
            pricer
                .price_call(spot, strike, rate, maturity)
                .map_err(display_to_py)
        } else {
            pricer
                .price_put(spot, strike, rate, maturity)
                .map_err(display_to_py)
        }
    })
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Register Fourier pricing functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(bs_cos_price, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(vg_cos_price, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(merton_jump_cos_price, m)?)?;
    Ok(())
}
