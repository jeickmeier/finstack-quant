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
//! - `bs_greeks` and the barrier / Asian / lookback / quanto prices take the
//!   serde form of the Rust `OptionMarketParams` as one `market` dict;
//!   `heston_price` takes the option contract and Heston variance dynamics separately.

use crate::bindings::core::table::{PyArrowTable, PyLabeledSeries};
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::module_utils::{opt_spec_from_py, spec_from_py};
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::core_to_py;
use finstack_quant_core::wire::serde_parse;
use finstack_quant_models::closed_form::{
    BsGreeks, DEFAULT_ASIAN_AVERAGING, DEFAULT_LOOKBACK_STRIKE_TYPE, DEFAULT_THETA_DAYS_PER_YEAR,
    ForwardGreeks, HestonFourierSettings, OptionCalculation, OptionCalculationResult, OptionGreeks,
    asian_option_price, bachelier_greeks, bachelier_price, barrier_option_price,
    black_shifted_price, black_shifted_vega, black76_greeks, black76_implied_vol, black76_price,
    bs_greeks, bs_implied_vol, bs_price, heston_price, lookback_option_price, option_calculation,
    quanto_option_price, vanilla_expiry_payoff,
};
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::{OptionContractParams, OptionMarketParams, OptionType};
use finstack_quant_models::volatility::heston::HestonParams;
use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::prelude::*;

/// Extract a typed market, or parse it from a dictionary or canonical JSON.
///
/// Shared by the closed-form and Monte Carlo option bindings.
pub(crate) fn option_market(market: &Bound<'_, PyAny>) -> PyResult<OptionMarketParams> {
    if let Ok(typed) = market.cast::<PyOptionMarketParams>() {
        return Ok(typed.borrow().inner.clone());
    }
    spec_from_py(market.py(), market, "OptionMarketParams")
}

#[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams")]
#[pyclass(
    name = "OptionMarketParams",
    module = "finstack_quant.models",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
struct PyOptionMarketParams {
    inner: OptionMarketParams,
}

