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
//! - Black-Scholes Greeks use canonical Rust scaling: `vega` and `rho_*` are
//!   per-1% move and `theta` is per day under ACT/365 (or 252 via
//!   `theta_days_per_year`). Forward-option Greeks are undiscounted and their
//!   `vega` is per unit volatility change.

use crate::bindings::macros::wire_methods;
use crate::bindings::pandas_utils::{
    labeled_values_to_series, serde_object_to_single_row_dataframe,
};
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;
use finstack_quant_core::wire::serde_parse;
use finstack_quant_models::closed_form::{
    asian_option_price, bachelier_greeks, bachelier_price, barrier_option_price, black76_greeks,
    black76_implied_vol, black76_price, black_shifted_price, black_shifted_vega, bs_greeks,
    bs_implied_vol, bs_price, heston_price, lookback_option_price, quanto_option_price,
    vanilla_expiry_payoff, BsGreeks, ForwardGreeks, HestonPricingParams, DEFAULT_ASIAN_AVERAGING,
    DEFAULT_LOOKBACK_STRIKE_TYPE, DEFAULT_THETA_DAYS_PER_YEAR,
};
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::OptionType;
use pyo3::prelude::*;

const BS_GREEK_LABELS: [&str; 6] = ["delta", "gamma", "vega", "theta", "rho_r", "rho_q"];

// BsGreeks

#[doc = pydoc!("finstack_quant.models", "BsGreeks")]
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
    #[doc = pydoc!("finstack_quant.models", "BsGreeks.delta")]
    #[getter]
    fn delta(&self) -> f64 {
        self.inner.delta
    }

    #[doc = pydoc!("finstack_quant.models", "BsGreeks.gamma")]
    #[getter]
    fn gamma(&self) -> f64 {
        self.inner.gamma
    }

    #[doc = pydoc!("finstack_quant.models", "BsGreeks.vega")]
    #[getter]
    fn vega(&self) -> f64 {
        self.inner.vega
    }

    #[doc = pydoc!("finstack_quant.models", "BsGreeks.theta")]
    #[getter]
    fn theta(&self) -> f64 {
        self.inner.theta
    }

    #[doc = pydoc!("finstack_quant.models", "BsGreeks.rho_r")]
    #[getter]
    fn rho_r(&self) -> f64 {
        self.inner.rho_r
    }

    #[doc = pydoc!("finstack_quant.models", "BsGreeks.rho_q")]
    #[getter]
    fn rho_q(&self) -> f64 {
        self.inner.rho_q
    }

    #[doc = pydoc!("finstack_quant.models", "BsGreeks.is_valid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    #[doc = pydoc!("finstack_quant.models", "BsGreeks.to_series")]
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = BS_GREEK_LABELS.iter().map(|s| (*s).to_string()).collect();
        labeled_values_to_series(py, &labels, self.values(), "bs_greeks")
    }

    #[doc = pydoc!("finstack_quant.models", "BsGreeks.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("BsGreeks", &self.inner)
    }
}

wire_methods!(
    PyBsGreeks,
    BsGreeks,
    "BsGreeks",
    module = "finstack_quant.models"
);

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

// ForwardGreeks

const FORWARD_GREEK_LABELS: [&str; 3] = ["delta", "gamma", "vega"];

#[doc = pydoc!("finstack_quant.models", "ForwardGreeks")]
#[pyclass(
    name = "ForwardGreeks",
    module = "finstack_quant.models",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyForwardGreeks {
    pub(crate) inner: ForwardGreeks,
}

#[pymethods]
impl PyForwardGreeks {
    #[doc = pydoc!("finstack_quant.models", "ForwardGreeks.delta")]
    #[getter]
    fn delta(&self) -> f64 {
        self.inner.delta
    }

    #[doc = pydoc!("finstack_quant.models", "ForwardGreeks.gamma")]
    #[getter]
    fn gamma(&self) -> f64 {
        self.inner.gamma
    }

    #[doc = pydoc!("finstack_quant.models", "ForwardGreeks.vega")]
    #[getter]
    fn vega(&self) -> f64 {
        self.inner.vega
    }

    #[doc = pydoc!("finstack_quant.models", "ForwardGreeks.to_series")]
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = FORWARD_GREEK_LABELS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let values = vec![self.inner.delta, self.inner.gamma, self.inner.vega];
        labeled_values_to_series(py, &labels, values, "forward_greeks")
    }

    #[doc = pydoc!("finstack_quant.models", "ForwardGreeks.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("ForwardGreeks", &self.inner)
    }
}

wire_methods!(
    PyForwardGreeks,
    ForwardGreeks,
    "ForwardGreeks",
    module = "finstack_quant.models"
);

// bs_price

#[doc = pydoc!("finstack_quant.models", "bs_price")]
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

#[doc = pydoc!("finstack_quant.models", "vanilla_expiry_payoff")]
#[pyfunction(name = "vanilla_expiry_payoff")]
#[pyo3(signature = (spot, strike, is_call))]
fn vanilla_expiry_payoff_wrapper(spot: f64, strike: f64, is_call: bool) -> PyResult<f64> {
    vanilla_expiry_payoff(spot, strike, OptionType::from(is_call)).map_err(core_to_py)
}

// bs_greeks

