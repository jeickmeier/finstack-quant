//! Closed-form analytic option primitives (Black-Scholes, Black-76, implied vol).
//!
//! Thin wrappers around `finstack_quant_valuations::models::closed_form`
//! that expose the per-unit pricing and Greek formulas to Python without
//! requiring a full `MarketContext` / `Instrument` round trip.
//!
//! Conventions mirror the underlying Rust crate:
//!
//! - `r`, `q` are continuously-compounded annualized rates (decimal).
//! - `sigma` is annualized volatility (decimal).
//! - `t` is time to expiry in years.
//! - Greeks use the canonical Rust scaling: `vega` and `rho_*` are per-1% move,
//!   `theta` is per day under ACT/365 (use 252 day-count via `theta_days` if you
//!   want a business-day convention).

use crate::errors::{display_to_py, value_error};
use finstack_quant_valuations::models::closed_form::implied_vol::{
    black76_implied_vol, bs_implied_vol,
};
use finstack_quant_valuations::models::closed_form::{
    arithmetic_asian_call_tw, arithmetic_asian_put_tw, bs_greeks_checked, bs_price_checked,
    checked_closed_form_value, down_in_call, down_out_call, fixed_strike_lookback_call,
    fixed_strike_lookback_put, floating_strike_lookback_call, floating_strike_lookback_put,
    geometric_asian_call, geometric_asian_put, option_type_from_bool, quanto_call, quanto_put,
    up_in_call, up_out_call, BsGreeks,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;

// ---------------------------------------------------------------------------
// bs_price
// ---------------------------------------------------------------------------

/// Black-Scholes / Garman-Kohlhagen per-unit price of a European option.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price `S`.
/// strike : float
///     Strike price `K`.
/// r : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// q : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// sigma : float
///     Annualized volatility (decimal, e.g. ``0.20`` for 20%).
/// t : float
///     Time to expiry in years.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
///
/// Returns
/// -------
/// float
///     Present-value option price (per unit; multiply by contract size to scale).
///
/// Raises
/// ------
/// ValueError
///     If the inputs produce a non-finite price (e.g. negative volatility).
#[pyfunction(name = "bs_price")]
#[pyo3(signature = (spot, strike, r, q, sigma, t, is_call))]
fn bs_price_wrapper(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    is_call: bool,
) -> PyResult<f64> {
    bs_price_checked(spot, strike, r, q, sigma, t, option_type_from_bool(is_call))
        .map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// bs_greeks
// ---------------------------------------------------------------------------

/// Black-Scholes / Garman-Kohlhagen Greeks for a European option.
///
/// Returns a dict with ``delta``, ``gamma``, ``vega``, ``theta``, ``rho`` (=rho_r),
/// and ``rho_q``. ``vega`` and both rho values are per 1% move; ``theta`` is
/// per day using the `theta_days` day-count (ACT/365 by default).
///
/// Parameters
/// ----------
/// spot, strike, r, q, sigma, t, is_call
///     Same as :func:`bs_price`.
/// theta_days : float, optional
///     Day-count denominator for per-day theta (default ``365.0``). Pass
///     ``252.0`` for business-day-scaled theta, ``360.0`` for ACT/360.
///
/// Returns
/// -------
/// dict
///     ``{"delta": ..., "gamma": ..., "vega": ..., "theta": ..., "rho": ..., "rho_q": ...}``.
#[pyfunction(name = "bs_greeks")]
#[pyo3(signature = (spot, strike, r, q, sigma, t, is_call, theta_days=365.0))]
#[allow(clippy::too_many_arguments)]
fn bs_greeks_wrapper<'py>(
    py: Python<'py>,
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    is_call: bool,
    theta_days: f64,
) -> PyResult<Bound<'py, PyDict>> {
    if !theta_days.is_finite() || theta_days <= 0.0 {
        return Err(value_error(format!(
            "theta_days must be positive, got {theta_days}"
        )));
    }
    let greeks: BsGreeks = bs_greeks_checked(
        spot,
        strike,
        r,
        q,
        sigma,
        t,
        option_type_from_bool(is_call),
        theta_days,
    )
    .map_err(display_to_py)?;
    let out = PyDict::new(py);
    out.set_item("delta", greeks.delta)?;
    out.set_item("gamma", greeks.gamma)?;
    out.set_item("vega", greeks.vega)?;
    out.set_item("theta", greeks.theta)?;
    out.set_item("rho", greeks.rho_r)?;
    out.set_item("rho_q", greeks.rho_q)?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// bs_implied_vol
// ---------------------------------------------------------------------------

/// Solve for Black-Scholes / Garman-Kohlhagen implied volatility.
///
/// Uses a Newton-in-vega hybrid with bisection fallback. Returns ``0.0`` when
/// ``t <= 0`` (expired — volatility is undefined); raises on non-finite inputs
/// or target prices outside the no-arbitrage bracket.
///
/// Parameters
/// ----------
/// spot, strike, r, q, t, is_call
///     Same as :func:`bs_price`.
/// price : float
///     Target per-unit option price.
///
/// Returns
/// -------
/// float
///     Implied volatility (annualized, decimal).
#[pyfunction(name = "bs_implied_vol")]
#[pyo3(signature = (spot, strike, r, q, t, price, is_call))]
fn bs_implied_vol_wrapper(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    t: f64,
    price: f64,
    is_call: bool,
) -> PyResult<f64> {
    bs_implied_vol(spot, strike, r, q, t, option_type_from_bool(is_call), price)
        .map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// black76_implied_vol
// ---------------------------------------------------------------------------

/// Solve for Black-76 (forward-based) implied volatility.
///
/// Takes a forward price, strike, discount factor, time to expiry, and target
/// price; returns the lognormal implied vol consistent with the Black-76
/// pricing formula.
///
/// Parameters
/// ----------
/// forward : float
///     Forward price `F`.
/// strike : float
///     Strike `K`.
/// df : float
///     Discount factor from expiry to settlement (``exp(-r * t)`` for
///     continuously-compounded rate ``r``).
/// t : float
///     Time to expiry in years.
/// price : float
///     Target per-unit option price.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
///
/// Returns
/// -------
/// float
///     Implied volatility (annualized, decimal).
#[pyfunction(name = "black76_implied_vol")]
#[pyo3(signature = (forward, strike, df, t, price, is_call))]
fn black76_implied_vol_wrapper(
    forward: f64,
    strike: f64,
    df: f64,
    t: f64,
    price: f64,
    is_call: bool,
) -> PyResult<f64> {
    black76_implied_vol(
        forward,
        strike,
        df,
        t,
        option_type_from_bool(is_call),
        price,
    )
    .map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Closed-form exotics: barrier / asian / lookback / quanto
// ---------------------------------------------------------------------------

/// Reiner-Rubinstein continuous-monitoring barrier call price.
///
/// Parameters
/// ----------
/// spot, strike, r, q, sigma, t
///     Same as :func:`bs_price`.
/// barrier : float
///     Barrier level.
/// direction : str
///     ``"up"`` or ``"down"`` (relative to spot / barrier).
/// knock : str
///     ``"in"`` (knock-in) or ``"out"`` (knock-out).
///
/// Returns
/// -------
/// float
///     Per-unit option price.
#[pyfunction(name = "barrier_call")]
#[pyo3(signature = (spot, strike, barrier, r, q, sigma, t, direction, knock))]
#[allow(clippy::too_many_arguments)]
fn barrier_call_wrapper(
    spot: f64,
    strike: f64,
    barrier: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    direction: &str,
    knock: &str,
) -> PyResult<f64> {
    let value = match (direction, knock) {
        ("up", "in") => up_in_call(spot, strike, barrier, t, r, q, sigma),
        ("up", "out") => up_out_call(spot, strike, barrier, t, r, q, sigma),
        ("down", "in") => down_in_call(spot, strike, barrier, t, r, q, sigma),
        ("down", "out") => down_out_call(spot, strike, barrier, t, r, q, sigma),
        _ => {
            return Err(crate::errors::value_error(format!(
                "unknown barrier spec: direction='{direction}' knock='{knock}'; \
                 expected direction in {{'up','down'}} and knock in {{'in','out'}}"
            )))
        }
    };
    finstack_quant_valuations::models::closed_form::checked_closed_form_value(
        value,
        "barrier price",
    )
    .map_err(crate::errors::core_to_py)
}

/// Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option call.
///
/// Parameters
/// ----------
/// spot, strike, r, q, sigma, t
///     Same as :func:`bs_price`.
/// num_fixings : int
///     Number of averaging fixings.
/// averaging : str, optional
///     ``"arithmetic"`` (Turnbull-Wakeman, default) or ``"geometric"``
///     (Kemna-Vorst exact).
/// is_call : bool, optional
///     ``True`` for call (default), ``False`` for put.
#[pyfunction(name = "asian_option_price")]
#[pyo3(signature = (spot, strike, r, q, sigma, t, num_fixings, averaging="arithmetic", is_call=true))]
#[allow(clippy::too_many_arguments)]
fn asian_option_wrapper(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    num_fixings: usize,
    averaging: &str,
    is_call: bool,
) -> PyResult<f64> {
    let value = match (averaging, is_call) {
        ("arithmetic", true) => arithmetic_asian_call_tw(spot, strike, t, r, q, sigma, num_fixings),
        ("arithmetic", false) => arithmetic_asian_put_tw(spot, strike, t, r, q, sigma, num_fixings),
        ("geometric", true) => geometric_asian_call(spot, strike, t, r, q, sigma, num_fixings),
        ("geometric", false) => geometric_asian_put(spot, strike, t, r, q, sigma, num_fixings),
        _ => {
            return Err(crate::errors::value_error(format!(
                "unknown averaging '{averaging}'; expected 'arithmetic' or 'geometric'"
            )))
        }
    };
    // Reject non-finite results (e.g. degenerate sigma=0 / t=0 / num_fixings=0)
    // at the host boundary, matching the barrier/quanto wrappers.
    checked_closed_form_value(value, "asian option price").map_err(display_to_py)
}

/// Conze-Viswanathan lookback option price.
///
/// Parameters
/// ----------
/// spot, strike, r, q, sigma, t
///     Same as :func:`bs_price`. For floating-strike, ``strike`` is ignored.
/// extremum : float
///     Observed historical extremum — max for fixed-strike call / floating-
///     strike put, min for fixed-strike put / floating-strike call. For a
///     fresh option with no observation, use ``spot``.
/// strike_type : str, optional
///     ``"fixed"`` (default) or ``"floating"``.
/// is_call : bool, optional
///     ``True`` for call (default), ``False`` for put.
#[pyfunction(name = "lookback_option_price")]
#[pyo3(signature = (spot, strike, r, q, sigma, t, extremum, strike_type="fixed", is_call=true))]
#[allow(clippy::too_many_arguments)]
fn lookback_option_wrapper(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    extremum: f64,
    strike_type: &str,
    is_call: bool,
) -> PyResult<f64> {
    let value = match (strike_type, is_call) {
        ("fixed", true) => fixed_strike_lookback_call(spot, strike, t, r, q, sigma, extremum),
        ("fixed", false) => fixed_strike_lookback_put(spot, strike, t, r, q, sigma, extremum),
        ("floating", true) => floating_strike_lookback_call(spot, t, r, q, sigma, extremum),
        ("floating", false) => floating_strike_lookback_put(spot, t, r, q, sigma, extremum),
        _ => {
            return Err(crate::errors::value_error(format!(
                "unknown strike_type '{strike_type}'; expected 'fixed' or 'floating'"
            )))
        }
    };
    // Reject non-finite results at the host boundary, matching barrier/quanto.
    checked_closed_form_value(value, "lookback option price").map_err(display_to_py)
}

/// Quanto option (cross-currency, FX-adjusted) price in domestic currency.
///
/// Parameters
/// ----------
/// spot : float
///     Spot price of the foreign asset in foreign currency.
/// strike : float
///     Strike in foreign currency.
/// t : float
///     Time to expiry in years.
/// rate_domestic, rate_foreign : float
///     Continuously-compounded domestic and foreign rates.
/// div_yield : float
///     Foreign asset dividend yield.
/// vol_asset : float
///     Foreign asset volatility.
/// vol_fx : float
///     Domestic/foreign FX volatility.
/// correlation : float
///     Correlation between asset and FX returns (``[-1, 1]``).
/// is_call : bool, optional
///     ``True`` for call (default), ``False`` for put.
///
/// Raises
/// ------
/// ValueError
///     If the inputs produce a non-finite price.
#[pyfunction(name = "quanto_option_price")]
#[pyo3(signature = (spot, strike, t, rate_domestic, rate_foreign, div_yield, vol_asset, vol_fx, correlation, is_call=true))]
#[allow(clippy::too_many_arguments)]
fn quanto_option_wrapper(
    spot: f64,
    strike: f64,
    t: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
    vol_fx: f64,
    correlation: f64,
    is_call: bool,
) -> PyResult<f64> {
    let price = if is_call {
        quanto_call(
            spot,
            strike,
            t,
            rate_domestic,
            rate_foreign,
            div_yield,
            vol_asset,
            vol_fx,
            correlation,
        )
    } else {
        quanto_put(
            spot,
            strike,
            t,
            rate_domestic,
            rate_foreign,
            div_yield,
            vol_asset,
            vol_fx,
            correlation,
        )
    };
    checked_closed_form_value(price, "quanto option price").map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the analytic option primitives on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(bs_price_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(bs_greeks_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(bs_implied_vol_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(black76_implied_vol_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(barrier_call_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(asian_option_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(lookback_option_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(quanto_option_wrapper, m)?)?;
    Ok(())
}