#[pymethods]
impl PyOptionMarketParams {
    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.__init__")]
    #[new]
    #[pyo3(signature = (*, spot, strike, rate, dividend_yield, volatility, time_to_expiry, option_type))]
    fn new(
        spot: f64,
        strike: f64,
        rate: f64,
        dividend_yield: f64,
        volatility: f64,
        time_to_expiry: f64,
        option_type: &str,
    ) -> PyResult<Self> {
        let inner = OptionMarketParams {
            spot,
            strike,
            rate,
            volatility,
            time_to_expiry,
            dividend_yield,
            option_type: serde_parse(option_type).map_err(core_to_py)?,
        };
        inner.validate_allow_zero_vol().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.contract")]
    fn contract(&self) -> PyOptionContractParams {
        PyOptionContractParams {
            inner: self.inner.contract(),
        }
    }

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.option_type")]
    #[getter]
    fn option_type(&self) -> String {
        self.inner.option_type.to_string()
    }
}

crate::bindings::macros::py_getters!(PyOptionMarketParams |market| {
    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.spot")]
    spot -> f64 = market.inner.spot;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.strike")]
    strike -> f64 = market.inner.strike;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.rate")]
    rate -> f64 = market.inner.rate;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.dividend_yield")]
    dividend_yield -> f64 = market.inner.dividend_yield;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.volatility")]
    volatility -> f64 = market.inner.volatility;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionMarketParams.time_to_expiry")]
    time_to_expiry -> f64 = market.inner.time_to_expiry;

});

pub(crate) fn option_contract(contract: &Bound<'_, PyAny>) -> PyResult<OptionContractParams> {
    if let Ok(typed) = contract.cast::<PyOptionContractParams>() {
        return Ok(typed.get().inner);
    }
    spec_from_py(contract.py(), contract, "OptionContractParams")
}

#[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams")]
#[pyclass(
    name = "OptionContractParams",
    module = "finstack_quant.models",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
struct PyOptionContractParams {
    inner: OptionContractParams,
}

#[pymethods]
impl PyOptionContractParams {
    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams.__init__")]
    #[new]
    #[pyo3(signature = (*, spot, strike, rate, dividend_yield, time_to_expiry, option_type))]
    fn new(
        spot: f64,
        strike: f64,
        rate: f64,
        dividend_yield: f64,
        time_to_expiry: f64,
        option_type: &str,
    ) -> PyResult<Self> {
        let inner = OptionContractParams {
            spot,
            strike,
            rate,
            time_to_expiry,
            dividend_yield,
            option_type: serde_parse(option_type).map_err(core_to_py)?,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams.with_volatility")]
    fn with_volatility(&self, volatility: f64) -> PyResult<PyOptionMarketParams> {
        let inner = self.inner.with_volatility(volatility).map_err(core_to_py)?;
        Ok(PyOptionMarketParams { inner })
    }

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams.option_type")]
    #[getter]
    fn option_type(&self) -> String {
        self.inner.option_type.to_string()
    }
}

crate::bindings::macros::py_getters!(PyOptionContractParams |market| {
    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams.spot")]
    spot -> f64 = market.inner.spot;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams.strike")]
    strike -> f64 = market.inner.strike;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams.rate")]
    rate -> f64 = market.inner.rate;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams.dividend_yield")]
    dividend_yield -> f64 = market.inner.dividend_yield;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionContractParams.time_to_expiry")]
    time_to_expiry -> f64 = market.inner.time_to_expiry;

});

const BS_GREEK_LABELS: [&str; 6] = ["delta", "gamma", "vega", "theta", "rho_r", "rho_q"];

// BsGreeks

#[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks")]
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
    #[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks.is_valid")]
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    #[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks.to_series")]
    fn to_series(&self) -> PyResult<PyLabeledSeries> {
        let labels: Vec<String> = BS_GREEK_LABELS.iter().map(|s| (*s).to_string()).collect();
        Ok(PyLabeledSeries::from_f64("bs_greeks", labels, self.values())?.with_index_name("greek"))
    }

    fn __repr__(&self) -> String {
        repr_from_serde("BsGreeks", &self.inner)
    }
}

py_getters!(PyBsGreeks |s| {
    #[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks.delta")]
    delta -> f64 = s.inner.delta;

    #[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks.gamma")]
    gamma -> f64 = s.inner.gamma;

    #[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks.vega")]
    vega -> f64 = s.inner.vega;

    #[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks.theta")]
    theta -> f64 = s.inner.theta;

    #[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks.rho_r")]
    rho_r -> f64 = s.inner.rho_r;

    #[doc = pydoc!("finstack_quant.models._closed_form", "BsGreeks.rho_q")]
    rho_q -> f64 = s.inner.rho_q;
});

tabular_methods!(
    PyBsGreeks,
    "BsGreeks",
    module = "finstack_quant.models._closed_form"
);
wire_methods!(
    PyBsGreeks,
    BsGreeks,
    "BsGreeks",
    module = "finstack_quant.models._closed_form"
);

impl PyBsGreeks {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("delta", vec![self.inner.delta])?
            .f64("gamma", vec![self.inner.gamma])?
            .f64("vega", vec![self.inner.vega])?
            .f64("theta", vec![self.inner.theta])?
            .f64("rho_r", vec![self.inner.rho_r])?
            .f64("rho_q", vec![self.inner.rho_q])?
            .build()
    }

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

#[doc = pydoc!("finstack_quant.models._closed_form", "ForwardGreeks")]
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
    #[doc = pydoc!("finstack_quant.models._closed_form", "ForwardGreeks.to_series")]
    fn to_series(&self) -> PyResult<PyLabeledSeries> {
        let labels: Vec<String> = FORWARD_GREEK_LABELS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let values = vec![self.inner.delta, self.inner.gamma, self.inner.vega];
        Ok(PyLabeledSeries::from_f64("forward_greeks", labels, values)?.with_index_name("greek"))
    }

    fn __repr__(&self) -> String {
        repr_from_serde("ForwardGreeks", &self.inner)
    }
}

py_getters!(PyForwardGreeks |s| {
    #[doc = pydoc!("finstack_quant.models._closed_form", "ForwardGreeks.delta")]
    delta -> f64 = s.inner.delta;

    #[doc = pydoc!("finstack_quant.models._closed_form", "ForwardGreeks.gamma")]
    gamma -> f64 = s.inner.gamma;

    #[doc = pydoc!("finstack_quant.models._closed_form", "ForwardGreeks.vega")]
    vega -> f64 = s.inner.vega;
});

impl PyForwardGreeks {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("delta", vec![self.inner.delta])?
            .f64("gamma", vec![self.inner.gamma])?
            .f64("vega", vec![self.inner.vega])?
            .build()
    }
}