#[doc = pydoc!("finstack_quant.models", "bs_greeks")]
#[pyfunction(name = "bs_greeks")]
#[pyo3(
    signature = (spot, strike, rate, div_yield, vol, expiry, is_call, theta_days_per_year=DEFAULT_THETA_DAYS_PER_YEAR),
    text_signature = "(spot, strike, rate, div_yield, vol, expiry, is_call, theta_days_per_year=365.0)"
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
    theta_days_per_year: f64,
) -> PyResult<PyBsGreeks> {
    // theta_days_per_year validation (finite, > 0) lives in `bs_greeks`.
    bs_greeks(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        OptionType::from(is_call),
        theta_days_per_year,
    )
    .map(PyBsGreeks::from_inner)
    .map_err(core_to_py)
}

// bs_implied_vol

#[doc = pydoc!("finstack_quant.models", "bs_implied_vol")]
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
        price,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

// black76_implied_vol

#[doc = pydoc!("finstack_quant.models", "black76_implied_vol")]
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
        price,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

// black76_price / black76_greeks

#[doc = pydoc!("finstack_quant.models", "black76_price")]
#[pyfunction(name = "black76_price")]
#[pyo3(signature = (forward, strike, df, vol, expiry, is_call))]
fn black76_price_wrapper(
    forward: f64,
    strike: f64,
    df: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
) -> PyResult<f64> {
    black76_price(forward, strike, df, vol, expiry, OptionType::from(is_call)).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models", "black76_greeks")]
#[pyfunction(name = "black76_greeks")]
#[pyo3(signature = (forward, strike, vol, expiry, is_call))]
fn black76_greeks_wrapper(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
) -> PyResult<PyForwardGreeks> {
    black76_greeks(forward, strike, vol, expiry, OptionType::from(is_call))
        .map(|inner| PyForwardGreeks { inner })
        .map_err(core_to_py)
}

// bachelier_price / bachelier_greeks

#[doc = pydoc!("finstack_quant.models", "bachelier_price")]
#[pyfunction(name = "bachelier_price")]
#[pyo3(signature = (forward, strike, normal_vol, expiry, is_call))]
fn bachelier_price_wrapper(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    is_call: bool,
) -> PyResult<f64> {
    bachelier_price(
        forward,
        strike,
        normal_vol,
        expiry,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models", "bachelier_greeks")]
#[pyfunction(name = "bachelier_greeks")]
#[pyo3(signature = (forward, strike, normal_vol, expiry, is_call))]
fn bachelier_greeks_wrapper(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    is_call: bool,
) -> PyResult<PyForwardGreeks> {
    bachelier_greeks(
        forward,
        strike,
        normal_vol,
        expiry,
        OptionType::from(is_call),
    )
    .map(|inner| PyForwardGreeks { inner })
    .map_err(core_to_py)
}

// black_shifted_price / black_shifted_vega

#[doc = pydoc!("finstack_quant.models", "black_shifted_price")]
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
    black_shifted_price(
        forward,
        strike,
        vol,
        expiry,
        shift,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models", "black_shifted_vega")]
#[pyfunction(name = "black_shifted_vega")]
#[pyo3(signature = (forward, strike, vol, expiry, shift))]
fn black_shifted_vega_wrapper(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
) -> PyResult<f64> {
    black_shifted_vega(forward, strike, vol, expiry, shift).map_err(core_to_py)
}

// barrier_option_price

#[doc = pydoc!("finstack_quant.models", "barrier_option_price")]
#[pyfunction(name = "barrier_option_price")]
#[pyo3(signature = (spot, strike, barrier, rate, div_yield, vol, expiry, direction, knock, is_call=true))]
#[allow(clippy::too_many_arguments)]
fn barrier_option_price_wrapper(
    spot: f64,
    strike: f64,
    barrier: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    direction: &str,
    knock: &str,
    is_call: bool,
) -> PyResult<f64> {
    barrier_option_price(
        spot,
        strike,
        barrier,
        rate,
        div_yield,
        vol,
        expiry,
        direction,
        knock,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models", "asian_option_price")]
#[pyfunction(name = "asian_option_price")]
#[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_fixings, averaging=None, is_call=true))]
#[allow(clippy::too_many_arguments)]
fn asian_option_wrapper(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_fixings: usize,
    averaging: Option<&str>,
    is_call: bool,
) -> PyResult<f64> {
    let averaging = averaging
        .map(serde_parse::<AveragingMethod>)
        .transpose()
        .map_err(core_to_py)?
        .unwrap_or(DEFAULT_ASIAN_AVERAGING);
    asian_option_price(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_fixings,
        averaging,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models", "lookback_option_price")]
#[pyfunction(name = "lookback_option_price")]
#[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, extremum, strike_type=DEFAULT_LOOKBACK_STRIKE_TYPE, is_call=true))]
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
    lookback_option_price(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        extremum,
        strike_type,
        OptionType::from(is_call),
    )
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models", "quanto_option_price")]
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

#[doc = pydoc!("finstack_quant.models", "heston_price")]
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
    let option_type = OptionType::from(is_call);
    py.detach(move || heston_price(spot, strike, expiry, &params, option_type, None))
        .map_err(core_to_py)
}

/// Register the analytic option primitives on the models submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBsGreeks>()?;
    m.add_class::<PyForwardGreeks>()?;
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
    m.add_function(wrap_pyfunction!(barrier_option_price_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(asian_option_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(lookback_option_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(quanto_option_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(heston_price_wrapper, m)?)?;
    Ok(())
}
