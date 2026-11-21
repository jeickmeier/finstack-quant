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

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use std::sync::Arc;

use crate::bindings::core::market_data::curves::{
    PyFxDeltaVolSurface, PyVolCube, PyVolCubeExpirySlice, PyVolSurface,
};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::core_to_py;
use finstack_quant_models::volatility as vol;
use finstack_quant_models::volatility::VolatilityConvention;
use finstack_quant_models::volatility::local_vol::LocalVolSurface;
use finstack_quant_models::volatility::sabr::{
    SabrCalibrationOutcome, SabrCalibrator, SabrParameters, SabrShift, SabrSmile,
};
use finstack_quant_models::volatility::svi::{SviParams, calibrate_svi as rust_calibrate_svi};
use pyo3::prelude::*;
use pyo3::types::PyList;

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

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.is_shifted")]
    fn is_shifted(&self) -> bool {
        self.inner.is_shifted()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.supports_negative_rates")]
    fn supports_negative_rates(&self) -> bool {
        self.inner.supports_negative_rates()
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.implied_vol")]
    fn implied_vol(&self, forward: f64, strike: f64, t: f64, quote_type: &str) -> PyResult<f64> {
        let quote_type = finstack_quant_core::wire::serde_parse(quote_type).map_err(core_to_py)?;
        self.inner
            .implied_vol(forward, strike, t, quote_type)
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

macro_rules! sabr_preset {
    (infallible $($name:ident),+ $(,)?) => {
        #[pymethods]
        impl PySabrParameters {
            $(
                #[doc = pydoc!("finstack_quant.models.volatility", concat!("SabrParameters.", stringify!($name)))]
                #[staticmethod]
                fn $name() -> Self {
                    Self {
                        inner: SabrParameters::$name(),
                    }
                }
            )+
        }
    };
    (fallible $($name:ident($($arg:ident),+)),+ $(,)?) => {
        #[pymethods]
        impl PySabrParameters {
            $(
                #[doc = pydoc!("finstack_quant.models.volatility", concat!("SabrParameters.", stringify!($name)))]
                #[staticmethod]
                fn $name($($arg: f64),+) -> PyResult<Self> {
                    SabrParameters::$name($($arg),+)
                        .map(|inner| Self { inner })
                        .map_err(core_to_py)
                }
            )+
        }
    };
}

sabr_preset!(infallible equity_default, rates_default);
sabr_preset!(
    fallible
    rates_standard(alpha, nu, rho),
    normal(alpha, nu, rho),
    lognormal(alpha, nu, rho),
    shifted_normal(alpha, nu, rho, shift),
    shifted_lognormal(alpha, nu, rho, shift),
);

py_getters!(PySabrParameters |s| {
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.alpha")]
    alpha -> f64 = s.inner.alpha;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.beta")]
    beta -> f64 = s.inner.beta;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.nu")]
    nu -> f64 = s.inner.nu;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.rho")]
    rho -> f64 = s.inner.rho;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrParameters.shift")]
    shift -> Option<f64> = s.inner.shift;
});

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

py_getters!(PySabrCalibrationOutcome |s| {
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.parameters")]
    parameters -> PySabrParameters = PySabrParameters { inner: s.inner.parameters.clone() };
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.total_iterations")]
    total_iterations -> usize = s.inner.total_iterations;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.winning_iterations")]
    winning_iterations -> usize = s.inner.winning_iterations;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.residual_evaluations")]
    residual_evaluations -> usize = s.inner.residual_evaluations;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.winning_start")]
    winning_start -> [f64; 3] = s.inner.winning_start;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.quote_errors")]
    quote_errors -> Vec<f64> = s.inner.quote_errors.clone();
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.max_relative_quote_error")]
    max_relative_quote_error -> f64 = s.inner.max_relative_quote_error;
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrationOutcome.parameters_at_bounds")]
    parameters_at_bounds -> Vec<String> = s.inner.parameters_at_bounds.clone();
});

#[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile")]
#[pyclass(name = "SabrSmile", module = "finstack_quant.models.volatility")]
pub struct PySabrSmile {
    inner: SabrSmile,
}

#[pymethods]
impl PySabrSmile {
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.__init__")]
    #[new]
    fn new(params: PySabrParameters, forward: f64, t: f64, quote_type: &str) -> PyResult<Self> {
        let quote_type = finstack_quant_core::wire::serde_parse(quote_type).map_err(core_to_py)?;
        Ok(Self {
            inner: SabrSmile::new(params.inner, forward, t, quote_type),
        })
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

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.to_arrow")]
    #[pyo3(signature = (strikes), text_signature = "($self, strikes)")]
    fn to_arrow(&self, strikes: Vec<f64>) -> PyResult<PyArrowTable> {
        self.sabr_smile_table(&strikes)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.to_pandas")]
    #[pyo3(signature = (strikes), text_signature = "($self, strikes)")]
    fn to_pandas<'py>(&self, py: Python<'py>, strikes: Vec<f64>) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.sabr_smile_table(&strikes)?)?;
        PyArrowTable::to_pandas(&table, py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.to_polars")]
    #[pyo3(signature = (strikes), text_signature = "($self, strikes)")]
    fn to_polars<'py>(&self, py: Python<'py>, strikes: Vec<f64>) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.sabr_smile_table(&strikes)?)?;
        PyArrowTable::to_polars(&table, py)
    }

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.to_pyarrow")]
    #[pyo3(signature = (strikes), text_signature = "($self, strikes)")]
    fn to_pyarrow<'py>(&self, py: Python<'py>, strikes: Vec<f64>) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.sabr_smile_table(&strikes)?)?;
        PyArrowTable::to_pyarrow(&table, py)
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