tabular_methods!(
    PyForwardGreeks,
    "ForwardGreeks",
    module = "finstack_quant.models._closed_form"
);
wire_methods!(
    PyForwardGreeks,
    ForwardGreeks,
    "ForwardGreeks",
    module = "finstack_quant.models._closed_form"
);

// OptionCalculationResult

#[doc = pydoc!("finstack_quant.models._closed_form", "OptionCalculationResult")]
#[pyclass(
    name = "OptionCalculationResult",
    module = "finstack_quant.models",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyOptionCalculationResult {
    pub(crate) inner: OptionCalculationResult,
}

py_getters!(PyOptionCalculationResult |s| {
    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionCalculationResult.price")]
    price -> f64 = s.inner.price;

    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionCalculationResult.implied_vol")]
    implied_vol -> f64 = s.inner.implied_vol;
});

#[pymethods]
impl PyOptionCalculationResult {
    #[doc = pydoc!("finstack_quant.models._closed_form", "OptionCalculationResult.greeks")]
    #[getter]
    fn greeks(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self.inner.greeks {
            OptionGreeks::BlackScholes(greeks) => {
                Ok(Py::new(py, PyBsGreeks::from_inner(greeks))?.into_any())
            }
            OptionGreeks::Forward(inner) => Ok(Py::new(py, PyForwardGreeks { inner })?.into_any()),
        }
    }

    fn __repr__(&self) -> String {
        repr_from_serde("OptionCalculationResult", &self.inner)
    }
}

wire_methods!(
    PyOptionCalculationResult,
    OptionCalculationResult,
    "OptionCalculationResult",
    module = "finstack_quant.models._closed_form"
);

#[doc = pydoc!("finstack_quant.models._closed_form", "option_calculation")]
#[pyfunction(name = "option_calculation")]
#[pyo3(text_signature = "(request)")]
fn option_calculation_wrapper(request: &Bound<'_, PyAny>) -> PyResult<PyOptionCalculationResult> {
    let request: OptionCalculation =
        crate::bindings::module_utils::spec_from_py(request.py(), request, "OptionCalculation")?;
    Ok(PyOptionCalculationResult {
        inner: option_calculation(&request).map_err(core_to_py)?,
    })
}

fn option_axis(obj: Option<&Bound<'_, PyAny>>, label: &str) -> PyResult<Option<Vec<f64>>> {
    let Some(obj) = obj else {
        return Ok(None);
    };
    if let Ok(value) = obj.extract::<f64>() {
        return Ok(Some(vec![value]));
    }
    let py = obj.py();
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("dtype", "float64")?;
    let array = py
        .import("numpy")?
        .getattr("asarray")?
        .call((obj,), Some(&kwargs))?;
    let array = array.extract::<PyReadonlyArray1<'_, f64>>().map_err(|_| {
        crate::errors::value_error(format!(
            "{label} must be a scalar or a one-dimensional array"
        ))
    })?;
    Ok(Some(array.as_array().iter().copied().collect()))
}

#[doc = pydoc!("finstack_quant.models._closed_form", "bs_price_many")]
#[pyfunction]
#[pyo3(signature = (market, spots=None, strikes=None))]
fn bs_price_many<'py>(
    market: &Bound<'py, PyAny>,
    spots: Option<&Bound<'py, PyAny>>,
    strikes: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyArray1<f64>>> {
    let py = market.py();
    let market = option_market(market)?;
    let spots = option_axis(spots, "spots")?;
    let strikes = option_axis(strikes, "strikes")?;
    py.detach(|| {
        finstack_quant_models::closed_form::bs_price_many(
            &market,
            spots.as_deref(),
            strikes.as_deref(),
        )
    })
    .map(|prices| prices.into_pyarray(py))
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models._closed_form", "bs_greeks_many")]
#[pyfunction]
#[pyo3(
    signature = (market, spots=None, strikes=None, theta_days_per_year=DEFAULT_THETA_DAYS_PER_YEAR),
    text_signature = "(market, spots=None, strikes=None, theta_days_per_year=365.0)"
)]
fn bs_greeks_many(
    market: &Bound<'_, PyAny>,
    spots: Option<&Bound<'_, PyAny>>,
    strikes: Option<&Bound<'_, PyAny>>,
    theta_days_per_year: f64,
) -> PyResult<Vec<PyBsGreeks>> {
    let py = market.py();
    let market = option_market(market)?;
    let spots = option_axis(spots, "spots")?;
    let strikes = option_axis(strikes, "strikes")?;
    py.detach(|| {
        finstack_quant_models::closed_form::bs_greeks_many(
            &market,
            spots.as_deref(),
            strikes.as_deref(),
            theta_days_per_year,
        )
    })
    .map(|rows| rows.into_iter().map(PyBsGreeks::from_inner).collect())
    .map_err(core_to_py)
}

