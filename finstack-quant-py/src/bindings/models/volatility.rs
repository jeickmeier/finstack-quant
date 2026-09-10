//! Product-independent volatility model and evaluator bindings.
//!
//! Units: SABR ``alpha`` is the initial (Black or normal, depending on
//! ``beta``) volatility as a decimal, ``nu`` is vol-of-vol as a decimal,
//! ``rho`` is a correlation in ``[-1, 1]`` and ``shift`` is a displacement in
//! the forward's rate units. Normal (Bachelier) vols are absolute, in the
//! forward's units; Black vols are decimals.
//!
//! Exposes four classes from `finstack_quant_models::volatility::sabr`:
//!
//! * [`SabrParameters`] — the four canonical parameters `(alpha, beta, nu, rho)`
//!   plus an optional `shift` for negative-rate environments.
//! * [`SabrModel`] — wraps `SabrParameters` with an `implied_vol(forward, strike, t)` method.
//! * [`SabrSmile`] — fixes a forward and expiry, provides `implied_vol(strike)`, bulk
//!   smile generation, and optional no-arbitrage diagnostics.
//! * [`SabrCalibrator`] — Levenberg-Marquardt calibration to market vols with
//!   beta fixed (standard quant convention).
//!
//! # Naming note
//!
//! Rust and Python use the canonical PascalCase forms (`SabrParameters`,
//! `SabrModel`, `SabrSmile`, `SabrCalibrator`).

use std::sync::Arc;

