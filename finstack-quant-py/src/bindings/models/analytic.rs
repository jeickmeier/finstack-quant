//! Closed-form analytic option primitives (Black-Scholes, Black-76, Bachelier,
//! shifted Black, Heston Fourier, implied vol, and closed-form exotics).
//!
//! Thin wrappers around `finstack_quant_models::closed_form`
//! that expose the per-unit pricing and Greek formulas to Python without
//! requiring a full `MarketContext` / `Instrument` round trip.
//!
//! Conventions mirror the underlying Rust crate:
//!
//! - `rate`, `div_yield` are continuously-compounded annualized rates (decimal).
//! - `vol` is annualized lognormal volatility (decimal); `normal_vol` is an
//!   absolute (Bachelier) volatility in the units of the forward.
//! - `expiry` is time to expiry in years.
//! - Greeks use the canonical Rust scaling: `vega` and `rho_*` are per-1% move,
//!   `theta` is per day under ACT/365 (use 252 day-count via `theta_days` if you
//!   want a business-day convention).

use crate::bindings::pandas_utils::{
    labeled_values_to_series, serde_object_to_single_row_dataframe,
};
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_models::closed_form::implied_vol::{black76_implied_vol, bs_implied_vol};
use finstack_quant_models::closed_form::{
    asian_option_price_str, bachelier_call, bachelier_delta_call, bachelier_delta_put,
    bachelier_gamma, bachelier_put, bachelier_vega, barrier_call_str, barrier_put_str, black_call,
    black_delta_call, black_delta_put, black_gamma, black_put, black_shifted_call,
    black_shifted_put, black_shifted_vega, black_vega, bs_greeks, bs_price,
    checked_closed_form_value, heston_call_price_fourier, heston_put_price_fourier,
    lookback_option_price_str, quanto_option_price, vanilla_expiry_payoff, BsGreeks,
    HestonPricingParams,
};
use finstack_quant_models::OptionType;
use pyo3::prelude::*;
use pyo3::types::PyDict;

const DEFAULT_THETA_DAYS_PER_YEAR: f64 = 365.0;

const BS_GREEK_LABELS: [&str; 6] = ["delta", "gamma", "vega", "theta", "rho_r", "rho_q"];

// BsGreeks

/// Black-Scholes / Garman-Kohlhagen Greeks for one European option (per unit).
///
/// Returned by ``bs_greeks``. ``vega``, ``rho_r`` and ``rho_q`` are per 1%
/// move; ``theta`` is per day under the ``theta_days`` basis passed to
/// ``bs_greeks``; ``delta`` and ``gamma`` are per unit of spot.
///
/// The object is immutable, compares by value, is picklable, and exposes
/// ``to_series()`` / ``to_dataframe()`` pandas exits plus ``to_json`` /
/// ``from_json`` for wire round-trips.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import bs_greeks
/// >>> g = bs_greeks(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)
/// >>> round(g.delta, 4)
/// 0.6368
/// >>> g.to_series().index.tolist()
/// ['delta', 'gamma', 'vega', 'theta', 'rho_r', 'rho_q']
#[pyclass(
    name = "BsGreeks",
    module = "finstack_quant.models",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyBsGreeks {
    pub(crate) inner: BsGreeks,
}

impl PyBsGreeks {
    pub(crate) fn from_inner(inner: BsGreeks) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyBsGreeks {
    /// Spot delta per unit of underlying (``dV/dS``).
    #[getter]
    fn delta(&self) -> f64 {
        self.inner.delta
    }

    /// Gamma per unit of underlying (``d2V/dS2``).
    #[getter]
    fn gamma(&self) -> f64 {
        self.inner.gamma
    }

    /// Vega per 1% (0.01) move in volatility.
    #[getter]
    fn vega(&self) -> f64 {
        self.inner.vega
    }

    /// Theta per day under the ``theta_days`` basis (negative = decay).
    #[getter]
    fn theta(&self) -> f64 {
        self.inner.theta
    }

    /// Rho to the domestic / risk-free rate per 1% (0.01) move.
    #[getter]
    fn rho_r(&self) -> f64 {
        self.inner.rho_r
    }

    /// Rho to the dividend yield / foreign rate per 1% (0.01) move.
    #[getter]
    fn rho_q(&self) -> f64 {
        self.inner.rho_q
    }

    /// ``True`` when every Greek is finite and ``gamma`` / ``vega`` are non-negative.
    ///
    /// Delta is deliberately not bounded: with negative carry ``|delta|`` may
    /// legitimately exceed one.
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Return the Greeks as a float ``pandas.Series`` named ``bs_greeks``.
    ///
    /// Index order is ``delta, gamma, vega, theta, rho_r, rho_q``.
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = BS_GREEK_LABELS.iter().map(|s| (*s).to_string()).collect();
        labeled_values_to_series(py, &labels, self.values(), "bs_greeks")
    }

