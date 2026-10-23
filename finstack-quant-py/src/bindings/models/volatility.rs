//! Product-independent volatility model and evaluator bindings.
//!
//! Units: SABR ``alpha`` is the initial (Black or normal, depending on
//! ``beta``) volatility as a decimal, ``nu`` is vol-of-vol as a decimal,
//! ``rho`` is a correlation in ``[-1, 1]`` and ``shift`` is a displacement in
//! the forward's rate units. Normal (Bachelier) vols are absolute, in the
//! forward's units; Black vols are decimals.
//!
//! Exposes three classes from `finstack_quant_models::volatility::sabr`:
//!
//! * [`SabrParameters`] — the four canonical parameters `(alpha, beta, nu, rho)`
//!   plus an optional `shift` for negative-rate environments, with
//!   `implied_vol(forward, strike, t)`.
//! * [`SabrSmile`] — fixes a forward and expiry, provides `implied_vol(strike)`, bulk
//!   smile generation, and optional no-arbitrage diagnostics.
//! * [`SabrCalibrator`] — Levenberg-Marquardt calibration to market vols with
//!   beta fixed (standard quant convention).
//!
//! # Naming note
//!
//! Rust and Python use the canonical PascalCase forms (`SabrParameters`,
//! `SabrSmile`, `SabrCalibrator`).

use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use std::sync::Arc;

use crate::bindings::core::market_data::curves::{
    PyFxDeltaVolSurface, PyVolCube, PyVolCubeExpirySlice, PyVolSurface,
};
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;
use finstack_quant_models::volatility as vol;
use finstack_quant_models::volatility::local_vol::LocalVolSurface;
use finstack_quant_models::volatility::sabr::{
    SabrCalibrationOutcome, SabrCalibrator, SabrParameters, SabrShift, SabrSmile,
};
use finstack_quant_models::volatility::svi::{calibrate_svi as rust_calibrate_svi, SviParams};
use finstack_quant_models::volatility::VolatilityConvention;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

#[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters")]
#[pyclass(
    name = "SabrParameters",
    module = "finstack_quant.models.volatility",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PySabrParameters {
    pub(crate) inner: SabrParameters,
}