use crate::bindings::core::market_data::curves::{PyFxDeltaVolSurface, PyVolCube, PyVolSurface};
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_models::volatility as vol;
use finstack_quant_models::volatility::sabr::{
    SabrCalibrator, SabrModel, SabrParameters, SabrSmile,
};
use finstack_quant_models::volatility::svi::{calibrate_svi as rust_calibrate_svi, SviParams};
use finstack_quant_models::volatility::VolatilityConvention;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// SABR model parameters ``(alpha, beta, nu, rho)`` with optional ``shift``.
///
/// Constructed with validation: ``alpha > 0``, ``beta in [0, 1]``,
/// ``nu >= 0``, ``rho in [-1, 1]``, and when supplied ``shift > 0``.
///
/// Sources
/// -------
/// - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr
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
    /// Create a new ``SabrParameters`` instance.
    ///
    /// Parameters
    /// ----------
    /// alpha : float
    ///     Initial volatility level as a decimal (Black vol for ``beta > 0``,
    ///     absolute normal vol for ``beta == 0``). Must be strictly positive.
    /// beta : float
    ///     CEV exponent. Must be in ``[0, 1]``.
    /// nu : float
    ///     Volatility of volatility as a decimal. Must be non-negative.
    /// rho : float
    ///     Correlation between asset and volatility. Must be in ``[-1, 1]``.
    /// shift : float, optional
    ///     Displacement for negative-rate support, in the same rate units as
    ///     the forward (e.g. ``0.03`` for a 3% shift). When provided must be
    ///     strictly positive.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any parameter is outside its domain.
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

    /// Equity-standard defaults: ``alpha=0.20, beta=1.0, nu=0.30, rho=-0.20``.
    #[staticmethod]
    fn equity_default() -> Self {
        Self {
            inner: SabrParameters::equity_default(),
        }
    }

    /// Rates-standard defaults: ``alpha=0.02, beta=0.5, nu=0.30, rho=0.0``.
    #[staticmethod]
    fn rates_default() -> Self {
        Self {
            inner: SabrParameters::rates_default(),
        }
    }

    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }

    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }

    #[getter]
    fn nu(&self) -> f64 {
        self.inner.nu
    }

    #[getter]
    fn rho(&self) -> f64 {
        self.inner.rho
    }

    #[getter]
    fn shift(&self) -> Option<f64> {
        self.inner.shift
    }

    /// ``True`` if the parameters include a non-zero shift (negative-rate support).
    fn is_shifted(&self) -> bool {
        self.inner.is_shifted()
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

/// Hagan-2002 SABR model wrapping a :class:`SabrParameters` instance.
///
/// Sources
/// -------
/// - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr
#[pyclass(name = "SabrModel", module = "finstack_quant.models.volatility")]
pub struct PySabrModel {
    pub(crate) inner: SabrModel,
}

#[pymethods]
impl PySabrModel {
    /// Construct a new SABR model.
    ///
    /// Parameters
    /// ----------
    /// params : SabrParameters
    ///     Calibrated SABR parameter set.
    #[new]
    fn new(params: PySabrParameters) -> Self {
        Self {
            inner: SabrModel::new(params.inner),
        }
    }

    /// Hagan-2002 implied volatility.
    ///
    /// Parameters
    /// ----------
    /// forward : float
    ///     Forward price / rate.
    /// strike : float
    ///     Option strike.
    /// t : float
    ///     Time to expiry in years.
    ///
    /// Returns
    /// -------
    /// float
    ///     Implied volatility (annualised decimal Black vol, or absolute
    ///     normal vol when ``beta == 0``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``t`` is not positive or the forward / strike is outside the
    ///     (shifted) SABR domain.
    fn implied_vol(&self, forward: f64, strike: f64, t: f64) -> PyResult<f64> {
        self.inner
            .implied_volatility(forward, strike, t)
            .map_err(core_to_py)
    }

    /// Parameters used by this model.
    #[getter]
    fn params(&self) -> PySabrParameters {
        PySabrParameters {
            inner: self.inner.parameters().clone(),
        }
    }

    /// ``True`` for normal beta=0 dynamics or a configured displacement.
    fn supports_negative_rates(&self) -> bool {
        self.inner.supports_negative_rates()
    }

    fn __repr__(&self) -> String {
        let p = self.inner.parameters();
        let shift_repr = match p.shift {
            Some(s) => format!(", shift={s}"),
            None => String::new(),
        };
        format!(
            "SabrModel(alpha={}, beta={}, nu={}, rho={}{})",
            p.alpha, p.beta, p.nu, p.rho, shift_repr
        )
    }
}

/// Volatility smile generator for a fixed ``(forward, t)`` pair.
///
/// Sources
/// -------
/// - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr
#[pyclass(name = "SabrSmile", module = "finstack_quant.models.volatility")]
pub struct PySabrSmile {
    inner: SabrSmile,
}

#[pymethods]
impl PySabrSmile {
    /// Construct a smile for the given forward and time-to-expiry.
    ///
    /// Parameters
    /// ----------
    /// params : SabrParameters
    ///     Calibrated SABR parameters.
    /// forward : float
    ///     Forward price / rate.
    /// t : float
    ///     Time to expiry in years.
    #[new]
    fn new(params: PySabrParameters, forward: f64, t: f64) -> Self {
        let model = SabrModel::new(params.inner);
        Self {
            inner: SabrSmile::new(model, forward, t),
        }
    }

    /// At-the-money implied volatility.
    fn atm_vol(&self) -> PyResult<f64> {
        self.inner.atm_vol().map_err(core_to_py)
    }

    /// Implied volatility at a single strike.
    fn implied_vol(&self, strike: f64) -> PyResult<f64> {
        self.inner
            .generate_smile(&[strike])
            .map_err(core_to_py)?
            .first()
            .copied()
            .ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    "SABR smile returned no volatility for the requested strike",
                )
            })
    }

    /// Generate implied volatilities for a vector of strikes.
    ///
    /// Returns
    /// -------
    /// list[float]
    ///     Implied volatilities in strike order.
    fn generate_smile(&self, strikes: Vec<f64>) -> PyResult<Vec<f64>> {
        self.inner.generate_smile(&strikes).map_err(core_to_py)
    }

    /// Arbitrage diagnostics (butterfly + monotonicity) across ``strikes``.
    ///
    /// Parameters
    /// ----------
    /// strikes : list[float]
    ///     Strike grid to evaluate. Must be sorted in ascending order for
    ///     monotonicity checks to be meaningful.
    /// r : float, optional
    ///     Risk-free rate (default ``0.0``).
    /// q : float, optional
    ///     Dividend / foreign rate (default ``0.0``).
    ///
    /// Returns
    /// -------
    /// dict
    ///     ``{"arbitrage_free": bool, "butterfly_violations": [...],
    ///     "monotonicity_violations": [...]}``. Violation lists contain dicts
    ///     with strike, price, and severity fields.
    #[pyo3(signature = (strikes, r=0.0, q=0.0))]
    fn arbitrage_diagnostics<'py>(
        &self,
        py: Python<'py>,
        strikes: Vec<f64>,
        r: f64,
        q: f64,
    ) -> PyResult<Bound<'py, PyDict>> {
        let result = self
            .inner
            .validate_no_arbitrage(&strikes, r, q)
            .map_err(core_to_py)?;
        let out = PyDict::new(py);
        out.set_item("arbitrage_free", result.is_arbitrage_free())?;
        out.set_item(
            "butterfly_violations",
            serde_to_py(py, &result.butterfly_violations)?,
        )?;
        out.set_item(
            "monotonicity_violations",
            serde_to_py(py, &result.monotonicity_violations)?,
        )?;
        Ok(out)
    }

    /// Tabulate the smile on a strike grid as a ``pandas.DataFrame``.
    ///
    /// Parameters
    /// ----------
    /// strikes : list[float]
    ///     Strikes at which to evaluate the smile.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     Columns ``strike``, ``vol`` (decimal Black vol, or absolute normal
    ///     vol when ``beta == 0``) and ``log_moneyness`` (``ln(K / F)``;
    ///     ``NaN`` when the ratio is not positive).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any strike or the stored forward / expiry is outside the model
    ///     domain.
    fn to_dataframe<'py>(&self, py: Python<'py>, strikes: Vec<f64>) -> PyResult<Bound<'py, PyAny>> {
        let vols = self.inner.generate_smile(&strikes).map_err(core_to_py)?;
        let forward = self.inner.forward();
        let log_moneyness: Vec<f64> = strikes
            .iter()
            .map(|k| {
                let ratio = k / forward;
                if ratio > 0.0 {
                    ratio.ln()
                } else {
                    f64::NAN
                }
            })
            .collect();
        let data = PyDict::new(py);
        data.set_item("strike", strikes)?;
        data.set_item("vol", vols)?;
        data.set_item("log_moneyness", log_moneyness)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Forward used by this smile.
    #[getter]
    fn forward(&self) -> f64 {
        self.inner.forward()
    }

    /// Time to expiry in years used by this smile.
    #[getter]
    fn t(&self) -> f64 {
        self.inner.time_to_expiry()
    }

    fn __repr__(&self) -> String {
        let p = self.inner.model().parameters();
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

/// SABR calibrator using Levenberg-Marquardt with beta fixed.
///
/// Sources
/// -------
/// - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr
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
    /// Construct a calibrator with production defaults (tolerance ``1e-4``
    /// on the vega-weighted SSE objective, ``max_iter=2000``,
    /// finite-difference gradients).
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// High-precision calibrator (tolerance ``1e-8``, ``max_iter=200``).
    #[staticmethod]
    fn high_precision() -> Self {
        Self {
            inner: SabrCalibrator::high_precision(),
        }
    }

    /// Override the maximum relative error accepted for any final volatility quote.
    ///
    /// Preserves all other previously-set fields (e.g. the ``max_iter=200``
    /// from :meth:`high_precision`) by cloning the existing calibrator and
    /// only adjusting the tolerance.
    ///
    /// Parameters
    /// ----------
    /// tolerance : float
    ///     Positive finite relative quote-error limit; 1e-4 permits at most
    ///     0.01% of each quoted volatility. Invalid settings fail at calibration.
    fn with_tolerance(&self, tolerance: f64) -> Self {
        Self {
            inner: self.inner.clone().with_tolerance(tolerance),
        }
    }

    /// Override the solver iteration cap.
    ///
    /// Parameters
    /// ----------
    /// max_iterations : int
    ///     Positive cap on Levenberg-Marquardt iterations before the fit is
    ///     reported as non-converged. Pair a tight ``with_tolerance`` with a
    ///     larger budget.
    ///
    /// Returns
    /// -------
    /// SabrCalibrator
    ///     Copy of this calibrator with the iteration cap replaced.
    fn with_max_iterations(&self, max_iterations: usize) -> Self {
        Self {
            inner: self.inner.clone().with_max_iterations(max_iterations),
        }
    }

    /// Calibrate SABR parameters to a market vol smile.
    ///
    /// Parameters
    /// ----------
    /// forward : float
    ///     Forward price / rate.
    /// strikes : list[float]
    ///     Strikes at which market vols are quoted.
    /// market_vols : list[float]
    ///     Observed Black implied volatilities at each strike.
    /// t : float
    ///     Time to expiry in years.
    /// beta : float
    ///     Fixed CEV exponent (``1.0`` for equity lognormal, ``0.5`` for
    ///     rates, ``0.0`` for normal vol). Required — matching the Rust and
    ///     WASM signatures — so the convention is always an explicit choice.
    ///
    /// Returns
    /// -------
    /// SabrParameters
    ///     Calibrated parameters (``beta`` fixed to the input value).
    fn calibrate(
        &self,
        py: Python<'_>,
        forward: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        t: f64,
        beta: f64,
    ) -> PyResult<PySabrParameters> {
        py.detach(|| {
            self.inner
                .calibrate(forward, &strikes, &market_vols, t, beta)
        })
        .map(|inner| PySabrParameters { inner })
        .map_err(core_to_py)
    }

    /// Calibrate with automatic shift selection for negative-rate smiles.
    ///
    /// When the forward or any strike is negative, a shifted-SABR fit is
    /// performed with an automatically chosen shift; otherwise this behaves
    /// like :meth:`calibrate`.
    ///
    /// Parameters
    /// ----------
    /// forward : float
    ///     Forward price / rate (may be negative).
    /// strikes : list[float]
    ///     Strikes at which market vols are quoted (may be negative).
    /// market_vols : list[float]
    ///     Observed Black implied volatilities at each strike.
    /// t : float
    ///     Time to expiry in years.
    /// beta : float
    ///     Fixed CEV exponent.
    ///
    /// Returns
    /// -------
    /// SabrParameters
    ///     Calibrated parameters; ``shift`` is set when a shifted fit was used.
    fn calibrate_auto_shift(
        &self,
        py: Python<'_>,
        forward: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        t: f64,
        beta: f64,
    ) -> PyResult<PySabrParameters> {
        py.detach(|| {
            self.inner
                .calibrate_auto_shift(forward, &strikes, &market_vols, t, beta)
        })
        .map(|inner| PySabrParameters { inner })
        .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "SabrCalibrator(tolerance={}, max_iterations={})",
            self.inner.tolerance(),
            self.inner.max_iterations()
        )
    }
}