py_getters!(PySabrSmile |s| {
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.forward")]
    forward -> f64 = s.inner.forward();
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrSmile.time_to_expiry")]
    time_to_expiry -> f64 = s.inner.time_to_expiry();
});

impl PySabrSmile {
    fn sabr_smile_table(&self, strikes: &[f64]) -> PyResult<PyArrowTable> {
        let vols = self.inner.generate_smile(strikes).map_err(core_to_py)?;
        let log_moneyness: Vec<f64> = strikes
            .iter()
            .map(|&k| self.inner.log_moneyness(k))
            .collect();
        TableBuilder::new()
            .f64("strike", strikes.to_vec())?
            .f64("vol", vols)?
            .f64("log_moneyness", log_moneyness)?
            .build()
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
        forward: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        t: f64,
        beta: f64,
        quote_type: &str,
    ) -> PyResult<PySabrCalibrationOutcome> {
        let quote_type = finstack_quant_core::wire::serde_parse(quote_type).map_err(core_to_py)?;
        Python::attach(|py| {
            py.detach(|| {
                self.inner
                    .calibrate(forward, &strikes, &market_vols, t, beta, quote_type)
            })
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

    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.shift")]
    #[getter]
    fn shift<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.shift())
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

py_getters!(PySabrCalibrator |s| {
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.tolerance")]
    tolerance -> f64 = s.inner.tolerance();
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.max_iterations")]
    max_iterations -> usize = s.inner.max_iterations();
    #[doc = pydoc!("finstack_quant.models.volatility", "SabrCalibrator.atm_pinning")]
    atm_pinning -> bool = s.inner.atm_pinning();
});

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
fn get_cube_vol(
    py: Python<'_>,
    cube: &PyVolCube,
    expiry: f64,
    tenor: f64,
    strike: f64,
    convention: &Bound<'_, PyAny>,
) -> PyResult<f64> {
    let convention: VolatilityConvention = py_to_serde(py, convention, "convention")?;
    vol::get_cube_vol(&cube.inner, expiry, tenor, strike, convention).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "get_cube_vol_clamped")]
#[pyfunction]
fn get_cube_vol_clamped(
    py: Python<'_>,
    cube: &PyVolCube,
    expiry: f64,
    tenor: f64,
    strike: f64,
    convention: &Bound<'_, PyAny>,
) -> PyResult<f64> {
    let convention: VolatilityConvention = py_to_serde(py, convention, "convention")?;
    vol::get_cube_vol_clamped(&cube.inner, expiry, tenor, strike, convention).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "materialize_cube_tenor_slice")]
#[pyfunction]
fn materialize_cube_tenor_slice(
    py: Python<'_>,
    cube: &PyVolCube,
    tenor: f64,
    strikes: Vec<f64>,
    convention: &Bound<'_, PyAny>,
    interpolation_mode: &str,
) -> PyResult<PyVolSurface> {
    let convention: VolatilityConvention = py_to_serde(py, convention, "convention")?;
    let mode = finstack_quant_core::wire::serde_parse(interpolation_mode).map_err(core_to_py)?;
    vol::materialize_cube_tenor_slice(&cube.inner, tenor, &strikes, convention, mode)
        .map(|surface| PyVolSurface::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "materialize_cube_expiry_slice")]
#[pyfunction]
fn materialize_cube_expiry_slice(
    py: Python<'_>,
    cube: &PyVolCube,
    expiry: f64,
    strikes: Vec<f64>,
    convention: &Bound<'_, PyAny>,
) -> PyResult<PyVolCubeExpirySlice> {
    let convention: VolatilityConvention = py_to_serde(py, convention, "convention")?;
    vol::materialize_cube_expiry_slice(&cube.inner, expiry, &strikes, convention)
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
fn get_fx_delta_pillar_vols<'py>(
    py: Python<'py>,
    surface: &PyFxDeltaVolSurface,
    expiry_index: usize,
) -> PyResult<Bound<'py, pyo3::types::PyTuple>> {
    let vols = vol::get_fx_delta_pillar_vols(&surface.inner, expiry_index).map_err(core_to_py)?;
    pyo3::types::PyTuple::new(py, vols)
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
    delivery_forwards: Vec<f64>,
    interpolation_mode: &str,
) -> PyResult<PyVolSurface> {
    let mode = finstack_quant_core::wire::serde_parse(interpolation_mode).map_err(core_to_py)?;
    vol::materialize_fx_delta_surface(&surface.inner, &delivery_forwards, mode)
        .map(|surface| PyVolSurface::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "delta_to_strike")]
