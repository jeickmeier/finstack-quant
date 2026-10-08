//! Python bindings for Monte Carlo Greek estimators.
//!
//! Thin wrappers around the canonical GBM European finite-difference
//! convenience entry points in `finstack_quant_models::monte_carlo::greeks::gbm_european`.

use super::engine::resolve_currency;
use super::results::PyEstimate;
use crate::errors::core_to_py;
use finstack_quant_models::monte_carlo::greeks::gbm_european::{
    finite_diff_delta_gbm, finite_diff_gamma_gbm, GbmEuropeanFdSpec,
};
use finstack_quant_models::OptionType;
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
        currency: Some(resolve_currency(currency)?),
    })
}

/// Finite-difference delta for a vanilla European option under GBM.
///
/// Central difference in spot with common random numbers: the up and down
/// valuations share each path's random draws, the estimate is the mean of
/// the per-path differences, and ``stderr`` is the paired
/// (common-random-number) standard error of those differences.
///
/// Parameters
/// ----------
/// spot : float
///     Finite positive spot price. The absolute bump is
///     ``max(abs(spot) * bump_size, 1e-8)`` and the symmetric down-bumped
///     state must remain at least ``1e-12``.
/// strike : float
///     Strike price.
/// rate : float
///     Risk-free rate (continuously compounded decimal).
/// div_yield : float
///     Dividend yield (continuously compounded decimal).
/// vol : float
///     Annualized volatility (decimal); must be strictly positive.
/// expiry : float
///     Maturity in years.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
/// num_paths : int, optional
///     Paths per evaluation; defaults to the registry value.
/// seed : int, optional
///     RNG seed; defaults to the registry value.
/// num_steps : int, optional
///     Time-grid steps; defaults to the registry value.
/// bump_size : float, optional
///     Relative Monte Carlo spot shock (registry default ``0.01`` = 1% of
///     spot), not a closed-form local step.
/// currency : Currency or str, optional
///     Currency stamped on the simulated payoffs; defaults to the registry value.
///
/// Returns
/// -------
/// Estimate
///     ``mean`` is the delta, ``stderr`` its paired (common-random-number)
///     standard error, ``ci_lower`` / ``ci_upper`` the symmetric 95% band.
///
/// Raises
/// ------
/// ValueError
///     If ``vol`` is not strictly positive, the inputs cannot form a
///     symmetric central stencil, or another pricing input is invalid.
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
    py.detach(|| finite_diff_delta_gbm(spec))
        .map(PyEstimate::from_inner)
        .map_err(core_to_py)
}

/// Finite-difference gamma for a vanilla European option under GBM.
///
/// Second central difference in spot with common random numbers; the
/// estimate is the mean of the per-path second differences and ``stderr``
/// is their paired (common-random-number) standard error.
///
/// Parameters
/// ----------
/// spot : float
///     Finite positive spot price; see ``finite_diff_delta`` for the bump rule.
/// strike : float
///     Strike price.
/// rate : float
///     Risk-free rate (continuously compounded decimal).
/// div_yield : float
///     Dividend yield (continuously compounded decimal).
/// vol : float
///     Annualized volatility (decimal); must be strictly positive.
/// expiry : float
///     Maturity in years.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
/// num_paths : int, optional
///     Paths per evaluation; defaults to the registry value.
/// seed : int, optional
///     RNG seed; defaults to the registry value.
/// num_steps : int, optional
///     Time-grid steps; defaults to the registry value.
/// bump_size : float, optional
///     Relative Monte Carlo spot shock (registry default ``0.01``).
/// currency : Currency or str, optional
///     Currency stamped on the simulated payoffs; defaults to the registry value.
///
/// Returns
/// -------
/// Estimate
///     ``mean`` is the gamma, ``stderr`` its paired (common-random-number)
///     standard error.
///
/// Raises
/// ------
/// ValueError
///     If ``vol`` is not strictly positive, the inputs cannot form a
///     symmetric central stencil, or another pricing input is invalid.
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
    py.detach(|| finite_diff_gamma_gbm(spec))
        .map(PyEstimate::from_inner)
        .map_err(core_to_py)
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(finite_diff_delta, m)?)?;
    m.add_function(wrap_pyfunction!(finite_diff_gamma, m)?)?;
    Ok(())
}