// bs_price

#[doc = pydoc!("finstack_quant.models._closed_form", "bs_price")]
#[pyfunction(name = "bs_price")]
fn bs_price_wrapper(market: &Bound<'_, PyAny>) -> PyResult<f64> {
    bs_price(&option_market(market)?).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models._closed_form", "vanilla_expiry_payoff")]
#[pyfunction(name = "vanilla_expiry_payoff")]
#[pyo3(signature = (spot, strike, is_call))]
fn vanilla_expiry_payoff_wrapper(spot: f64, strike: f64, is_call: bool) -> PyResult<f64> {
    vanilla_expiry_payoff(spot, strike, OptionType::from(is_call)).map_err(core_to_py)
}

// bs_greeks

#[doc = pydoc!("finstack_quant.models._closed_form", "bs_greeks")]
#[pyfunction(name = "bs_greeks")]
#[pyo3(
    signature = (market, theta_days_per_year=DEFAULT_THETA_DAYS_PER_YEAR),
    text_signature = "(market, theta_days_per_year=365.0)"
)]
fn bs_greeks_wrapper(market: &Bound<'_, PyAny>, theta_days_per_year: f64) -> PyResult<PyBsGreeks> {
    // theta_days_per_year validation (finite, > 0) lives in `bs_greeks`.
    bs_greeks(&option_market(market)?, theta_days_per_year)
        .map(PyBsGreeks::from_inner)
        .map_err(core_to_py)
}

// bs_implied_vol

#[doc = pydoc!("finstack_quant.models._closed_form", "bs_implied_vol")]
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

#[doc = pydoc!("finstack_quant.models._closed_form", "black76_implied_vol")]
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

#[doc = pydoc!("finstack_quant.models._closed_form", "black76_price")]
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

#[doc = pydoc!("finstack_quant.models._closed_form", "black76_greeks")]
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

#[doc = pydoc!("finstack_quant.models._closed_form", "bachelier_price")]
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

#[doc = pydoc!("finstack_quant.models._closed_form", "bachelier_greeks")]
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

#[doc = pydoc!("finstack_quant.models._closed_form", "black_shifted_price")]
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

#[doc = pydoc!("finstack_quant.models._closed_form", "black_shifted_vega")]
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