#[pyfunction]
fn delta_to_strike(
    is_call: bool,
    delta_abs: f64,
    forward: f64,
    vol: f64,
    expiry: f64,
) -> PyResult<f64> {
    vol::delta_to_strike(is_call, delta_abs, forward, vol, expiry).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility", "strike_to_delta")]
#[pyfunction]
fn strike_to_delta(
    is_call: bool,
    strike: f64,
    forward: f64,
    vol: f64,
    expiry: f64,
) -> PyResult<f64> {
    vol::strike_to_delta(is_call, strike, forward, vol, expiry).map_err(core_to_py)
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

py_getters!(PySviParams |s| {
    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.a")]
    a -> f64 = s.inner.a;
    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.b")]
    b -> f64 = s.inner.b;
    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.rho")]
    rho -> f64 = s.inner.rho;
    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.m")]
    m -> f64 = s.inner.m;
    #[doc = pydoc!("finstack_quant.models.volatility", "SviParams.sigma")]
    sigma -> f64 = s.inner.sigma;
});

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

    fn __repr__(&self) -> String {
        let (expiries, strikes) = self.inner.grid_shape();
        format!("LocalVolSurface(expiries={expiries}, strikes={strikes})")
    }
}

py_getters!(PyLocalVolSurface |s| {
    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.expiries")]
    expiries -> Vec<f64> = s.inner.expiries().to_vec();
    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.strikes")]
    strikes -> Vec<f64> = s.inner.strikes().to_vec();
    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.local_vols")]
    local_vols -> Vec<f64> = s.inner.local_vols().to_vec();
    #[doc = pydoc!("finstack_quant.models.volatility", "LocalVolSurface.grid_shape")]
    grid_shape -> (usize, usize) = s.inner.grid_shape();
});

impl PyLocalVolSurface {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let expiries = self.inner.expiries();
        let strikes = self.inner.strikes();
        let vols = self.inner.local_vols();
        let expected = expiries.len() * strikes.len();
        if vols.len() != expected {
            return Err(crate::errors::value_error(format!(
                "local vol grid has {} values but {} expiries x {} strikes",
                vols.len(),
                expiries.len(),
                strikes.len()
            )));
        }
        let mut exp_col = Vec::with_capacity(expected);
        let mut strike_col = Vec::with_capacity(expected);
        for &expiry in expiries {
            for &strike in strikes {
                exp_col.push(expiry);
                strike_col.push(strike);
            }
        }
        TableBuilder::new()
            .f64("expiry", exp_col)?
            .f64("strike", strike_col)?
            .f64("local_vol", vols.to_vec())?
            .build()
    }
}

tabular_methods!(
    PyLocalVolSurface,
    "LocalVolSurface",
    module = "finstack_quant.models.volatility"
);
wire_methods!(
    PyLocalVolSurface,
    LocalVolSurface,
    "LocalVolSurface",
    module = "finstack_quant.models.volatility"
);

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
    m.add_function(wrap_pyfunction!(materialize_cube_tenor_slice, &m)?)?;
    m.add_function(wrap_pyfunction!(materialize_cube_expiry_slice, &m)?)?;
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
                "check_surface",
                "check_surface_grid",
                "convert_atm_volatility",
                "delta_to_strike",
                "get_cube_expiry_slice_vol",
                "get_cube_expiry_slice_vol_clamped",
                "get_cube_vol",
                "get_cube_vol_clamped",
                "get_fx_delta_pillar_vols",
                "get_fx_delta_vol",
                "get_surface_vol",
                "get_surface_vol_clamped",
                "implied_vol_bachelier",
                "implied_vol_black",
                "materialize_cube_expiry_slice",
                "materialize_cube_tenor_slice",
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
