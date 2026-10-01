//! Process helpers and canonical Monte Carlo convenience functions.

use super::results::{PyGbmPathSummary, PyMoneyEstimate};
use crate::bindings::core::currency::extract_currency;
use crate::errors::core_to_py;
use finstack_quant_core::currency::Currency;
use pyo3::prelude::*;

/// Simulate a compact set of GBM spot paths through Rust path capture.
///
/// Parameters
/// ----------
/// spot : float
///     Positive initial underlying price.
/// rate : float
///     Continuously compounded annual risk-free rate (decimal).
/// div_yield : float
///     Continuously compounded annual dividend or carry yield (decimal).
/// vol : float
///     Annualized GBM volatility (decimal); must be strictly positive.
/// expiry : float
///     Positive simulation horizon in years.
/// num_steps : int
///     Number of equally spaced steps over the horizon.
/// num_paths : int
///     Number of captured paths (at most ``100_000``).
/// seed : int, optional
///     Deterministic Philox seed. ``None`` uses the Rust ``GbmPathConfig``
///     default seed (``42``), so two calls without a seed are identical.
/// antithetic : bool, default False
///     Antithetic pairing request; path capture rejects ``True``.
///
/// Returns
/// -------
/// GbmPathSummary
///     Shared time grid and captured spot paths.
///
/// Raises
/// ------
/// ValueError
///     If an input is out of domain (non-positive ``spot`` / ``vol`` /
///     ``expiry``, zero ``num_steps`` or ``num_paths``, the path cap is
///     exceeded) or ``antithetic`` is ``True``.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (spot, rate, div_yield, vol, expiry, num_steps, num_paths, seed=None, antithetic=false))]
fn simulate_gbm_paths(
    py: Python<'_>,
    spot: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_steps: usize,
    num_paths: usize,
    seed: Option<u64>,
    antithetic: bool,
) -> PyResult<PyGbmPathSummary> {
    let mut config = finstack_quant_models::monte_carlo::GbmPathConfig::new(
        spot, rate, div_yield, vol, expiry, num_steps, num_paths,
    )
    .with_antithetic(antithetic);
    if let Some(seed) = seed {
        config = config.with_seed(seed);
    }
    py.detach(move || finstack_quant_models::monte_carlo::simulate_gbm_paths(&config))
        .map(PyGbmPathSummary::from_inner)
        .map_err(core_to_py)
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
///     Simulated paths. Defaults to the configured European-pricer default.
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
///     Simulated paths. Defaults to the configured European-pricer default.
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
    m.add_function(wrap_pyfunction!(simulate_gbm_paths, m)?)?;
    m.add_function(wrap_pyfunction!(heston_satisfies_feller, m)?)?;
    m.add_function(wrap_pyfunction!(price_heston_call, m)?)?;
    m.add_function(wrap_pyfunction!(price_heston_put, m)?)?;
    Ok(())
}