    /// Return the Greeks as a single-row ``pandas.DataFrame``.
    ///
    /// Columns are ``delta, gamma, vega, theta, rho_r, rho_q``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    /// Serialize to compact JSON with the canonical field names.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "BsGreeks"))
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when a field is missing or unknown.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: BsGreeks =
            serde_json::from_str(json).map_err(|e| serde_json_to_py(e, "invalid BsGreeks JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` (and therefore ``copy.deepcopy``, ``multiprocessing``).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("BsGreeks", &self.inner)
    }
}

impl PyBsGreeks {
    fn values(&self) -> Vec<f64> {
        vec![
            self.inner.delta,
            self.inner.gamma,
            self.inner.vega,
            self.inner.theta,
            self.inner.rho_r,
            self.inner.rho_q,
        ]
    }
}

// bs_price

/// Black-Scholes / Garman-Kohlhagen per-unit price of a European option.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price ``S``.
/// strike : float
///     Strike price ``K``.
/// rate : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// div_yield : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// vol : float
///     Annualized volatility (decimal, e.g. ``0.20`` for 20%).
/// expiry : float
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
///     If spot or strike is non-positive, volatility or expiry is negative,
///     any numerical input is non-finite, or discounted legs or price overflow.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import bs_price
/// >>> round(bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True), 4)
/// 10.4506
///
/// Sources
/// -------
/// - Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973
/// - Merton (1973): see docs/REFERENCES.md#merton-1973
/// - Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983
#[pyfunction(name = "bs_price")]
#[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, is_call))]
fn bs_price_wrapper(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
) -> PyResult<f64> {
    bs_price(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

/// Vanilla option payoff at expiry: ``max(±(spot - strike), 0)``.
///
/// Parameters
/// ----------
/// spot : float
///     Underlying level at expiry, in the same price units as ``strike``.
/// strike : float
///     Exercise price; must be finite and strictly positive.
/// is_call : bool
///     ``True`` for a call (``max(spot - strike, 0)``), ``False`` for a put
///     (``max(strike - spot, 0)``).
///
/// Returns
/// -------
/// float
///     Undiscounted expiry payoff in the same units as ``spot`` and ``strike``.
///
/// Raises
/// ------
/// ValueError
///     If ``spot`` is non-finite or ``strike`` is non-finite or not strictly
///     positive.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import vanilla_expiry_payoff
/// >>> vanilla_expiry_payoff(110.0, 100.0, True)
/// 10.0
#[pyfunction(name = "vanilla_expiry_payoff")]
#[pyo3(signature = (spot, strike, is_call))]
fn vanilla_expiry_payoff_wrapper(spot: f64, strike: f64, is_call: bool) -> PyResult<f64> {
    vanilla_expiry_payoff(spot, strike, OptionType::from(is_call)).map_err(core_to_py)
}

// bs_greeks

/// Black-Scholes / Garman-Kohlhagen Greeks for a European option.
///
/// Returns a ``BsGreeks`` value with ``delta``, ``gamma``, ``vega``, ``theta``,
/// ``rho_r`` and ``rho_q``. ``vega`` and both rho values are per 1% move;
/// ``theta`` is per day using the ``theta_days`` day-count (ACT/365 by default).
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price ``S``.
/// strike : float
///     Strike price ``K``.
/// rate : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// div_yield : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// vol : float
///     Annualized volatility (decimal, e.g. ``0.20`` for 20%); must be positive.
/// expiry : float
///     Time to expiry in years; must be positive.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
/// theta_days : float, optional
///     Day-count denominator for per-day theta (default ``365.0``). Pass
///     ``252.0`` for business-day-scaled theta, ``360.0`` for ACT/360.
///
/// Returns
/// -------
/// BsGreeks
///     Typed Greeks with ``to_series()`` / ``to_dataframe()`` exits.
///
/// Raises
/// ------
/// ValueError
///     If any input is non-finite; ``spot`` or ``strike`` is non-positive;
///     ``vol``, ``expiry`` or ``theta_days`` is non-positive; or a Greek is
///     non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import bs_greeks
/// >>> g = bs_greeks(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)
/// >>> round(g.delta, 4), round(g.rho_r, 4)
/// (0.6368, 0.5323)
///
/// Sources
/// -------
/// - Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973
/// - Merton (1973): see docs/REFERENCES.md#merton-1973
/// - Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983
#[pyfunction(name = "bs_greeks")]
#[pyo3(
    signature = (spot, strike, rate, div_yield, vol, expiry, is_call, theta_days=DEFAULT_THETA_DAYS_PER_YEAR),
    text_signature = "(spot, strike, rate, div_yield, vol, expiry, is_call, theta_days=365.0)"
)]
#[allow(clippy::too_many_arguments)]
fn bs_greeks_wrapper(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
    theta_days: f64,
) -> PyResult<PyBsGreeks> {
    // theta_days validation (finite, > 0) lives in `bs_greeks`.
    bs_greeks(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        OptionType::from(is_call),
        theta_days,
    )
    .map(PyBsGreeks::from_inner)
    .map_err(core_to_py)
}

// bs_implied_vol

/// Solve for Black-Scholes / Garman-Kohlhagen implied volatility.
///
/// Uses a Newton-in-vega hybrid with bisection fallback. Raises on non-finite
/// inputs, an expired option (``expiry <= 0``), or target prices outside the
/// no-arbitrage bracket.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price ``S``.
/// strike : float
///     Strike price ``K``.
/// rate : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// div_yield : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// expiry : float
///     Time to expiry in years; must be strictly positive.
/// price : float
///     Target per-unit option price.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
///
/// Returns
/// -------
/// float
///     Implied volatility (annualized, decimal).
///
/// Raises
/// ------
/// ValueError
///     If an input is non-finite; ``expiry``, ``spot``, ``strike`` or
///     ``price`` is non-positive; ``price`` is at or below intrinsic value or
///     cannot be bracketed; or the solver does not converge.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import bs_implied_vol, bs_price
/// >>> price = bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)
/// >>> round(bs_implied_vol(100.0, 100.0, 0.05, 0.0, 1.0, price, True), 6)
/// 0.2
///
/// Sources
/// -------
/// - Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973
/// - Merton (1973): see docs/REFERENCES.md#merton-1973
#[pyfunction(name = "bs_implied_vol")]
#[pyo3(signature = (spot, strike, rate, div_yield, expiry, price, is_call))]
fn bs_implied_vol_wrapper(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    expiry: f64,
    price: f64,
    is_call: bool,
) -> PyResult<f64> {
    bs_implied_vol(
        spot,
        strike,
        rate,
        div_yield,
        expiry,
        OptionType::from(is_call),
        price,
    )
    .map_err(core_to_py)
}

// black76_implied_vol

/// Solve for Black-76 (forward-based) implied volatility.
///
/// Takes a forward price, strike, discount factor, time to expiry, and target
/// price; returns the lognormal implied vol consistent with the Black-76
/// pricing formula.
///
/// Parameters
/// ----------
/// forward : float
///     Forward price ``F``.
/// strike : float
///     Strike ``K``.
/// df : float
///     Discount factor from expiry to settlement (``exp(-rate * expiry)`` for
///     continuously-compounded ``rate``).
/// expiry : float
///     Time to expiry in years; must be strictly positive.
/// price : float
///     Target per-unit (discounted) option price.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
///
/// Returns
/// -------
/// float
///     Implied volatility (annualized, decimal).
///
/// Raises
/// ------
/// ValueError
///     If an input is non-finite; ``expiry``, ``forward``, ``strike``, ``df``
///     or ``price`` is non-positive; ``price`` is not above intrinsic or
///     cannot be bracketed; or the solver does not converge.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import black76_implied_vol
/// >>> round(black76_implied_vol(100.0, 100.0, 0.95, 1.0, 7.5673, True), 6)
/// 0.2
///
/// Sources
/// -------
/// - Black (1976): see docs/REFERENCES.md#black-1976
#[pyfunction(name = "black76_implied_vol")]
#[pyo3(signature = (forward, strike, df, expiry, price, is_call))]
fn black76_implied_vol_wrapper(
    forward: f64,
    strike: f64,
    df: f64,
    expiry: f64,
    price: f64,
    is_call: bool,
) -> PyResult<f64> {
    black76_implied_vol(
        forward,
        strike,
        df,
        expiry,
        OptionType::from(is_call),
        price,
    )
    .map_err(core_to_py)
}

// black76_price / black76_greeks

/// Black-76 per-unit price of a European option on a forward.
///
/// ``df * Black(forward, strike, vol, expiry)``: the undiscounted Black
/// premium scaled by the supplied discount factor.
///
/// Parameters
/// ----------
/// forward : float
///     Forward price or rate ``F`` at expiry.
/// strike : float
///     Strike ``K`` in the same units as ``forward``.
/// df : float
///     Discount factor from valuation date to expiry (positive decimal).
/// expiry : float
///     Time to expiry in years.
/// vol : float
///     Annualized lognormal (Black) volatility, decimal.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
///
/// Returns
/// -------
/// float
///     Discounted per-unit option price in the units of ``forward``.
///
/// Raises
/// ------
/// ValueError
///     If the inputs produce a non-finite price (for example a negative or
///     non-finite volatility or forward).
///
/// Examples
/// --------
/// >>> from finstack_quant.models import black76_price
/// >>> round(black76_price(100.0, 100.0, 0.95, 1.0, 0.2, True), 4)
/// 7.5673
///
/// Sources
/// -------
/// - Black (1976): see docs/REFERENCES.md#black-1976
#[pyfunction(name = "black76_price")]
#[pyo3(signature = (forward, strike, df, expiry, vol, is_call))]
fn black76_price_wrapper(
    forward: f64,
    strike: f64,
    df: f64,
    expiry: f64,
    vol: f64,
    is_call: bool,
) -> PyResult<f64> {
    let undiscounted = if is_call {
        black_call(forward, strike, vol, expiry)
    } else {
        black_put(forward, strike, vol, expiry)
    };
    checked_closed_form_value(df * undiscounted, "Black-76 price").map_err(core_to_py)
}

/// Black-76 forward Greeks ``{"delta", "gamma", "vega"}`` (undiscounted).
///
/// ``delta`` and ``gamma`` are with respect to the forward; ``vega`` is per
/// unit (1.0) change in ``vol``. Multiply by the discount factor to obtain
/// present-value sensitivities.
///
/// Parameters
/// ----------
/// forward : float
///     Forward price or rate ``F`` at expiry.
/// strike : float
///     Strike ``K`` in the same units as ``forward``.
/// expiry : float
///     Time to expiry in years.
/// vol : float
///     Annualized lognormal (Black) volatility, decimal.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put (only ``delta`` differs).
///
/// Returns
/// -------
/// dict[str, float]
///     ``{"delta": ..., "gamma": ..., "vega": ...}``.
///
/// Raises
/// ------
/// ValueError
///     If any Greek is non-finite for the supplied inputs.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import black76_greeks
/// >>> g = black76_greeks(100.0, 100.0, 1.0, 0.2, True)
/// >>> round(g["delta"], 4)
/// 0.5398
///
/// Sources
/// -------
/// - Black (1976): see docs/REFERENCES.md#black-1976
#[pyfunction(name = "black76_greeks")]
#[pyo3(signature = (forward, strike, expiry, vol, is_call))]
fn black76_greeks_wrapper<'py>(
    py: Python<'py>,
    forward: f64,
    strike: f64,
    expiry: f64,
    vol: f64,
    is_call: bool,
) -> PyResult<Bound<'py, PyDict>> {
    let delta = if is_call {
        black_delta_call(forward, strike, vol, expiry)
    } else {
        black_delta_put(forward, strike, vol, expiry)
    };
    forward_greeks_dict(
        py,
        delta,
        black_gamma(forward, strike, vol, expiry),
        black_vega(forward, strike, vol, expiry),
        "Black-76",
    )
}

