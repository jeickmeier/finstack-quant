//! Python bindings for `finstack_quant_models::rates::hull_white`.
//!
//! Exposes the product-independent Hull-White one-factor parameter set and
//! the closed-form scalar kernels (convexity adjustment, bond-price
//! volatility, zero-coupon bond options, caplet normal-vol proxy, cap/floor
//! pricing). Quote preparation and fitting live in `finstack_quant.calibration`.

use crate::bindings::core::market_data::curves::PyDiscountCurve;
use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_models::rates::hull_white::{self, HullWhiteCalibrationParams, HullWhiteParams};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};

#[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams")]
#[pyclass(
    name = "HullWhiteParams",
    module = "finstack_quant.models.rates.hull_white",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyHullWhiteParams {
    pub(crate) inner: HullWhiteParams,
}

impl PyHullWhiteParams {
    pub(crate) fn from_inner(inner: HullWhiteParams) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyHullWhiteParams {
    #[new]
    #[pyo3(text_signature = "(kappa, sigma)")]
    fn new(kappa: f64, sigma: f64) -> PyResult<Self> {
        HullWhiteParams::constant(kappa, sigma)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams.piecewise")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, kappa, times, values)")]
    fn piecewise(
        _cls: &Bound<'_, PyType>,
        kappa: f64,
        times: Vec<f64>,
        values: Vec<f64>,
    ) -> PyResult<Self> {
        HullWhiteParams::piecewise(kappa, times, values)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams.kappa")]
    #[getter]
    fn kappa(&self) -> f64 {
        self.inner.kappa
    }

    #[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams.get_times")]
    fn get_times(&self) -> Vec<f64> {
        self.inner.get_times().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams.get_values")]
    fn get_values(&self) -> Vec<f64> {
        self.inner.get_values().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams.get_sigma")]
    #[pyo3(text_signature = "(self, t)")]
    fn get_sigma(&self, t: f64) -> f64 {
        self.inner.get_sigma(t)
    }

    #[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams.state_variance")]
    #[pyo3(text_signature = "(self, t)")]
    fn state_variance(&self, t: f64) -> PyResult<f64> {
        self.inner.state_variance(t).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams.state_covariance")]
    #[pyo3(text_signature = "(self, left_time, right_time)")]
    fn state_covariance(&self, left_time: f64, right_time: f64) -> PyResult<f64> {
        self.inner
            .state_covariance(left_time, right_time)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.hull_white", "HullWhiteParams.bond_vol")]
    #[pyo3(text_signature = "(self, t, expiry, maturity)")]
    fn bond_vol(&self, t: f64, expiry: f64, maturity: f64) -> PyResult<f64> {
        self.inner.bond_vol(t, expiry, maturity).map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "HullWhiteParams(kappa={:?}, times={:?}, values={:?})",
            self.inner.kappa,
            self.inner.get_times(),
            self.inner.get_values()
        )
    }
}

wire_methods!(
    PyHullWhiteParams,
    HullWhiteParams,
    "HullWhiteParams",
    module = "finstack_quant.models.rates.hull_white"
);

#[doc = pydoc!("finstack_quant.models.rates.hull_white", "hw1f_convexity_adjustment")]
#[pyfunction]
#[pyo3(text_signature = "(kappa, sigma, t_settle, t_end)")]
fn hw1f_convexity_adjustment(kappa: f64, sigma: f64, t_settle: f64, t_end: f64) -> f64 {
    hull_white::hw1f_convexity_adjustment(kappa, sigma, t_settle, t_end)
}

#[doc = pydoc!("finstack_quant.models.rates.hull_white", "hw_bond_vol")]
#[pyfunction]
#[pyo3(text_signature = "(kappa, sigma, t, expiry, maturity)")]
fn hw_bond_vol(kappa: f64, sigma: f64, t: f64, expiry: f64, maturity: f64) -> f64 {
    hull_white::hw_bond_vol(kappa, sigma, t, expiry, maturity)
}

#[doc = pydoc!("finstack_quant.models.rates.hull_white", "hw1f_zcb_option_price")]
#[pyfunction]
#[pyo3(text_signature = "(p0_expiry, p0_maturity, strike, bond_vol, is_call)")]
fn hw1f_zcb_option_price(
    p0_expiry: f64,
    p0_maturity: f64,
    strike: f64,
    bond_vol: f64,
    is_call: bool,
) -> f64 {
    hull_white::hw1f_zcb_option_price(p0_expiry, p0_maturity, strike, bond_vol, is_call)
}

#[doc = pydoc!("finstack_quant.models.rates.hull_white", "hw1f_caplet_forward_rate_normal_vol")]
#[pyfunction]
#[pyo3(text_signature = "(kappa, sigma, t_fix, accrual)")]
fn hw1f_caplet_forward_rate_normal_vol(kappa: f64, sigma: f64, t_fix: f64, accrual: f64) -> f64 {
    hull_white::hw1f_caplet_forward_rate_normal_vol(kappa, sigma, t_fix, accrual)
}

#[doc = pydoc!("finstack_quant.models.rates.hull_white", "hw1f_cap_floor_price")]
#[pyfunction]
#[pyo3(signature = (kappa, sigma, periods, strike, is_cap, discount_curve, forward_curve))]
#[pyo3(text_signature = "(kappa, sigma, periods, strike, is_cap, discount_curve, forward_curve)")]
#[allow(clippy::too_many_arguments)]
fn hw1f_cap_floor_price(
    kappa: f64,
    sigma: f64,
    periods: Vec<(f64, f64, f64)>,
    strike: f64,
    is_cap: bool,
    discount_curve: PyRef<'_, PyDiscountCurve>,
    forward_curve: PyRef<'_, PyDiscountCurve>,
) -> PyResult<f64> {
    let params = HullWhiteCalibrationParams::new(kappa, sigma).map_err(core_to_py)?;
    let discount = std::sync::Arc::clone(&discount_curve.inner);
    let forward = std::sync::Arc::clone(&forward_curve.inner);
    let discount_df = |t: f64| discount.df(t);
    let forward_df = |t: f64| forward.df(t);
    Ok(hull_white::hw1f_cap_floor_price(
        params,
        &discount_df,
        &forward_df,
        &periods,
        strike,
        is_cap,
    ))
}

/// Build the `finstack_quant.models.rates.hull_white` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "hull_white")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.rates.hull_white", "__module__"),
    )?;
    m.add_class::<PyHullWhiteParams>()?;
    m.add_function(wrap_pyfunction!(hw1f_cap_floor_price, &m)?)?;
    m.add_function(wrap_pyfunction!(hw1f_caplet_forward_rate_normal_vol, &m)?)?;
    m.add_function(wrap_pyfunction!(hw1f_convexity_adjustment, &m)?)?;
    m.add_function(wrap_pyfunction!(hw1f_zcb_option_price, &m)?)?;
    m.add_function(wrap_pyfunction!(hw_bond_vol, &m)?)?;
    let all = PyList::new(
        py,
        [
            "HullWhiteParams",
            "hw1f_cap_floor_price",
            "hw1f_caplet_forward_rate_normal_vol",
            "hw1f_convexity_adjustment",
            "hw1f_zcb_option_price",
            "hw_bond_vol",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