/// Evaluate a core volatility surface with checked grid bounds.
#[pyfunction]
fn get_surface_vol(surface: &PyVolSurface, expiry: f64, strike: f64) -> PyResult<f64> {
    vol::get_surface_vol(&surface.inner, expiry, strike).map_err(core_to_py)
}

/// Evaluate a core volatility surface with flat coordinate extrapolation.
#[pyfunction]
fn get_surface_vol_clamped(surface: &PyVolSurface, expiry: f64, strike: f64) -> f64 {
    vol::get_surface_vol_clamped(&surface.inner, expiry, strike)
}

/// Evaluate a core SABR cube with checked expiry and tenor bounds.
#[pyfunction]
fn get_cube_vol(cube: &PyVolCube, expiry: f64, tenor: f64, strike: f64) -> PyResult<f64> {
    vol::get_cube_vol(&cube.inner, expiry, tenor, strike).map_err(core_to_py)
}

/// Evaluate a core SABR cube with clamped expiry and tenor coordinates.
#[pyfunction]
fn get_cube_vol_clamped(cube: &PyVolCube, expiry: f64, tenor: f64, strike: f64) -> f64 {
    vol::get_cube_vol_clamped(&cube.inner, expiry, tenor, strike)
}

/// Evaluate normal/Bachelier volatility from a core SABR cube.
///
/// Returns an **absolute** normal volatility in the cube's rate units (e.g.
/// ``0.0075`` for 75 bp on decimal rates), not a Black decimal vol.
///
/// Raises ``ValueError`` for out-of-grid coordinates or an invalid
/// shifted-SABR domain.
#[pyfunction]
fn get_cube_normal_vol(cube: &PyVolCube, expiry: f64, tenor: f64, strike: f64) -> PyResult<f64> {
    vol::get_cube_normal_vol(&cube.inner, expiry, tenor, strike).map_err(core_to_py)
}