// bachelier_price / bachelier_greeks

/// Bachelier (normal-model) undiscounted per-unit price of a European option.
///
/// Parameters
/// ----------
/// forward : float
///     Forward price or rate ``F`` at expiry (may be negative).
/// strike : float
///     Strike ``K`` in the same units as ``forward``.
/// normal_vol : float
///     Annualized **absolute** (normal / Bachelier) volatility in the units of
///     ``forward`` — e.g. ``0.0075`` for 75 bp on a rate quoted as a decimal.
/// expiry : float
///     Time to expiry in years.
/// is_call : bool
///     ``True`` for a call (payer), ``False`` for a put (receiver).
///
/// Returns
/// -------
/// float
///     Undiscounted per-unit option value in the units of ``forward``;
///     multiply by the discount factor for present value.
///
/// Raises
/// ------
/// ValueError
///     If the inputs produce a non-finite price.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import bachelier_price
/// >>> round(bachelier_price(0.03, 0.03, 0.0075, 1.0, True), 6)
/// 0.002992
///
/// Sources
/// -------
/// - Bachelier (1900): see docs/REFERENCES.md#bachelier-1900
#[pyfunction(name = "bachelier_price")]
#[pyo3(signature = (forward, strike, normal_vol, expiry, is_call))]
fn bachelier_price_wrapper(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    is_call: bool,
) -> PyResult<f64> {
    let value = if is_call {
        bachelier_call(forward, strike, normal_vol, expiry)
    } else {
        bachelier_put(forward, strike, normal_vol, expiry)
    };
    checked_closed_form_value(value, "Bachelier price").map_err(core_to_py)
}