#[doc = pydoc!("finstack_quant.models._closed_form", "barrier_option_price")]
#[pyfunction(name = "barrier_option_price")]
#[pyo3(signature = (market, barrier, direction, knock))]
fn barrier_option_price_wrapper(
    market: &Bound<'_, PyAny>,
    barrier: f64,
    direction: &str,
    knock: &str,
) -> PyResult<f64> {
    barrier_option_price(&option_market(market)?, barrier, direction, knock).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models._closed_form", "asian_option_price")]
#[pyfunction(name = "asian_option_price")]
#[pyo3(signature = (market, num_fixings, averaging=None))]
fn asian_option_wrapper(
    market: &Bound<'_, PyAny>,
    num_fixings: usize,
    averaging: Option<&str>,
) -> PyResult<f64> {
    let market = option_market(market)?;
    let averaging = averaging
        .map(serde_parse::<AveragingMethod>)
        .transpose()
        .map_err(core_to_py)?
        .unwrap_or(DEFAULT_ASIAN_AVERAGING);
    asian_option_price(&market, num_fixings, averaging).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models._closed_form", "lookback_option_price")]
#[pyfunction(name = "lookback_option_price")]
#[pyo3(signature = (market, extremum, strike_type=DEFAULT_LOOKBACK_STRIKE_TYPE))]
fn lookback_option_wrapper(
    market: &Bound<'_, PyAny>,
    extremum: f64,
    strike_type: &str,
) -> PyResult<f64> {
    lookback_option_price(&option_market(market)?, extremum, strike_type).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models._closed_form", "quanto_option_price")]
#[pyfunction(name = "quanto_option_price")]
#[pyo3(signature = (market, rate_foreign, vol_fx, correlation))]
fn quanto_option_wrapper(
    market: &Bound<'_, PyAny>,
    rate_foreign: f64,
    vol_fx: f64,
    correlation: f64,
) -> PyResult<f64> {
    quanto_option_price(&option_market(market)?, rate_foreign, vol_fx, correlation)
        .map_err(core_to_py)
}

/// Extract native Heston variance dynamics from the typed or structured host value.
pub(crate) fn heston_params(params: &Bound<'_, PyAny>) -> PyResult<HestonParams> {
    if let Ok(params) = params.cast::<PyHestonParams>() {
        return Ok(params.borrow().inner);
    }
    spec_from_py(params.py(), params, "HestonParams")
}

#[doc = pydoc!("finstack_quant.models._closed_form", "HestonParams")]
#[pyclass(
    name = "HestonParams",
    module = "finstack_quant.models",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
struct PyHestonParams {
    inner: HestonParams,
}

#[pymethods]
impl PyHestonParams {
    #[doc = pydoc!("finstack_quant.models._closed_form", "HestonParams.__init__")]
    #[new]
    #[pyo3(signature = (*, v0, kappa, theta, vol_of_vol, rho))]
    fn new(v0: f64, kappa: f64, theta: f64, vol_of_vol: f64, rho: f64) -> PyResult<Self> {
        let inner = HestonParams::new(v0, kappa, theta, vol_of_vol, rho).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.models._closed_form", "HestonParams.satisfies_feller")]
    fn satisfies_feller(&self) -> bool {
        self.inner.satisfies_feller()
    }

    fn __repr__(&self) -> String {
        repr_from_serde("HestonParams", &self.inner)
    }
}

py_getters!(PyHestonParams |s| {
    #[doc = pydoc!("finstack_quant.models._closed_form", "HestonParams.v0")]
    v0 -> f64 = s.inner.v0;

    #[doc = pydoc!("finstack_quant.models._closed_form", "HestonParams.kappa")]
    kappa -> f64 = s.inner.kappa;

    #[doc = pydoc!("finstack_quant.models._closed_form", "HestonParams.theta")]
    theta -> f64 = s.inner.theta;

    #[doc = pydoc!("finstack_quant.models._closed_form", "HestonParams.vol_of_vol")]
    vol_of_vol -> f64 = s.inner.vol_of_vol;

    #[doc = pydoc!("finstack_quant.models._closed_form", "HestonParams.rho")]
    rho -> f64 = s.inner.rho;
});

wire_methods!(
    PyHestonParams,
    HestonParams,
    "HestonParams",
    module = "finstack_quant.models._closed_form"
);

#[doc = pydoc!("finstack_quant.models._closed_form", "heston_price")]
#[pyfunction(name = "heston_price")]
#[pyo3(signature = (contract, heston, settings=None))]
fn heston_price_wrapper(
    py: Python<'_>,
    contract: &Bound<'_, PyAny>,
    heston: &Bound<'_, PyAny>,
    settings: Option<&Bound<'_, PyAny>>,
) -> PyResult<f64> {
    let contract = option_contract(contract)?;
    let heston = heston_params(heston)?;
    let settings: Option<HestonFourierSettings> =
        opt_spec_from_py(py, settings, "HestonFourierSettings")?;
    py.detach(move || heston_price(&contract, &heston, settings.as_ref()))
        .map_err(core_to_py)
}

/// Register the analytic option primitives on the models submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBsGreeks>()?;
    m.add_class::<PyOptionMarketParams>()?;
    m.add_class::<PyOptionContractParams>()?;
    m.add_class::<PyHestonParams>()?;
    m.add_class::<PyForwardGreeks>()?;
    m.add_class::<PyOptionCalculationResult>()?;
    m.add_function(wrap_pyfunction!(option_calculation_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(bs_price_wrapper, m)?)?;
    m.add_function(wrap_pyfunction!(bs_price_many, m)?)?;
    m.add_function(wrap_pyfunction!(bs_greeks_many, m)?)?;
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