/// Evaluate clamped normal/Bachelier volatility from a core SABR cube.
///
/// Returns an absolute normal volatility in the cube's rate units; ``NaN``
/// when the expansion is undefined. Does not raise.
#[pyfunction]
fn get_cube_normal_vol_clamped(cube: &PyVolCube, expiry: f64, tenor: f64, strike: f64) -> f64 {
    vol::get_cube_normal_vol_clamped(&cube.inner, expiry, tenor, strike)
}

/// Materialize a cube tenor slice as a lognormal core surface artifact.
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

/// Materialize a cube tenor slice as a normal-volatility core surface artifact.
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

/// Materialize a cube expiry slice as a lognormal tenor-axis surface artifact.
#[pyfunction]
fn materialize_cube_expiry_slice(
    cube: &PyVolCube,
    expiry: f64,
    strikes: Vec<f64>,
) -> PyResult<PyVolSurface> {
    vol::materialize_cube_expiry_slice(&cube.inner, expiry, &strikes)
        .map(|surface| PyVolSurface::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

/// Materialize a cube expiry slice as a normal-volatility tenor-axis surface artifact.
#[pyfunction]
fn materialize_cube_expiry_slice_normal(
    cube: &PyVolCube,
    expiry: f64,
    strikes: Vec<f64>,
) -> PyResult<PyVolSurface> {
    vol::materialize_cube_expiry_slice_normal(&cube.inner, expiry, &strikes)
        .map(|surface| PyVolSurface::from_inner(Arc::new(surface)))
        .map_err(core_to_py)
}

/// Return ATM, 25-delta put, and 25-delta call vols at a stored FX expiry.
#[pyfunction]
fn get_fx_delta_pillar_vols(
    surface: &PyFxDeltaVolSurface,
    expiry_index: usize,
) -> PyResult<(f64, f64, f64)> {
    vol::get_fx_delta_pillar_vols(&surface.inner, expiry_index).map_err(core_to_py)
}

/// Evaluate an FX delta-quoted core surface at an expiry, strike, and forward.
#[pyfunction]
fn get_fx_delta_vol(
    surface: &PyFxDeltaVolSurface,
    expiry: f64,
    strike: f64,
    forward: f64,
) -> PyResult<f64> {
    vol::get_fx_delta_vol(&surface.inner, expiry, strike, forward).map_err(core_to_py)
}

/// Materialize an FX delta-quoted artifact as a strike-axis core surface.
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

/// Convert premium-unadjusted forward call delta to strike.
///
/// The convention is fixed: forward (not spot), premium-unadjusted, call
/// delta in ``(0, 1)``. ``vol`` is a decimal Black vol. Does not raise;
/// invalid inputs propagate IEEE non-finite results.
#[pyfunction]
fn delta_to_strike(delta: f64, forward: f64, vol: f64, expiry: f64) -> f64 {
    vol::delta_to_strike(delta, forward, vol, expiry)
}

/// Convert strike to premium-unadjusted forward call delta.
///
/// Same fixed convention as ``delta_to_strike``. Does not raise.
#[pyfunction]
fn strike_to_delta(strike: f64, forward: f64, vol: f64, expiry: f64) -> f64 {
    vol::strike_to_delta(strike, forward, vol, expiry)
}

/// Tabulate a core ``VolSurface`` grid as a ``pandas.DataFrame``.
///
/// Parameters
/// ----------
/// surface : VolSurface
///     Data-only surface, e.g. the result of ``materialize_cube_tenor_slice``
///     or ``materialize_fx_delta_surface``.
///
/// Returns
/// -------
/// pandas.DataFrame
///     Expiry-major grid: index ``expiry`` (years), one column per strike /
///     secondary-axis node, values in the surface's ``quote_type`` units
///     (decimal Black vol or absolute normal vol).
///
/// Raises
/// ------
/// ValueError
///     If the stored grid length does not match ``expiries x strikes``.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.market_data import FxDeltaVolSurface
/// >>> from finstack_quant.models.volatility import materialize_fx_delta_surface, surface_to_dataframe
/// >>> s = FxDeltaVolSurface("FX", [1.0], [0.12], [0.01], [0.002])
/// >>> surface_to_dataframe(materialize_fx_delta_surface(s, 1.1, 0.03, 0.02)).shape
/// (1, 3)
#[pyfunction]
fn surface_to_dataframe<'py>(
    py: Python<'py>,
    surface: &PyVolSurface,
) -> PyResult<Bound<'py, PyAny>> {
    let expiries = surface.inner.expiries();
    let strikes = surface.inner.strikes();
    let vols = surface.inner.vols();
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

/// Convert an ATM volatility quote between normal, lognormal and shifted-lognormal conventions.
///
/// Prices are equated at the money (strike = forward) and the target-convention
/// vol is solved by Brent bisection, so the conversion is deterministic and
/// returns an explicit error rather than a guess.
///
/// Parameters
/// ----------
/// vol : float
///     Input volatility in the source convention: decimal Black vol for
///     ``"lognormal"`` / shifted-lognormal, absolute vol in the forward's rate
///     units for ``"normal"`` (``0.0075`` = 75 bp on decimal rates). Must be
///     positive.
/// from_convention, to_convention : str or dict
///     ``"normal"``, ``"lognormal"``, or ``{"shifted_lognormal": {"shift": s}}``
///     with ``shift`` in the forward's rate units (the serde form of the Rust
///     ``VolatilityConvention`` enum).
/// forward_rate : float
///     ATM forward rate or price used as both forward and strike. Must be
///     positive for lognormal conventions and satisfy ``forward + shift > 0``
///     for shifted ones.
/// time_to_expiry : float
///     Time to expiry in years (non-negative). At ``0`` the quote is returned
///     unchanged.
///
/// Returns
/// -------
/// float
///     Volatility in the target convention.
///
/// Raises
/// ------
/// ValueError
///     If ``vol`` / ``time_to_expiry`` / a convention string is invalid, or
///     the forward is outside the convention domain.
/// RuntimeError
///     If the price-matching solver fails to converge.
///
/// Examples
/// --------
/// >>> from finstack_quant.models.volatility import convert_atm_volatility
/// >>> ln = convert_atm_volatility(0.01, "normal", "lognormal", 0.05, 1.0)
/// >>> round(convert_atm_volatility(ln, "lognormal", "normal", 0.05, 1.0), 10)
/// 0.01
///
/// Sources
/// -------
/// - Hagan-Lesniewski-Woodward (2002) normal/lognormal ATM equivalence; see docs/REFERENCES.md#hagan-2002-sabr
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

/// Gatheral SVI ("stochastic volatility inspired") total-variance smile parameters.
///
/// ``w(k) = a + b * (rho * (k - m) + sqrt((k - m)^2 + sigma^2))`` in
/// log-moneyness ``k = ln(K / F)``. Construction validates the Gatheral-Jacquier
/// necessary no-arbitrage conditions (``b >= 0``, ``sigma > 0``, ``|rho| < 1``,
/// non-negative minimum variance, Roger Lee moment bound).
///
/// Examples
/// --------
/// >>> from finstack_quant.models.volatility import SviParams
/// >>> p = SviParams(0.04, 0.1, -0.3, 0.0, 0.2)
/// >>> round(p.implied_vol(0.0, 1.0), 6)
/// 0.244949
///
/// Sources
/// -------
/// - Gatheral (2004): see docs/REFERENCES.md#gatheral-2004-svi
/// - Gatheral-Jacquier (2014): see docs/REFERENCES.md#gatheral-jacquier-2014-svi
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
    /// Create validated SVI parameters.
    ///
    /// Parameters
    /// ----------
    /// a : float
    ///     Overall total-variance level.
    /// b : float
    ///     Wing slope (``>= 0``).
    /// rho : float
    ///     Rotation / asymmetry in ``(-1, 1)``.
    /// m : float
    ///     Log-moneyness translation of the minimum-variance point.
    /// sigma : float
    ///     Vertex smoothing (``> 0``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a parameter is non-finite or violates the no-arbitrage
    ///     conditions.
    #[new]
    #[pyo3(signature = (a, b, rho, m, sigma))]
    fn new(a: f64, b: f64, rho: f64, m: f64, sigma: f64) -> PyResult<Self> {
        let inner = SviParams {
            a,
            b,
            rho,
            m,
            sigma,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Overall total-variance level ``a``.
    #[getter]
    fn a(&self) -> f64 {
        self.inner.a
    }

    /// Wing slope ``b``.
    #[getter]
    fn b(&self) -> f64 {
        self.inner.b
    }

    /// Rotation / asymmetry ``rho``.
    #[getter]
    fn rho(&self) -> f64 {
        self.inner.rho
    }

    /// Translation ``m`` of the minimum-variance point (log-moneyness).
    #[getter]
    fn m(&self) -> f64 {
        self.inner.m
    }

    /// Vertex smoothing ``sigma``.
    #[getter]
    fn sigma(&self) -> f64 {
        self.inner.sigma
    }

    /// Total implied variance ``w(k) = vol^2 * T`` at log-moneyness ``k = ln(K / F)``.
    ///
    /// Parameters
    /// ----------
    /// k : float
    ///     Log-moneyness ``ln(K / F)``.
    ///
    /// Returns
    /// -------
    /// float
    ///     Total variance (dimensionless). Does not raise.
    fn total_variance(&self, k: f64) -> f64 {
        self.inner.total_variance(k)
    }

    /// Black implied volatility ``sqrt(w(k) / t)`` at log-moneyness ``k``.
    ///
    /// Parameters
    /// ----------
    /// k : float
    ///     Log-moneyness ``ln(K / F)``.
    /// t : float
    ///     Time to expiry in years (``> 0``).
    ///
    /// Returns
    /// -------
    /// float
    ///     Annualized Black volatility as a decimal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``t <= 0`` or the total variance at ``k`` is negative.
    fn implied_vol(&self, k: f64, t: f64) -> PyResult<f64> {
        self.inner.implied_vol(k, t).map_err(core_to_py)
    }

    /// Durrleman ``g(k)`` butterfly density function at log-moneyness ``k``.
    ///
    /// ``g(k) >= 0`` everywhere is the necessary and sufficient
    /// butterfly-arbitrage-free condition. Does not raise.
    fn durrleman_g(&self, k: f64) -> f64 {
        self.inner.durrleman_g(k)
    }

    /// Serialize to compact JSON (``a, b, rho, m, sigma``).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "SviParams"))
    }

    /// Deserialize from JSON; validation runs on load.
    ///
    /// Raises ``ValueError`` on malformed JSON or invalid parameters.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: SviParams = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid SviParams JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` (and therefore ``copy.deepcopy``, ``multiprocessing``).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("SviParams", &self.inner)
    }
}