/// Bachelier (normal-model) forward Greeks ``{"delta", "gamma", "vega"}``.
///
/// ``delta`` and ``gamma`` are with respect to the forward; ``vega`` is per
/// unit (1.0) change in ``normal_vol`` (absolute units). All values are
/// undiscounted.
///
/// Parameters
/// ----------
/// forward : float
///     Forward price or rate ``F`` at expiry (may be negative).
/// strike : float
///     Strike ``K`` in the same units as ``forward``.
/// normal_vol : float
///     Annualized absolute (normal) volatility in the units of ``forward``.
/// expiry : float
///     Time to expiry in years.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put (only ``delta`` differs).
///
/// Returns
/// -------
/// dict[str, float]
///     ``{"delta": ..., "gamma": ..., "vega": ...}``.
///
/// Raises
/// ------
/// ValueError
///     If any Greek is non-finite for the supplied inputs.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import bachelier_greeks
/// >>> round(bachelier_greeks(0.03, 0.03, 0.0075, 1.0, True)["delta"], 2)
/// 0.5
///
/// Sources
/// -------
/// - Bachelier (1900): see docs/REFERENCES.md#bachelier-1900
#[pyfunction(name = "bachelier_greeks")]
#[pyo3(signature = (forward, strike, normal_vol, expiry, is_call))]
fn bachelier_greeks_wrapper<'py>(
    py: Python<'py>,
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    is_call: bool,
) -> PyResult<Bound<'py, PyDict>> {
    let delta = if is_call {
        bachelier_delta_call(forward, strike, normal_vol, expiry)
    } else {
        bachelier_delta_put(forward, strike, normal_vol, expiry)
    };
    forward_greeks_dict(
        py,
        delta,
        bachelier_gamma(forward, strike, normal_vol, expiry),
        bachelier_vega(forward, strike, normal_vol, expiry),
        "Bachelier",
    )
}

