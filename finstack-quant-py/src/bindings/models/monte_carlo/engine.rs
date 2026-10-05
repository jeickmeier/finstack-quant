//! Path simulation, process helpers and canonical Monte Carlo convenience functions.

use super::results::{PyMoneyEstimate, PyPathSummary};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::module_utils::py_to_json_value;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_models::monte_carlo::simulate::PathSimulationSpec;
use pyo3::prelude::*;

/// Simulate paths of any built-in process on a shared time grid.
///
/// Binds Rust ``monte_carlo::simulate::simulate_paths``: ``spec`` selects the
/// process, the discretization scheme, the time grid and the random streams,
/// and Rust validates every field. The GIL is released while paths are
/// simulated.
///
/// Parameters
/// ----------
/// spec : dict or str
///     ``PathSimulationSpec`` as a dict or its JSON text, with keys:
///
///     - ``process`` : dict tagged by ``"type"`` — ``"gbm"``,
///       ``"gbm_with_dividends"``, ``"multi_gbm"``, ``"brownian"``,
///       ``"multi_brownian"``, ``"multi_ou"``, ``"hull_white_1f"``, ``"cir"``,
///       ``"cir_plus_plus"``, ``"heston"``, ``"schwartz_smith"``,
///       ``"local_vol"``, ``"lmm"``, ``"rough_bergomi"``, ``"rough_heston"`` or
///       ``"cheyette_rough"`` — plus
///       that process's parameters. Rates, yields and volatilities are
///       annualized decimals.
///     - ``scheme`` : ``"default"`` (the process's canonical scheme; used when
///       omitted), ``"euler"``, ``"log_euler"`` or ``"milstein"``.
///     - ``initial_state`` : list of float, the state at time zero in the
///       process's state layout (``[spot]`` for GBM, ``[spot, variance]`` for
///       Heston, ``[short_rate]`` for the short-rate models).
///     - ``time_grid`` : ``{"type": "uniform", "expiry": years, "num_steps": n}``
///       or ``{"type": "times", "times": [0.0, ...]}`` in year fractions.
///     - ``num_paths`` : int in ``[1, 100_000]``, the number of independent
///       random streams.
///     - ``seed`` : int, root Philox seed; the same spec reproduces the same
///       paths bit for bit.
///     - ``antithetic`` : bool, default ``False``; store an antithetic partner
///       after each stream's path.
///     - ``fbm`` : dict, optional; fractional-noise generator for
///       ``"rough_bergomi"`` and ``"cheyette_rough"``: ``{"type": "volterra"}``
///       (used when omitted), ``{"type": "cholesky"}`` or
///       ``{"type": "windowed_conditional", "near_field_size": n}``.
///
/// Returns
/// -------
/// PathSummary
///     Every simulated state, including the initial state at time zero.
///
/// Raises
/// ------
/// ValueError
///     If ``spec`` is not valid ``PathSimulationSpec`` data (unknown key or
///     tag, missing field, wrong type); a process parameter or correlation
///     matrix is out of range; the scheme is not available for the process;
///     ``fbm`` is set for a process that does not consume fractional noise;
///     ``initial_state`` has the wrong length or lies outside the process's
///     domain; the time grid is invalid; ``num_paths`` is outside
///     ``[1, 100_000]``; the output would exceed ``64_000_000`` stored values;
///     or a simulated state is non-finite.
#[pyfunction]
#[pyo3(text_signature = "(spec)")]
fn simulate_paths(py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<PyPathSummary> {
    let value = py_to_json_value(py, spec, "PathSimulationSpec")?;
    py.detach(move || {
        let spec: PathSimulationSpec = serde_json::from_value(value)
            .map_err(|e| serde_json_to_py(e, "invalid PathSimulationSpec"))?;
        finstack_quant_models::monte_carlo::simulate::simulate_paths(&spec).map_err(core_to_py)
    })
    .map(PyPathSummary::from_inner)
}

/// Test the inclusive Feller condition ``2 * kappa * theta >= vol_of_vol**2``.
///
/// This is the Monte Carlo engine's own predicate
/// (`finstack_quant_models::monte_carlo::process::heston::feller_condition`), so the
/// answer at the boundary matches :func:`price_heston_call` /
/// :func:`price_heston_put`. Inputs are not validated: non-finite values
/// typically yield ``False``.
///
/// Parameters
/// ----------
/// kappa : float
///     Mean-reversion speed of the variance process per year.
/// theta : float
///     Long-run variance level in squared-volatility units.
/// vol_of_vol : float
///     Annualized volatility of the variance process.
///
/// Returns
/// -------
/// bool
///     ``True`` when ``2 * kappa * theta >= vol_of_vol**2``.
///
/// Sources
/// -------
/// - Heston (1993): see docs/REFERENCES.md#heston-1993
#[pyfunction]
fn heston_satisfies_feller(kappa: f64, theta: f64, vol_of_vol: f64) -> bool {
    finstack_quant_models::monte_carlo::process::heston::feller_condition(kappa, theta, vol_of_vol)
}

/// Resolve an optional currency argument, defaulting to the registry default.
pub(super) fn resolve_currency(
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<finstack_quant_core::currency::Currency> {
    finstack_quant_models::monte_carlo::convenience::resolve_currency(extract_optional_currency(
        currency,
    )?)
    .map_err(core_to_py)
}

/// Extract an optional currency argument without applying any default.
///
/// Canonical entry points in the Monte Carlo crate own the registry default;
/// the binding only marshals an explicitly supplied currency.
fn extract_optional_currency(currency: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Currency>> {
    currency.map(extract_currency).transpose()
}

#[allow(clippy::too_many_arguments)]
fn price_heston(
    py: Python<'_>,
    is_call: bool,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMoneyEstimate> {
    use finstack_quant_models::monte_carlo::pricer::heston as canonical;

    let ccy = extract_optional_currency(currency)?;
    py.detach(|| {
        if is_call {
            canonical::price_heston_call(
                spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry,
                num_paths, seed, num_steps, ccy,
            )
        } else {
            canonical::price_heston_put(
                spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry,
                num_paths, seed, num_steps, ccy,
            )
        }
    })
    .map(PyMoneyEstimate::from_inner)
    .map_err(core_to_py)
}

/// Price a European call under the Heston stochastic-volatility model by Monte Carlo.
///
/// Paths are generated with the Quadratic-Exponential (QE) discretization of
/// Andersen (2008), which stays stable when the Feller condition
/// (``2 * kappa * theta >= vol_of_vol**2``) is violated — the common case for
/// equity calibrations. Check it with
/// :func:`~finstack_quant.models.monte_carlo.heston_satisfies_feller`.
///
/// Parameters
/// ----------
/// spot : float
///     Current underlying price.
/// strike : float
///     Option strike.
/// rate : float
///     Continuously compounded risk-free rate.
/// div_yield : float
///     Continuous dividend yield.
/// kappa : float
///     Mean-reversion speed of the variance process.
/// theta : float
///     Long-run variance level.
/// vol_of_vol : float
///     Volatility of variance.
/// rho : float
///     Correlation between the spot and variance Brownian drivers, in ``[-1, 1]``.
/// v0 : float
///     Initial instantaneous variance (variance, not volatility).
/// expiry : float
///     Time to expiry in years.
/// num_paths : int, optional
///     Independent path estimators in ``[2, 10_000_000]``; each antithetic
///     pair counts once. Defaults to the configured European-pricer default.
/// seed : int, optional
///     RNG seed. The same seed reproduces the same price on any thread count.
/// num_steps : int, optional
///     Time steps per path.
/// currency : Currency or str, optional
///     Currency stamped on the result. Defaults to the configured default.
///
/// Returns
/// -------
/// MoneyEstimate
///     Price with its Monte Carlo standard error.
///
/// References
/// ----------
/// - Andersen QE (2008): see docs/REFERENCES.md#andersen-2008-heston-qe
/// - Heston (1993): see docs/REFERENCES.md#heston-1993
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths=None, seed=None, num_steps=None, currency=None))]
fn price_heston_call(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMoneyEstimate> {
    price_heston(
        py, true, spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry,
        num_paths, seed, num_steps, currency,
    )
}

/// Price a European put under the Heston stochastic-volatility model by Monte Carlo.
///
/// Identical machinery to :func:`price_heston_call` — QE discretization,
/// same parameters, same determinism guarantee — with a put payoff.
///
/// Parameters
/// ----------
/// spot : float
///     Current underlying price.
/// strike : float
///     Option strike.
/// rate : float
///     Continuously compounded risk-free rate.
/// div_yield : float
///     Continuous dividend yield.
/// kappa : float
///     Mean-reversion speed of the variance process.
/// theta : float
///     Long-run variance level.
/// vol_of_vol : float
///     Volatility of variance.
/// rho : float
///     Correlation between the spot and variance Brownian drivers, in ``[-1, 1]``.
/// v0 : float
///     Initial instantaneous variance (variance, not volatility).
/// expiry : float
///     Time to expiry in years.
/// num_paths : int, optional
///     Independent path estimators in ``[2, 10_000_000]``; each antithetic
///     pair counts once. Defaults to the configured European-pricer default.
/// seed : int, optional
///     RNG seed. The same seed reproduces the same price on any thread count.
/// num_steps : int, optional
///     Time steps per path.
/// currency : Currency or str, optional
///     Currency stamped on the result. Defaults to the configured default.
///
/// Returns
/// -------
/// MoneyEstimate
///     Price with its Monte Carlo standard error.
///
/// See Also
/// --------
/// price_heston_call : Call counterpart, with full model references.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths=None, seed=None, num_steps=None, currency=None))]
fn price_heston_put(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMoneyEstimate> {
    price_heston(
        py, false, spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry,
        num_paths, seed, num_steps, currency,
    )
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(simulate_paths, m)?)?;
    m.add_function(wrap_pyfunction!(heston_satisfies_feller, m)?)?;
    m.add_function(wrap_pyfunction!(price_heston_call, m)?)?;
    m.add_function(wrap_pyfunction!(price_heston_put, m)?)?;
    Ok(())
}