/// Calibrate SVI parameters to a market smile (Gatheral 2004).
///
/// Parameters
/// ----------
/// strikes : list[float]
///     Positive strikes (at least five).
/// vols : list[float]
///     Black implied vols (decimal) aligned one-for-one with ``strikes``.
/// forward : float
///     Positive forward at ``expiry``.
/// expiry : float
///     Positive time to expiry in years.
///
/// Returns
/// -------
/// SviParams
///     Fitted, validated parameters.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, fewer than five quotes are supplied, an input is
///     outside its domain, or the fit violates the no-arbitrage conditions.
/// RuntimeError
///     If the optimizer fails to converge or the fit RMSE exceeds the
///     acceptance threshold.
///
/// Examples
/// --------
/// >>> from finstack_quant.models.volatility import calibrate_svi
/// >>> strikes = [80.0, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0]
/// >>> vols = [0.30, 0.25, 0.22, 0.20, 0.21, 0.23, 0.28]
/// >>> p = calibrate_svi(strikes, vols, 100.0, 1.0)
/// >>> abs(p.implied_vol(0.0, 1.0) - 0.20) < 0.02
/// True
///
/// Sources
/// -------
/// - Gatheral (2004): see docs/REFERENCES.md#gatheral-2004-svi
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
    let m = PyModule::new(py, "volatility")?;
    m.setattr(
        "__doc__",
        "Product-independent volatility models, evaluators, fitting, and convention conversion.",
    )?;
    m.add_class::<PySabrParameters>()?;
    m.add_class::<PySabrModel>()?;
    m.add_class::<PySabrSmile>()?;
    m.add_class::<PySabrCalibrator>()?;
    m.add_class::<PySviParams>()?;
    m.add_function(wrap_pyfunction!(calibrate_svi, &m)?)?;
    m.add_function(wrap_pyfunction!(convert_atm_volatility, &m)?)?;
    m.add_function(wrap_pyfunction!(surface_to_dataframe, &m)?)?;
    m.add_function(wrap_pyfunction!(get_surface_vol, &m)?)?;
    m.add_function(wrap_pyfunction!(get_surface_vol_clamped, &m)?)?;
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
    m.add_function(wrap_pyfunction!(strike_to_delta, &m)?)?;
    super::volatility_arbitrage::register(py, &m)?;
    m.setattr(
        "__all__",
        PyList::new(
            py,
            [
                "ArbitrageReport",
                "SabrCalibrator",
                "SabrModel",
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
                "get_cube_normal_vol",
                "get_cube_normal_vol_clamped",
                "get_cube_vol",
                "get_cube_vol_clamped",
                "get_fx_delta_pillar_vols",
                "get_fx_delta_vol",
                "get_surface_vol",
                "get_surface_vol_clamped",
                "materialize_cube_expiry_slice",
                "materialize_cube_expiry_slice_normal",
                "materialize_cube_tenor_slice",
                "materialize_cube_tenor_slice_normal",
                "materialize_fx_delta_surface",
                "strike_to_delta",
                "surface_to_dataframe",
            ],
        )?,
    )?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "volatility",
        "finstack_quant.models",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