fn forward_greeks_dict<'py>(
    py: Python<'py>,
    delta: f64,
    gamma: f64,
    vega: f64,
    model: &str,
) -> PyResult<Bound<'py, PyDict>> {
    for (name, value) in [("delta", delta), ("gamma", gamma), ("vega", vega)] {
        checked_closed_form_value(value, &format!("{model} {name}")).map_err(core_to_py)?;
    }
    let out = PyDict::new(py);
    out.set_item("delta", delta)?;
    out.set_item("gamma", gamma)?;
    out.set_item("vega", vega)?;
    Ok(out)
}

// black_shifted_price / black_shifted_vega

/// Shifted (displaced) Black undiscounted per-unit price for negative-rate markets.
///
/// Prices ``Black(forward + shift, strike + shift, vol, expiry)`` so that
/// negative forwards and strikes remain in the lognormal domain.
///
/// Parameters
/// ----------
/// forward : float
///     Forward rate ``F`` at expiry (decimal; may be negative).
/// strike : float
///     Strike ``K`` (decimal, same units as ``forward``).
/// vol : float
///     Annualized shifted-lognormal volatility, decimal.
/// expiry : float
///     Time to expiry in years.
/// shift : float
///     Displacement added to both forward and strike, in the same rate
///     units as ``forward`` (e.g. ``0.03`` for a 3% shift); ``forward + shift``
///     and ``strike + shift`` must be positive.
/// is_call : bool
///     ``True`` for a call, ``False`` for a put.
///
/// Returns
/// -------
/// float
///     Undiscounted per-unit option value in the units of ``forward``.
///
/// Raises
/// ------
/// ValueError
///     If the inputs produce a non-finite price (for example a shifted
///     forward or strike that is not positive).
///
/// Examples
/// --------
/// >>> from finstack_quant.models import black_shifted_price
/// >>> round(black_shifted_price(-0.005, -0.005, 0.25, 1.0, 0.03, True), 6)
/// 0.002486
#[pyfunction(name = "black_shifted_price")]
#[pyo3(signature = (forward, strike, vol, expiry, shift, is_call))]
fn black_shifted_price_wrapper(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
    is_call: bool,
) -> PyResult<f64> {
    let value = if is_call {
        black_shifted_call(forward, strike, vol, expiry, shift)
    } else {
        black_shifted_put(forward, strike, vol, expiry, shift)
    };
    checked_closed_form_value(value, "shifted Black price").map_err(core_to_py)
}