#[pymethods]
impl PySabrParameters {
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.__init__")]
    #[new]
    #[pyo3(signature = (alpha, beta, nu, rho, shift=None))]
    fn new(alpha: f64, beta: f64, nu: f64, rho: f64, shift: Option<f64>) -> PyResult<Self> {
        let inner = match shift {
            Some(s) => SabrParameters::new_with_shift(alpha, beta, nu, rho, s),
            None => SabrParameters::new(alpha, beta, nu, rho),
        }
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.equity_default")]
    #[staticmethod]
    fn equity_default() -> Self {
        Self {
            inner: SabrParameters::equity_default(),
        }
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.rates_default")]
    #[staticmethod]
    fn rates_default() -> Self {
        Self {
            inner: SabrParameters::rates_default(),
        }
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.rates_standard")]
    #[staticmethod]
    fn rates_standard(alpha: f64, nu: f64, rho: f64) -> PyResult<Self> {
        SabrParameters::rates_standard(alpha, nu, rho)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.normal")]
    #[staticmethod]
    fn normal(alpha: f64, nu: f64, rho: f64) -> PyResult<Self> {
        SabrParameters::normal(alpha, nu, rho)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.lognormal")]
    #[staticmethod]
    fn lognormal(alpha: f64, nu: f64, rho: f64) -> PyResult<Self> {
        SabrParameters::lognormal(alpha, nu, rho)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.shifted_normal")]
    #[staticmethod]
    fn shifted_normal(alpha: f64, nu: f64, rho: f64, shift: f64) -> PyResult<Self> {
        SabrParameters::shifted_normal(alpha, nu, rho, shift)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.shifted_lognormal")]
    #[staticmethod]
    fn shifted_lognormal(alpha: f64, nu: f64, rho: f64, shift: f64) -> PyResult<Self> {
        SabrParameters::shifted_lognormal(alpha, nu, rho, shift)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.alpha")]
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.beta")]
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.nu")]
    #[getter]
    fn nu(&self) -> f64 {
        self.inner.nu
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.rho")]
    #[getter]
    fn rho(&self) -> f64 {
        self.inner.rho
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.shift")]
    #[getter]
    fn shift(&self) -> Option<f64> {
        self.inner.shift
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.is_shifted")]
    fn is_shifted(&self) -> bool {
        self.inner.is_shifted()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.supports_negative_rates")]
    fn supports_negative_rates(&self) -> bool {
        self.inner.supports_negative_rates()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.implied_vol")]
    fn implied_vol(&self, forward: f64, strike: f64, t: f64) -> PyResult<f64> {
        self.inner
            .implied_vol(forward, strike, t)
            .map_err(core_to_py)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, PySabrParameters>>()
            .map(|rhs| self.inner == rhs.inner)
            .unwrap_or(false)
    }

    fn __repr__(&self) -> String {
        let shift_repr = match self.inner.shift {
            Some(s) => format!(", shift={s}"),
            None => String::new(),
        };
        format!(
            "SabrParameters(alpha={}, beta={}, nu={}, rho={}{})",
            self.inner.alpha, self.inner.beta, self.inner.nu, self.inner.rho, shift_repr
        )
    }
}

wire_methods!(
    PySabrParameters,
    SabrParameters,
    "SabrParameters",
    module = "finstack_quant.models.volatility"
);

#[pyclass(
    name = "SabrCalibrationOutcome",
    module = "finstack_quant.models.volatility",
    frozen,
    from_py_object
)]
#[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome")]
#[derive(Clone)]
pub struct PySabrCalibrationOutcome {
    pub(crate) inner: SabrCalibrationOutcome,
}

#[pymethods]
impl PySabrCalibrationOutcome {
    #[getter]
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.parameters")]
    fn parameters(&self) -> PySabrParameters {
        PySabrParameters {
            inner: self.inner.parameters.clone(),
        }
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.total_iterations")]
    fn total_iterations(&self) -> usize {
        self.inner.total_iterations
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.winning_iterations")]
    fn winning_iterations(&self) -> usize {
        self.inner.winning_iterations
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.residual_evaluations")]
    fn residual_evaluations(&self) -> usize {
        self.inner.residual_evaluations
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.winning_start")]
    fn winning_start(&self) -> [f64; 3] {
        self.inner.winning_start
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.quote_errors")]
    fn quote_errors(&self) -> Vec<f64> {
        self.inner.quote_errors.clone()
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.max_relative_quote_error")]
    fn max_relative_quote_error(&self) -> f64 {
        self.inner.max_relative_quote_error
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.parameters_at_bounds")]
    fn parameters_at_bounds(&self) -> Vec<String> {
        self.inner.parameters_at_bounds.clone()
    }
}

#[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile")]
#[pyclass(name = "SabrSmile", module = "finstack_quant.models.volatility")]
pub struct PySabrSmile {
    inner: SabrSmile,
}

#[pymethods]
impl PySabrSmile {
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.__init__")]
    #[new]
    fn new(params: PySabrParameters, forward: f64, t: f64) -> Self {
        Self {
            inner: SabrSmile::new(params.inner, forward, t),
        }
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.atm_vol")]
    fn atm_vol(&self) -> PyResult<f64> {
        self.inner.atm_vol().map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.implied_vol")]
    fn implied_vol(&self, strike: f64) -> PyResult<f64> {
        self.inner.implied_vol(strike).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.generate_smile")]
    fn generate_smile(&self, strikes: Vec<f64>) -> PyResult<Vec<f64>> {
        self.inner.generate_smile(&strikes).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.validate_no_arbitrage")]
    #[pyo3(signature = (strikes, r))]
    fn validate_no_arbitrage<'py>(
        &self,
        py: Python<'py>,
        strikes: Vec<f64>,
        r: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let result = self
            .inner
            .validate_no_arbitrage(&strikes, r)
            .map_err(core_to_py)?;
        serde_to_py(py, &result)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.strike_from_delta")]
    fn strike_from_delta(&self, delta: f64, is_call: bool) -> PyResult<f64> {
        self.inner
            .strike_from_delta(delta, is_call)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.check_no_arbitrage")]
    fn check_no_arbitrage(&self, strikes: Vec<f64>, r: f64) -> PyResult<()> {
        self.inner
            .check_no_arbitrage(&strikes, r)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.repair_arbitrage")]
    fn repair_arbitrage(
        &self,
        strikes: Vec<f64>,
        r: f64,
        max_iterations: usize,
    ) -> PyResult<Vec<f64>> {
        self.inner
            .repair_arbitrage(&strikes, r, max_iterations)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>, strikes: Vec<f64>) -> PyResult<Bound<'py, PyAny>> {
        let vols = self.inner.generate_smile(&strikes).map_err(core_to_py)?;
        let log_moneyness: Vec<f64> = strikes
            .iter()
            .map(|&k| self.inner.log_moneyness(k))
            .collect();
        let data = PyDict::new(py);
        data.set_item("strike", strikes)?;
        data.set_item("vol", vols)?;
        data.set_item("log_moneyness", log_moneyness)?;
        dict_to_dataframe(py, &data, None)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.forward")]
    #[getter]
    fn forward(&self) -> f64 {
        self.inner.forward()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.time_to_expiry")]
    #[getter]
    fn time_to_expiry(&self) -> f64 {
        self.inner.time_to_expiry()
    }

    fn __repr__(&self) -> String {
        let p = self.inner.parameters();
        let shift_repr = match p.shift {
            Some(s) => format!(", shift={s}"),
            None => String::new(),
        };
        format!(
            "SabrSmile(forward={}, t={}, alpha={}, beta={}, nu={}, rho={}{})",
            self.inner.forward(),
            self.inner.time_to_expiry(),
            p.alpha,
            p.beta,
            p.nu,
            p.rho,
            shift_repr
        )
    }
}

#[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator")]
#[pyclass(name = "SabrCalibrator", module = "finstack_quant.models.volatility")]
pub struct PySabrCalibrator {
    inner: SabrCalibrator,
}

impl Default for PySabrCalibrator {
    fn default() -> Self {
        Self {
            inner: SabrCalibrator::new(),
        }
    }
}

#[pymethods]
impl PySabrCalibrator {
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.__init__")]
    #[new]
    fn new() -> Self {
        Self::default()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.high_precision")]
    #[staticmethod]
    fn high_precision() -> Self {
        Self {
            inner: SabrCalibrator::high_precision(),
        }
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.with_tolerance")]
    fn with_tolerance(&self, tolerance: f64) -> Self {
        Self {
            inner: self.inner.clone().with_tolerance(tolerance),
        }
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.with_max_iterations")]
    fn with_max_iterations(&self, max_iterations: usize) -> Self {
        Self {
            inner: self.inner.clone().with_max_iterations(max_iterations),
        }
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.calibrate")]
    fn calibrate(
        &self,
        py: Python<'_>,
        forward: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        t: f64,
        beta: f64,
    ) -> PyResult<PySabrCalibrationOutcome> {
        py.detach(|| {
            self.inner
                .calibrate(forward, &strikes, &market_vols, t, beta)
        })
        .map(|inner| PySabrCalibrationOutcome { inner })
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.with_shift")]
    #[pyo3(signature = (shift))]
    fn with_shift(&self, py: Python<'_>, shift: &Bound<'_, PyAny>) -> PyResult<Self> {
        let shift: SabrShift = py_to_serde(py, shift, "shift")?;
        Ok(Self {
            inner: self.inner.clone().with_shift(shift),
        })
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.with_atm_pinning")]
    fn with_atm_pinning(&self, atm_pinning: bool) -> Self {
        Self {
            inner: self.inner.clone().with_atm_pinning(atm_pinning),
        }
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.tolerance")]
    #[getter]
    fn tolerance(&self) -> f64 {
        self.inner.tolerance()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.max_iterations")]
    #[getter]
    fn max_iterations(&self) -> usize {
        self.inner.max_iterations()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.shift")]
    #[getter]
    fn shift<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.shift())
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.atm_pinning")]
    #[getter]
    fn atm_pinning(&self) -> bool {
        self.inner.atm_pinning()
    }

    fn __repr__(&self) -> String {
        let shift = match self.inner.shift() {
            SabrShift::None => "None".to_string(),
            SabrShift::Fixed(value) => value.to_string(),
            SabrShift::Auto => format!("'{}'", SabrShift::Auto),
        };
        format!(
            "SabrCalibrator(tolerance={}, max_iterations={}, shift={}, atm_pinning={})",
            self.inner.tolerance(),
            self.inner.max_iterations(),
            shift,
            if self.inner.atm_pinning() {
                "True"
            } else {
                "False"
            }
        )
    }
}

#[doc = pydoc!("finstack_quant.models.volatility", "implied_vol_bachelier")]
#[pyfunction]
#[pyo3(signature = (price, forward, strike, t, is_call))]
fn implied_vol_bachelier(
    price: f64,
    forward: f64,
    strike: f64,
    t: f64,
    is_call: bool,
) -> PyResult<f64> {
    vol::implied_vol_bachelier(price, forward, strike, t, is_call).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "implied_vol_black")]
#[pyfunction]
#[pyo3(signature = (price, forward, strike, t, is_call))]
fn implied_vol_black(
    price: f64,
    forward: f64,
    strike: f64,
    t: f64,
    is_call: bool,
) -> PyResult<f64> {
    vol::implied_vol_black(price, forward, strike, t, is_call).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_surface_vol")]
#[pyfunction]
fn get_surface_vol(surface: &PyVolSurface, expiry: f64, strike: f64) -> PyResult<f64> {
    vol::get_surface_vol(&surface.inner, expiry, strike).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_surface_vol_clamped")]
#[pyfunction]
fn get_surface_vol_clamped(surface: &PyVolSurface, expiry: f64, strike: f64) -> f64 {
    vol::get_surface_vol_clamped(&surface.inner, expiry, strike)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_cube_vol")]
#[pyfunction]
fn get_cube_vol(cube: &PyVolCube, expiry: f64, tenor: f64, strike: f64) -> PyResult<f64> {
    vol::get_cube_vol(&cube.inner, expiry, tenor, strike).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_cube_vol_clamped")]
#[pyfunction]
fn get_cube_vol_clamped(cube: &PyVolCube, expiry: f64, tenor: f64, strike: f64) -> f64 {
    vol::get_cube_vol_clamped(&cube.inner, expiry, tenor, strike)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_cube_normal_vol")]
#[pyfunction]
fn get_cube_normal_vol(cube: &PyVolCube, expiry: f64, tenor: f64, strike: f64) -> PyResult<f64> {
    vol::get_cube_normal_vol(&cube.inner, expiry, tenor, strike).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_cube_normal_vol_clamped")]
#[pyfunction]
fn get_cube_normal_vol_clamped(cube: &PyVolCube, expiry: f64, tenor: f64, strike: f64) -> f64 {
    vol::get_cube_normal_vol_clamped(&cube.inner, expiry, tenor, strike)
}

#[doc = pydoc!("finstack_quant.models.volatility", "materialize_cube_tenor_slice")]
#[pyfunction]
fn materialize_cube_tenor_slice(
    cube: &PyVolCube,
    tenor: f64,
    strikes: Vec<f64>,
) -> PyResult<PyVolSurface> {
    vol::materialize_cube_tenor_slice(&cube.inner, tenor, &strikes)
        .map(|surface| PyVolSurface::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "materialize_cube_tenor_slice_normal")]
#[pyfunction]
fn materialize_cube_tenor_slice_normal(
    cube: &PyVolCube,
    tenor: f64,
    strikes: Vec<f64>,
) -> PyResult<PyVolSurface> {
    vol::materialize_cube_tenor_slice_normal(&cube.inner, tenor, &strikes)
        .map(|surface| PyVolSurface::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "materialize_cube_expiry_slice")]
#[pyfunction]
fn materialize_cube_expiry_slice(
    cube: &PyVolCube,
    expiry: f64,
    strikes: Vec<f64>,
) -> PyResult<PyVolCubeExpirySlice> {
    vol::materialize_cube_expiry_slice(&cube.inner, expiry, &strikes)
        .map(|surface| PyVolCubeExpirySlice::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "materialize_cube_expiry_slice_normal")]
#[pyfunction]
fn materialize_cube_expiry_slice_normal(
    cube: &PyVolCube,
    expiry: f64,
    strikes: Vec<f64>,
) -> PyResult<PyVolCubeExpirySlice> {
    vol::materialize_cube_expiry_slice_normal(&cube.inner, expiry, &strikes)
        .map(|surface| PyVolCubeExpirySlice::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_cube_expiry_slice_vol")]
#[pyfunction]
fn get_cube_expiry_slice_vol(
    slice: &PyVolCubeExpirySlice,
    tenor: f64,
    strike: f64,
) -> PyResult<f64> {
    vol::get_cube_expiry_slice_vol(&slice.inner, tenor, strike).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_cube_expiry_slice_vol_clamped")]
#[pyfunction]
fn get_cube_expiry_slice_vol_clamped(slice: &PyVolCubeExpirySlice, tenor: f64, strike: f64) -> f64 {
    vol::get_cube_expiry_slice_vol_clamped(&slice.inner, tenor, strike)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_fx_delta_pillar_vols")]
#[pyfunction]
fn get_fx_delta_pillar_vols(
    surface: &PyFxDeltaVolSurface,
    expiry_index: usize,
) -> PyResult<(f64, f64, f64)> {
    vol::get_fx_delta_pillar_vols(&surface.inner, expiry_index).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_fx_delta_vol")]
#[pyfunction]
fn get_fx_delta_vol(
    surface: &PyFxDeltaVolSurface,
    expiry: f64,
    strike: f64,
    forward: f64,
) -> PyResult<f64> {
    vol::get_fx_delta_vol(&surface.inner, expiry, strike, forward).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "materialize_fx_delta_surface")]
#[pyfunction]
fn materialize_fx_delta_surface(
    surface: &PyFxDeltaVolSurface,
    spot: f64,
    domestic_rate: f64,
    foreign_rate: f64,
) -> PyResult<PyVolSurface> {
    vol::materialize_fx_delta_surface(&surface.inner, spot, domestic_rate, foreign_rate)
        .map(|surface| PyVolSurface::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "delta_to_strike")]
#[pyfunction]
fn delta_to_strike(delta: f64, forward: f64, vol: f64, expiry: f64) -> f64 {
    vol::delta_to_strike(delta, forward, vol, expiry)
}

#[doc = pydoc!("finstack_quant.models.volatility", "strike_to_delta")]
#[pyfunction]
fn strike_to_delta(strike: f64, forward: f64, vol: f64, expiry: f64) -> f64 {
    vol::strike_to_delta(strike, forward, vol, expiry)
}

fn grid_to_dataframe<'py>(
    py: Python<'py>,
    expiries: &[f64],
    strikes: &[f64],
    vols: &[f64],
) -> PyResult<Bound<'py, PyAny>> {
    if vols.len() != expiries.len() * strikes.len() {
        return Err(crate::errors::value_error(format!(
            "vol surface grid has {} values but {} expiries x {} strikes",
            vols.len(),
            expiries.len(),
            strikes.len()
        )));
    }
    let data = PyDict::new(py);
    for (j, strike) in strikes.iter().enumerate() {
        let column: Vec<f64> = (0..expiries.len())
            .map(|i| vols[i * strikes.len() + j])
            .collect();
        data.set_item(*strike, column)?;
    }
    let kwargs = PyDict::new(py);
    kwargs.set_item("name", "expiry")?;
    let index = py
        .import("pandas")?
        .getattr("Index")?
        .call((expiries.to_vec(),), Some(&kwargs))?;
    dict_to_dataframe(py, &data, Some(index))
}

#[doc = pydoc!("finstack_quant.models.volatility", "convert_atm_volatility")]
#[pyfunction]
#[pyo3(signature = (vol, from_convention, to_convention, forward_rate, time_to_expiry))]
fn convert_atm_volatility(
    py: Python<'_>,
    vol: f64,
    from_convention: &Bound<'_, PyAny>,
    to_convention: &Bound<'_, PyAny>,
    forward_rate: f64,
    time_to_expiry: f64,
) -> PyResult<f64> {
    let from: VolatilityConvention = py_to_serde(py, from_convention, "from_convention")?;
    let to: VolatilityConvention = py_to_serde(py, to_convention, "to_convention")?;
    vol::convert_atm_volatility(vol, from, to, forward_rate, time_to_expiry).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "SviParams")]
#[pyclass(
    name = "SviParams",
    module = "finstack_quant.models.volatility",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PySviParams {
    pub(crate) inner: SviParams,
}

#[pymethods]
impl PySviParams {
    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.__init__")]
    #[new]
    #[pyo3(signature = (a, b, rho, m, sigma))]
    fn new(a: f64, b: f64, rho: f64, m: f64, sigma: f64) -> PyResult<Self> {
        SviParams::new(a, b, rho, m, sigma)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.a")]
    #[getter]
    fn a(&self) -> f64 {
        self.inner.a
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.b")]
    #[getter]
    fn b(&self) -> f64 {
        self.inner.b
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.rho")]
    #[getter]
    fn rho(&self) -> f64 {
        self.inner.rho
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.m")]
    #[getter]
    fn m(&self) -> f64 {
        self.inner.m
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.sigma")]
    #[getter]
    fn sigma(&self) -> f64 {
        self.inner.sigma
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.total_variance")]
    fn total_variance(&self, k: f64) -> f64 {
        self.inner.total_variance(k)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.implied_vol")]
    fn implied_vol(&self, k: f64, t: f64) -> PyResult<f64> {
        self.inner.implied_vol(k, t).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.durrleman_g")]
    fn durrleman_g(&self, k: f64) -> f64 {
        self.inner.durrleman_g(k)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("SviParams", &self.inner)
    }
}

wire_methods!(
    PySviParams,
    SviParams,
    "SviParams",
    module = "finstack_quant.models.volatility"
);

#[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface")]
#[pyclass(
    name = "LocalVolSurface",
    module = "finstack_quant.models.volatility",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyLocalVolSurface {
    pub(crate) inner: LocalVolSurface,
}

#[pymethods]
impl PyLocalVolSurface {
    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.__init__")]
    #[new]
    #[pyo3(signature = (expiries, strikes, local_vols))]
    fn new(expiries: Vec<f64>, strikes: Vec<f64>, local_vols: Vec<f64>) -> PyResult<Self> {
        LocalVolSurface::new(expiries, strikes, local_vols)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.from_implied_vol")]
    #[staticmethod]
    #[pyo3(signature = (surface, forwards))]
    fn from_implied_vol(surface: &PyVolSurface, forwards: Vec<f64>) -> PyResult<Self> {
        LocalVolSurface::from_implied_vol(&surface.inner, &forwards)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.from_implied_vol_smoothed")]
    #[staticmethod]
    #[pyo3(signature = (surface, forwards, sigma_strikes))]
    fn from_implied_vol_smoothed(
        surface: &PyVolSurface,
        forwards: Vec<f64>,
        sigma_strikes: f64,
    ) -> PyResult<Self> {
        LocalVolSurface::from_implied_vol_smoothed(&surface.inner, &forwards, sigma_strikes)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.value")]
    #[pyo3(signature = (expiry, strike))]
    fn value(&self, expiry: f64, strike: f64) -> f64 {
        self.inner.value(expiry, strike)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.expiries")]
    #[getter]
    fn expiries(&self) -> Vec<f64> {
        self.inner.expiries().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.strikes")]
    #[getter]
    fn strikes(&self) -> Vec<f64> {
        self.inner.strikes().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.local_vols")]
    #[getter]
    fn local_vols(&self) -> Vec<f64> {
        self.inner.local_vols().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.grid_shape")]
    #[getter]
    fn grid_shape(&self) -> (usize, usize) {
        self.inner.grid_shape()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        grid_to_dataframe(
            py,
            self.inner.expiries(),
            self.inner.strikes(),
            self.inner.local_vols(),
        )
    }

    fn __repr__(&self) -> String {
        let (expiries, strikes) = self.inner.grid_shape();
        format!("LocalVolSurface(expiries={expiries}, strikes={strikes})")
    }
}

wire_methods!(
    PyLocalVolSurface,
    LocalVolSurface,
    "LocalVolSurface",
    module = "finstack_quant.models.volatility"
);
impl_repr_html_via_dataframe!(PyLocalVolSurface);

#[doc = pydoc!("finstack_quant.models.volatility", "calibrate_svi")]
#[pyfunction]
#[pyo3(signature = (strikes, vols, forward, expiry))]
fn calibrate_svi(
    py: Python<'_>,
    strikes: Vec<f64>,
    vols: Vec<f64>,
    forward: f64,
    expiry: f64,
) -> PyResult<PySviParams> {
    py.detach(move || rust_calibrate_svi(&strikes, &vols, forward, expiry))
        .map(|inner| PySviParams { inner })
        .map_err(core_to_py)
}

/// Register the volatility submodule under `finstack_quant.models`.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "volatility")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.volatility", "__module__"),
    )?;
    m.add_class::<PySabrParameters>()?;
    m.add_class::<PySabrSmile>()?;
    m.add_class::<PySabrCalibrator>()?;
    m.add_class::<PySabrCalibrationOutcome>()?;
    m.add_class::<PySviParams>()?;
    m.add_class::<PyLocalVolSurface>()?;
    m.add_function(wrap_pyfunction!(calibrate_svi, &m)?)?;
    m.add_function(wrap_pyfunction!(convert_atm_volatility, &m)?)?;
    m.add_function(wrap_pyfunction!(get_surface_vol, &m)?)?;
    m.add_function(wrap_pyfunction!(get_surface_vol_clamped, &m)?)?;
    m.add_function(wrap_pyfunction!(get_cube_expiry_slice_vol, &m)?)?;
    m.add_function(wrap_pyfunction!(get_cube_expiry_slice_vol_clamped, &m)?)?;
    m.add_function(wrap_pyfunction!(get_cube_vol, &m)?)?;
    m.add_function(wrap_pyfunction!(get_cube_vol_clamped, &m)?)?;
    m.add_function(wrap_pyfunction!(get_cube_normal_vol, &m)?)?;
    m.add_function(wrap_pyfunction!(get_cube_normal_vol_clamped, &m)?)?;
    m.add_function(wrap_pyfunction!(materialize_cube_tenor_slice, &m)?)?;
    m.add_function(wrap_pyfunction!(materialize_cube_tenor_slice_normal, &m)?)?;
    m.add_function(wrap_pyfunction!(materialize_cube_expiry_slice, &m)?)?;
    m.add_function(wrap_pyfunction!(materialize_cube_expiry_slice_normal, &m)?)?;
    m.add_function(wrap_pyfunction!(get_fx_delta_pillar_vols, &m)?)?;
    m.add_function(wrap_pyfunction!(get_fx_delta_vol, &m)?)?;
    m.add_function(wrap_pyfunction!(materialize_fx_delta_surface, &m)?)?;
    m.add_function(wrap_pyfunction!(delta_to_strike, &m)?)?;
    m.add_function(wrap_pyfunction!(implied_vol_bachelier, &m)?)?;
    m.add_function(wrap_pyfunction!(implied_vol_black, &m)?)?;
    m.add_function(wrap_pyfunction!(strike_to_delta, &m)?)?;
    super::volatility_arbitrage::register(py, &m)?;
    m.setattr(
        "__all__",
        PyList::new(
            py,
            [
                "ArbitrageReport",
                "LocalVolSurface",
                "SabrCalibrationOutcome",
                "SabrCalibrator",
                "SabrParameters",
                "SabrSmile",
                "SviParams",
                "calibrate_svi",
                "check_butterfly_grid",
                "check_calendar_spread_grid",
                "check_local_vol_density_grid",
                "check_surface_grid",
                "convert_atm_volatility",
                "delta_to_strike",
                "get_cube_expiry_slice_vol",
                "get_cube_expiry_slice_vol_clamped",
                "get_cube_normal_vol",
                "get_cube_normal_vol_clamped",
                "get_cube_vol",
                "get_cube_vol_clamped",
                "get_fx_delta_pillar_vols",
                "get_fx_delta_vol",
                "get_surface_vol",
                "get_surface_vol_clamped",
                "implied_vol_bachelier",
                "implied_vol_black",
                "materialize_cube_expiry_slice",
                "materialize_cube_expiry_slice_normal",
                "materialize_cube_tenor_slice",
                "materialize_cube_tenor_slice_normal",
                "materialize_fx_delta_surface",
                "strike_to_delta",
            ],
        )?,
    )?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