/// Shifted (displaced) Black vega per unit (1.0) change in ``vol``.
///
/// Parameters
/// ----------
/// forward : float
///     Forward rate ``F`` at expiry (decimal; may be negative).
/// strike : float
///     Strike ``K`` (decimal, same units as ``forward``).
/// vol : float
///     Annualized shifted-lognormal volatility, decimal.
/// expiry : float
///     Time to expiry in years.
/// shift : float
///     Displacement added to both forward and strike, in rate units.
///
/// Returns
/// -------
/// float
///     Undiscounted vega in the units of ``forward`` per unit vol.
///
/// Raises
/// ------
/// ValueError
///     If the inputs produce a non-finite vega.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import black_shifted_vega
/// >>> black_shifted_vega(-0.005, -0.005, 0.25, 1.0, 0.03) > 0
/// True
#[pyfunction(name = "black_shifted_vega")]
#[pyo3(signature = (forward, strike, vol, expiry, shift))]
fn black_shifted_vega_wrapper(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
) -> PyResult<f64> {
    checked_closed_form_value(
        black_shifted_vega(forward, strike, vol, expiry, shift),
        "shifted Black vega",
    )
    .map_err(core_to_py)
}

// barrier_call / barrier_put

/// Reiner-Rubinstein continuous-monitoring barrier call price.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price ``S``.
/// strike : float
///     Strike price ``K``.
/// barrier : float
///     Barrier level.
/// rate : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// div_yield : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// vol : float
///     Annualized volatility (decimal, e.g. ``0.20`` for 20%).
/// expiry : float
///     Time to expiry in years.
/// direction : str
///     ``"up"`` or ``"down"`` (relative to spot / barrier).
/// knock : str
///     ``"in"`` (knock-in) or ``"out"`` (knock-out).
///
/// Returns
/// -------
/// float
///     Per-unit option price.
///
/// Raises
/// ------
/// ValueError
///     If ``direction`` / ``knock`` is unsupported or the price is non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import barrier_call
/// >>> round(barrier_call(100.0, 100.0, 120.0, 0.05, 0.0, 0.2, 1.0, "up", "out"), 4)
/// 1.1761
///
/// Sources
/// -------
/// - Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991
#[pyfunction(name = "barrier_call")]
#[pyo3(signature = (spot, strike, barrier, rate, div_yield, vol, expiry, direction, knock))]
#[allow(clippy::too_many_arguments)]
fn barrier_call_wrapper(
    spot: f64,
    strike: f64,
    barrier: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    direction: &str,
    knock: &str,
) -> PyResult<f64> {
    barrier_call_str(
        spot, strike, barrier, expiry, rate, div_yield, vol, direction, knock,
    )
    .map_err(core_to_py)
}

/// Reiner-Rubinstein continuous-monitoring barrier put price.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price ``S``.
/// strike : float
///     Strike price ``K``.
/// barrier : float
///     Barrier level.
/// rate : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// div_yield : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// vol : float
///     Annualized volatility (decimal, e.g. ``0.20`` for 20%).
/// expiry : float
///     Time to expiry in years.
/// direction : str
///     ``"up"`` or ``"down"`` (relative to spot / barrier).
/// knock : str
///     ``"in"`` (knock-in) or ``"out"`` (knock-out).
///
/// Returns
/// -------
/// float
///     Per-unit option price.
///
/// Raises
/// ------
/// ValueError
///     If ``direction`` / ``knock`` is unsupported or the price is non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import barrier_put
/// >>> barrier_put(100.0, 100.0, 80.0, 0.05, 0.0, 0.2, 1.0, "down", "out") > 0
/// True
///
/// Sources
/// -------
/// - Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991
#[pyfunction(name = "barrier_put")]
#[pyo3(signature = (spot, strike, barrier, rate, div_yield, vol, expiry, direction, knock))]
#[allow(clippy::too_many_arguments)]
fn barrier_put_wrapper(
    spot: f64,
    strike: f64,
    barrier: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    direction: &str,
    knock: &str,
) -> PyResult<f64> {
    barrier_put_str(
        spot, strike, barrier, expiry, rate, div_yield, vol, direction, knock,
    )
    .map_err(core_to_py)
}

/// Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option price.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price ``S``.
/// strike : float
///     Strike price ``K``.
/// rate : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// div_yield : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// vol : float
///     Annualized volatility (decimal, e.g. ``0.20`` for 20%).
/// expiry : float
///     Time to expiry in years.
/// num_fixings : int
///     Number of averaging fixings.
/// averaging : str, optional
///     ``"arithmetic"`` (Turnbull-Wakeman, default) or ``"geometric"``
///     (Kemna-Vorst exact).
/// is_call : bool, optional
///     ``True`` for call (default), ``False`` for put.
///
/// Returns
/// -------
/// float
///     Per-unit option price.
///
/// Raises
/// ------
/// ValueError
///     If ``averaging`` is unsupported or the price is non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import asian_option_price
/// >>> round(asian_option_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, 12), 4)
/// 6.1742
///
/// Sources
/// -------
/// - Kemna-Vorst (1990): see docs/REFERENCES.md#kemna-vorst-1990
/// - Turnbull-Wakeman (1991): see docs/REFERENCES.md#turnbull-wakeman-1991
#[pyfunction(name = "asian_option_price")]
#[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_fixings, averaging="arithmetic", is_call=true))]
#[allow(clippy::too_many_arguments)]
fn asian_option_wrapper(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_fixings: usize,
    averaging: &str,
    is_call: bool,
) -> PyResult<f64> {
    asian_option_price_str(
        spot,
        strike,
        expiry,
        rate,
        div_yield,
        vol,
        num_fixings,
        averaging,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

/// Conze-Viswanathan lookback option price.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price ``S``.
/// strike : float
///     Strike price ``K``. Ignored when ``strike_type`` is ``"floating"``.
/// rate : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// div_yield : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// vol : float
///     Annualized volatility (decimal, e.g. ``0.20`` for 20%).
/// expiry : float
///     Time to expiry in years.
/// extremum : float
///     Observed historical extremum — max for fixed-strike call / floating-
///     strike put, min for fixed-strike put / floating-strike call. For a
///     fresh option with no observation, use ``spot``.
/// strike_type : str, optional
///     ``"fixed"`` (default) or ``"floating"``.
/// is_call : bool, optional
///     ``True`` for call (default), ``False`` for put.
///
/// Returns
/// -------
/// float
///     Per-unit option price.
///
/// Raises
/// ------
/// ValueError
///     If ``strike_type`` is unsupported or the price is non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import lookback_option_price
/// >>> round(lookback_option_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, 100.0), 4)
/// 19.1676
///
/// Sources
/// -------
/// - Conze-Viswanathan (1991): see docs/REFERENCES.md#conze-viswanathan-1991
#[pyfunction(name = "lookback_option_price")]
#[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, extremum, strike_type="fixed", is_call=true))]
#[allow(clippy::too_many_arguments)]
fn lookback_option_wrapper(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    extremum: f64,
    strike_type: &str,
    is_call: bool,
) -> PyResult<f64> {
    lookback_option_price_str(
        spot,
        strike,
        expiry,
        rate,
        div_yield,
        vol,
        extremum,
        strike_type,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

/// Quanto option (cross-currency, FX-adjusted) price in domestic currency.
///
/// Parameters
/// ----------
/// spot : float
///     Spot price of the foreign asset in foreign currency.
/// strike : float
///     Strike in foreign currency.
/// expiry : float
///     Time to expiry in years.
/// rate_domestic, rate_foreign : float
///     Continuously-compounded domestic and foreign rates (decimal).
/// div_yield : float
///     Foreign asset dividend yield (decimal).
/// vol_asset : float
///     Foreign asset volatility (decimal).
/// vol_fx : float
///     Domestic/foreign FX volatility (decimal).
/// correlation : float
///     Correlation between asset and FX returns (``[-1, 1]``).
/// is_call : bool, optional
///     ``True`` for call (default), ``False`` for put.
///
/// Returns
/// -------
/// float
///     Per-unit option price in domestic currency.
///
/// Raises
/// ------
/// ValueError
///     If the inputs produce a non-finite price.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import quanto_option_price
/// >>> round(quanto_option_price(100.0, 100.0, 1.0, 0.05, 0.02, 0.01, 0.2, 0.1, 0.3), 4)
/// 7.7844
///
/// Sources
/// -------
/// - Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983
#[pyfunction(name = "quanto_option_price")]
#[pyo3(signature = (spot, strike, expiry, rate_domestic, rate_foreign, div_yield, vol_asset, vol_fx, correlation, is_call=true))]
#[allow(clippy::too_many_arguments)]
fn quanto_option_wrapper(
    spot: f64,
    strike: f64,
    expiry: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
    vol_fx: f64,
    correlation: f64,
    is_call: bool,
) -> PyResult<f64> {
    quanto_option_price(
        spot,
        strike,
        expiry,
        rate_domestic,
        rate_foreign,
        div_yield,
        vol_asset,
        vol_fx,
        correlation,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

// heston_price

/// Closed-form (Fourier) Heston price of a European option.
///
/// Semi-analytical Heston (1993) price via the Lord-Kahl / Albrecher stable
/// characteristic-function branch with adaptive Gauss-Legendre quadrature.
/// Puts use put-call parity.
///
/// Parameters
/// ----------
/// spot : float
///     Current spot price ``S``.
/// strike : float
///     Strike price ``K``.
/// expiry : float
///     Time to expiry in years; ``expiry <= 0`` returns intrinsic value.
/// rate : float
///     Domestic / risk-free rate (continuously compounded, decimal).
/// div_yield : float
///     Dividend yield or foreign rate (continuously compounded, decimal).
/// kappa : float
///     Mean-reversion speed of the variance process (per year, positive).
/// theta : float
///     Long-run variance level (variance units, positive).
/// sigma_v : float
///     Volatility of variance (vol-of-vol, positive).
/// rho : float
///     Spot/variance correlation in ``(-1, 1)``.
/// v0 : float
///     Initial instantaneous variance (variance, not volatility; positive).
/// is_call : bool, optional
///     ``True`` for a call (default), ``False`` for a put.
///
/// Returns
/// -------
/// float
///     Present-value per-unit option price.
///
/// Raises
/// ------
/// ValueError
///     If a Heston parameter or rate is non-finite or outside its domain.
/// RuntimeError
///     If the Fourier integration fails to produce a finite price.
///
/// Examples
/// --------
/// >>> from finstack_quant.models import heston_price
/// >>> p = heston_price(100.0, 100.0, 1.0, 0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04)
/// >>> 5.0 < p < 15.0
/// True
///
/// Sources
/// -------
/// - Heston (1993): see docs/REFERENCES.md#heston-1993
/// - Albrecher et al. (2007): see docs/REFERENCES.md#albrecher-2007-little-heston-trap
#[pyfunction(name = "heston_price")]
#[pyo3(signature = (spot, strike, expiry, rate, div_yield, kappa, theta, sigma_v, rho, v0, is_call=true))]
#[allow(clippy::too_many_arguments)]
fn heston_price_wrapper(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    expiry: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    sigma_v: f64,
    rho: f64,
    v0: f64,
    is_call: bool,
) -> PyResult<f64> {
    let params = HestonPricingParams::new(rate, div_yield, kappa, theta, sigma_v, rho, v0)
        .map_err(core_to_py)?;
    py.detach(move || {
        if is_call {
            heston_call_price_fourier(spot, strike, expiry, &params, None)
        } else {
            heston_put_price_fourier(spot, strike, expiry, &params, None)
        }
    })
    .map_err(core_to_py)
}

/// Register the analytic option primitives on the models submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBsGreeks>()?;
    m.add_function(wrap_pyfunction!(bs_price_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(vanilla_expiry_payoff_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(bs_greeks_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(bs_implied_vol_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(black76_implied_vol_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(black76_price_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(black76_greeks_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(bachelier_price_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(bachelier_greeks_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(black_shifted_price_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(black_shifted_vega_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(barrier_call_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(barrier_put_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(asian_option_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(lookback_option_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(quanto_option_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(heston_price_wrapper, m)?)?;
    Ok(())
}
