//! `finstack_quant.calibration.hull_white`: direct Hull-White one-factor calibrators.

use super::report::PyCalibrationReport;
use crate::bindings::core::market_data::curves::PyDiscountCurve;
use crate::bindings::pickle_support::reduce_via_json;
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_calibration::hull_white::{
    self as rust_hw, CapFloorCalibrationConfig, CapFloorQuote, HullWhiteCalibrationParams,
    HullWhiteParams, PiecewiseSigmaCalibrationConfig, SwapFrequency, SwaptionQuote,
};
use finstack_quant_core::market_data::DiscountCurve;
use pyo3::prelude::*;
use pyo3::types::PyList;
use std::sync::Arc;

/// `__reduce__` payload for the swaption calibration config: callable plus its
/// `(frequency, fixed_kappa, initial_guess)` constructor arguments.
type ScalarConfigReduce<'py> = (
    Bound<'py, PyAny>,
    (
        f64,
        String,
        Option<f64>,
        Option<PyHullWhiteCalibrationParams>,
    ),
);

/// `__reduce__` payload for the cap/floor calibration config: callable plus its
/// `(fixed_kappa, sigma_min, sigma_max, frequency)` constructor arguments.
type PiecewiseConfigReduce<'py> = (Bound<'py, PyAny>, (f64, f64, f64, f64, String));

/// Docstring for the `finstack_quant.calibration.hull_white` namespace.
const MODULE_DOC: &str = "Direct Hull-White one-factor calibrators (swaptions, caps/floors, piecewise sigma).\n\nThese take a calibrated DiscountCurve and year-fraction quotes; the plan-level\nequivalents are the `hull_white` / `cap_floor_hull_white` calibration steps.\n\nExamples\n--------\n>>> from finstack_quant.calibration.hull_white import SwaptionQuote\n>>> SwaptionQuote(1.0, 5.0, 0.0065).expiry\n1.0\n";

fn parse_frequency(value: &str) -> PyResult<SwapFrequency> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).map_err(|_| {
        value_error(format!(
            "invalid swap frequency '{value}': expected 'annual', 'semi_annual' or 'quarterly'"
        ))
    })
}

fn frequency_name(value: SwapFrequency) -> String {
    value.to_string()
}

/// At-the-money swaption volatility quote in year fractions.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration.hull_white import SwaptionQuote
/// >>> q = SwaptionQuote(1.0, 5.0, 0.0065, is_normal_vol=True)
/// >>> (q.expiry, q.tenor, q.volatility, q.is_normal_vol)
/// (1.0, 5.0, 0.0065, True)
#[pyclass(
    name = "SwaptionQuote",
    module = "finstack_quant.calibration.hull_white",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySwaptionQuote {
    pub(crate) inner: SwaptionQuote,
}

#[pymethods]
impl PySwaptionQuote {
    /// Build a validated swaption quote.
    ///
    /// Parameters
    /// ----------
    /// expiry : float
    ///     Option expiry in years (> 0).
    /// tenor : float
    ///     Underlying swap tenor in years (> 0).
    /// volatility : float
    ///     Normal (absolute, e.g. ``0.0065``) or lognormal (decimal) volatility (> 0).
    /// is_normal_vol : bool, default True
    ///     ``True`` for a normal (Bachelier) quote, ``False`` for Black lognormal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any input is not positive and finite.
    #[new]
    #[pyo3(signature = (expiry, tenor, volatility, is_normal_vol = true))]
    #[pyo3(text_signature = "(expiry, tenor, volatility, is_normal_vol=True)")]
    fn new(expiry: f64, tenor: f64, volatility: f64, is_normal_vol: bool) -> PyResult<Self> {
        SwaptionQuote::try_new(expiry, tenor, volatility, is_normal_vol)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Option expiry in years.
    #[getter]
    fn expiry(&self) -> f64 {
        self.inner.expiry
    }

    /// Underlying swap tenor in years.
    #[getter]
    fn tenor(&self) -> f64 {
        self.inner.tenor
    }

    /// Quoted volatility.
    #[getter]
    fn volatility(&self) -> f64 {
        self.inner.volatility
    }

    /// Whether the volatility is normal (``True``) or lognormal.
    #[getter]
    fn is_normal_vol(&self) -> bool {
        self.inner.is_normal_vol
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize SwaptionQuote"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or the quote is invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| serde_json_to_py(e, "invalid SwaptionQuote JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "SwaptionQuote(expiry={}, tenor={}, volatility={}, is_normal_vol={})",
            self.inner.expiry,
            self.inner.tenor,
            self.inner.volatility,
            if self.inner.is_normal_vol {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// Cap or floor volatility quote in year fractions.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration.hull_white import CapFloorQuote
/// >>> q = CapFloorQuote(5.0, 0.04, 0.0070)
/// >>> (q.maturity, q.strike, q.is_cap)
/// (5.0, 0.04, True)
#[pyclass(
    name = "CapFloorQuote",
    module = "finstack_quant.calibration.hull_white",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCapFloorQuote {
    pub(crate) inner: CapFloorQuote,
}

#[pymethods]
impl PyCapFloorQuote {
    /// Build a validated cap/floor quote.
    ///
    /// Parameters
    /// ----------
    /// maturity : float
    ///     Cap/floor maturity in years (> 0).
    /// strike : float
    ///     Strike rate as a decimal.
    /// volatility : float
    ///     Normal (absolute) or lognormal (decimal) volatility (> 0).
    /// is_cap : bool, default True
    ///     ``True`` for a cap, ``False`` for a floor.
    /// is_normal_vol : bool, default True
    ///     ``True`` for a normal (Bachelier) quote, ``False`` for Black lognormal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the maturity or volatility is not positive and finite, or the
    ///     strike is not finite (lognormal quotes also require a positive strike).
    #[new]
    #[pyo3(signature = (maturity, strike, volatility, is_cap = true, is_normal_vol = true))]
    #[pyo3(text_signature = "(maturity, strike, volatility, is_cap=True, is_normal_vol=True)")]
    fn new(
        maturity: f64,
        strike: f64,
        volatility: f64,
        is_cap: bool,
        is_normal_vol: bool,
    ) -> PyResult<Self> {
        CapFloorQuote::try_new(maturity, strike, volatility, is_cap, is_normal_vol)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Maturity in years.
    #[getter]
    fn maturity(&self) -> f64 {
        self.inner.maturity
    }

    /// Strike rate (decimal).
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    /// Quoted volatility.
    #[getter]
    fn volatility(&self) -> f64 {
        self.inner.volatility
    }

    /// ``True`` for a cap, ``False`` for a floor.
    #[getter]
    fn is_cap(&self) -> bool {
        self.inner.is_cap
    }

    /// Whether the volatility is normal (``True``) or lognormal.
    #[getter]
    fn is_normal_vol(&self) -> bool {
        self.inner.is_normal_vol
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CapFloorQuote"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or the quote is invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| serde_json_to_py(e, "invalid CapFloorQuote JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CapFloorQuote(maturity={}, strike={}, volatility={}, is_cap={}, is_normal_vol={})",
            self.inner.maturity,
            self.inner.strike,
            self.inner.volatility,
            if self.inner.is_cap { "True" } else { "False" },
            if self.inner.is_normal_vol {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// Scalar Hull-White one-factor parameters: mean reversion ``kappa`` and volatility ``sigma``.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration.hull_white import HullWhiteCalibrationParams
/// >>> p = HullWhiteCalibrationParams(0.03, 0.01)
/// >>> (p.kappa, p.sigma)
/// (0.03, 0.01)
#[pyclass(
    name = "HullWhiteCalibrationParams",
    module = "finstack_quant.calibration.hull_white",
    frozen,
    skip_from_py_object,
    eq
)]
#[derive(Clone, PartialEq)]
pub struct PyHullWhiteCalibrationParams {
    pub(crate) inner: HullWhiteCalibrationParams,
}

#[pymethods]
impl PyHullWhiteCalibrationParams {
    /// Build validated scalar parameters.
    ///
    /// Parameters
    /// ----------
    /// kappa : float
    ///     Mean-reversion speed (> 0, per year).
    /// sigma : float
    ///     Short-rate volatility (> 0, absolute annualized).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If either parameter is not positive and finite.
    #[new]
    #[pyo3(text_signature = "(kappa, sigma)")]
    fn new(kappa: f64, sigma: f64) -> PyResult<Self> {
        HullWhiteCalibrationParams::new(kappa, sigma)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Mean-reversion speed.
    #[getter]
    fn kappa(&self) -> f64 {
        self.inner.kappa
    }

    /// Short-rate volatility.
    #[getter]
    fn sigma(&self) -> f64 {
        self.inner.sigma
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize HullWhiteCalibrationParams"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or the parameters are invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| serde_json_to_py(e, "invalid HullWhiteCalibrationParams JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "HullWhiteCalibrationParams(kappa={}, sigma={})",
            self.inner.kappa, self.inner.sigma
        )
    }
}

/// Hull-White parameters with a piecewise-constant sigma term structure.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration.hull_white import HullWhiteParams
/// >>> p = HullWhiteParams.from_json('{"kappa": 0.03, "volatility": {"times": [1.0, 2.0], "values": [0.01, 0.012]}}')
/// >>> (p.kappa, p.times, p.values)
/// (0.03, [1.0, 2.0], [0.01, 0.012])
#[pyclass(
    name = "HullWhiteParams",
    module = "finstack_quant.calibration.hull_white",
    frozen,
    skip_from_py_object,
    eq
)]
#[derive(Clone, PartialEq)]
pub struct PyHullWhiteParams {
    pub(crate) inner: HullWhiteParams,
}

#[pymethods]
impl PyHullWhiteParams {
    /// Mean-reversion speed.
    #[getter]
    fn kappa(&self) -> f64 {
        self.inner.kappa
    }

    /// Right end-points (years) of the constant-sigma intervals.
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.volatility.times().to_vec()
    }

    /// Sigma on each interval.
    #[getter]
    fn values(&self) -> Vec<f64> {
        self.inner.volatility.values().to_vec()
    }

    /// Sigma applying at ``time`` (years).
    ///
    /// Parameters
    /// ----------
    /// time : float
    ///     Year fraction at which the piecewise sigma is read.
    #[pyo3(text_signature = "($self, time)")]
    fn sigma_at(&self, time: f64) -> f64 {
        self.inner.volatility.value_at(time)
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize HullWhiteParams"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or the parameters are invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| serde_json_to_py(e, "invalid HullWhiteParams JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "HullWhiteParams(kappa={}, intervals={})",
            self.inner.kappa,
            self.inner.volatility.times().len()
        )
    }
}

/// Settings for scalar Hull-White calibration to cap/floor quotes.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration.hull_white import CapFloorCalibrationConfig
/// >>> cfg = CapFloorCalibrationConfig(frequency="quarterly", fixed_kappa=0.05)
/// >>> (cfg.frequency, cfg.fixed_kappa)
/// ('quarterly', 0.05)
#[pyclass(
    name = "CapFloorCalibrationConfig",
    module = "finstack_quant.calibration.hull_white",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCapFloorCalibrationConfig {
    pub(crate) inner: CapFloorCalibrationConfig,
}

#[pymethods]
impl PyCapFloorCalibrationConfig {
    /// Build cap/floor calibration settings.
    ///
    /// Parameters
    /// ----------
    /// fit_tolerance : float
    ///     Required positive maximum implied normal-vol quote error in decimal rate units.
    /// frequency : str, default "semi_annual"
    ///     Caplet payment frequency: ``"annual"``, ``"semi_annual"`` or ``"quarterly"``.
    /// fixed_kappa : float | None, default None
    ///     Hold mean reversion fixed at this value and solve sigma only.
    /// initial_guess : HullWhiteCalibrationParams | None, default None
    ///     Starting point for the two-parameter solve.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``frequency`` is not one of the accepted strings.
    #[new]
    #[pyo3(signature = (fit_tolerance, frequency = "semi_annual", fixed_kappa = None, initial_guess = None))]
    #[pyo3(
        text_signature = "(fit_tolerance, frequency='semi_annual', fixed_kappa=None, initial_guess=None)"
    )]
    fn new(
        fit_tolerance: f64,
        frequency: &str,
        fixed_kappa: Option<f64>,
        initial_guess: Option<PyRef<'_, PyHullWhiteCalibrationParams>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CapFloorCalibrationConfig {
                fit_tolerance,
                frequency: parse_frequency(frequency)?,
                fixed_kappa,
                initial_guess: initial_guess.map(|p| p.inner),
            },
        })
    }

    /// Maximum accepted implied normal-vol quote error in decimal rate units.
    #[getter]
    fn fit_tolerance(&self) -> f64 {
        self.inner.fit_tolerance
    }

    /// Caplet payment frequency name.
    #[getter]
    fn frequency(&self) -> String {
        frequency_name(self.inner.frequency)
    }

    /// Fixed mean reversion, when set.
    #[getter]
    fn fixed_kappa(&self) -> Option<f64> {
        self.inner.fixed_kappa
    }

    /// Initial guess, when set.
    #[getter]
    fn initial_guess(&self) -> Option<PyHullWhiteCalibrationParams> {
        self.inner
            .initial_guess
            .map(|inner| PyHullWhiteCalibrationParams { inner })
    }

    /// Pickle support through the constructor arguments.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<ScalarConfigReduce<'py>> {
        Ok((
            py.get_type::<Self>().into_any(),
            (
                self.inner.fit_tolerance,
                self.frequency(),
                self.fixed_kappa(),
                self.initial_guess(),
            ),
        ))
    }

    fn __repr__(&self) -> String {
        format!(
            "CapFloorCalibrationConfig(fit_tolerance={}, frequency={:?}, fixed_kappa={}, initial_guess={})",
            self.inner.fit_tolerance, self.frequency(),
            self.inner
                .fixed_kappa
                .map_or("None".to_string(), |k| k.to_string()),
            self.initial_guess()
                .map_or("None".to_string(), |p| p.__repr__())
        )
    }
}

/// Settings for the piecewise-constant sigma bootstrap to cap/floor quotes.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration.hull_white import PiecewiseSigmaCalibrationConfig
/// >>> cfg = PiecewiseSigmaCalibrationConfig(0.03, 1e-4, 0.1, 1e-4)
/// >>> (cfg.fixed_kappa, cfg.frequency)
/// (0.03, 'semi_annual')
#[pyclass(
    name = "PiecewiseSigmaCalibrationConfig",
    module = "finstack_quant.calibration.hull_white",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPiecewiseSigmaCalibrationConfig {
    pub(crate) inner: PiecewiseSigmaCalibrationConfig,
}

#[pymethods]
impl PyPiecewiseSigmaCalibrationConfig {
    /// Build piecewise-sigma bootstrap settings.
    ///
    /// Parameters
    /// ----------
    /// fixed_kappa : float
    ///     Mean reversion held fixed during the bootstrap (> 0).
    /// sigma_min : float
    ///     Lower bracket for each interval's sigma (> 0).
    /// sigma_max : float
    ///     Upper bracket for each interval's sigma (> ``sigma_min``).
    /// fit_tolerance : float
    ///     Required positive maximum implied normal-vol quote error in decimal rate units.
    /// frequency : str, default "semi_annual"
    ///     Caplet payment frequency: ``"annual"``, ``"semi_annual"`` or ``"quarterly"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``frequency`` is not one of the accepted strings. Numeric bounds
    ///     are validated when the bootstrap runs.
    #[new]
    #[pyo3(signature = (fixed_kappa, sigma_min, sigma_max, fit_tolerance, frequency = "semi_annual"))]
    #[pyo3(
        text_signature = "(fixed_kappa, sigma_min, sigma_max, fit_tolerance, frequency='semi_annual')"
    )]
    fn new(
        fixed_kappa: f64,
        sigma_min: f64,
        sigma_max: f64,
        fit_tolerance: f64,
        frequency: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: PiecewiseSigmaCalibrationConfig {
                fit_tolerance,
                fixed_kappa,
                sigma_min,
                sigma_max,
                frequency: parse_frequency(frequency)?,
            },
        })
    }

    /// Maximum accepted implied normal-vol quote error in decimal rate units.
    #[getter]
    fn fit_tolerance(&self) -> f64 {
        self.inner.fit_tolerance
    }

    /// Fixed mean reversion.
    #[getter]
    fn fixed_kappa(&self) -> f64 {
        self.inner.fixed_kappa
    }

    /// Lower sigma bracket.
    #[getter]
    fn sigma_min(&self) -> f64 {
        self.inner.sigma_min
    }

    /// Upper sigma bracket.
    #[getter]
    fn sigma_max(&self) -> f64 {
        self.inner.sigma_max
    }

    /// Caplet payment frequency name.
    #[getter]
    fn frequency(&self) -> String {
        frequency_name(self.inner.frequency)
    }

    /// Pickle support through the constructor arguments.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<PiecewiseConfigReduce<'py>> {
        Ok((
            py.get_type::<Self>().into_any(),
            (
                self.inner.fixed_kappa,
                self.inner.sigma_min,
                self.inner.sigma_max,
                self.inner.fit_tolerance,
                self.frequency(),
            ),
        ))
    }

    fn __repr__(&self) -> String {
        format!(
            "PiecewiseSigmaCalibrationConfig(fixed_kappa={}, sigma_min={}, sigma_max={}, fit_tolerance={}, frequency={:?})",
            self.inner.fixed_kappa, self.inner.sigma_min, self.inner.sigma_max, self.inner.fit_tolerance, self.frequency()
        )
    }
}

fn curve_of(curve: &Bound<'_, PyAny>, label: &str) -> PyResult<Arc<DiscountCurve>> {
    curve
        .cast::<PyDiscountCurve>()
        .map(|c| Arc::clone(&c.borrow().inner))
        .map_err(|_| value_error(format!("{label} must be a core.market_data.DiscountCurve")))
}

fn swaption_quotes(quotes: Vec<PyRef<'_, PySwaptionQuote>>) -> Vec<SwaptionQuote> {
    quotes.into_iter().map(|q| q.inner).collect()
}

fn cap_floor_quotes(quotes: Vec<PyRef<'_, PyCapFloorQuote>>) -> Vec<CapFloorQuote> {
    quotes.into_iter().map(|q| q.inner).collect()
}

/// Fit scalar Hull-White (kappa, sigma) to at-the-money swaption quotes.
///
/// Parameters
/// ----------
/// discount : DiscountCurve
///     Discount curve the swap annuities and forwards are read from.
/// quotes : list[SwaptionQuote]
///     At least two swaption quotes (two free parameters).
/// fit_tolerance : float
///     Required positive maximum reconstructed quote error: decimal rate volatility
///     for normal quotes, relative volatility for Black quotes. Independent of solver tolerance.
/// frequency : str, default "semi_annual"
///     Fixed-leg payment frequency: ``"annual"``, ``"semi_annual"`` or ``"quarterly"``.
/// initial_guess : HullWhiteCalibrationParams | None, default None
///     Starting point for the solver; Rust defaults when ``None``.
///
/// Returns
/// -------
/// tuple[HullWhiteCalibrationParams, CalibrationReport]
///     Fitted parameters and the fit report (residuals per quote in volatility units).
///
/// Raises
/// ------
/// ValueError
///     If fewer than two quotes are supplied, ``frequency`` is invalid, or
///     ``discount`` is not a ``DiscountCurve``.
/// RuntimeError
///     If the solver fails to converge.
#[pyfunction]
#[pyo3(signature = (discount, quotes, fit_tolerance, frequency = "semi_annual", initial_guess = None))]
#[pyo3(
    text_signature = "(discount, quotes, fit_tolerance, frequency='semi_annual', initial_guess=None)"
)]
fn calibrate_hull_white_to_swaptions(
    py: Python<'_>,
    discount: &Bound<'_, PyAny>,
    quotes: Vec<PyRef<'_, PySwaptionQuote>>,
    fit_tolerance: f64,
    frequency: &str,
    initial_guess: Option<PyRef<'_, PyHullWhiteCalibrationParams>>,
) -> PyResult<(PyHullWhiteCalibrationParams, PyCalibrationReport)> {
    let curve = curve_of(discount, "discount")?;
    let quotes = swaption_quotes(quotes);
    let frequency = parse_frequency(frequency)?;
    let initial_guess = initial_guess.map(|p| p.inner);
    let (params, report) = py
        .detach(move || {
            let model_curve = finstack_quant_models::rates::clock::ModelDiscountCurve::new(
                curve.as_ref(),
                curve.base_date(),
            )?;
            let df = |t| model_curve.get_df(t).unwrap_or(f64::NAN);
            rust_hw::calibrate_hull_white_to_swaptions(
                &df,
                &quotes,
                frequency,
                None,
                initial_guess,
                fit_tolerance,
            )
        })
        .map_err(core_to_py)?;
    Ok((
        PyHullWhiteCalibrationParams { inner: params },
        PyCalibrationReport::from_inner(report),
    ))
}

/// Fit scalar Hull-White (kappa, sigma) to cap/floor quotes.
///
/// Parameters
/// ----------
/// discount : DiscountCurve
///     Discounting curve.
/// forward : DiscountCurve | None, default None
///     Curve projecting the caplet forwards; ``discount`` when ``None``.
/// quotes : list[CapFloorQuote]
///     Cap/floor quotes (one quote requires ``config.fixed_kappa``).
/// config : CapFloorCalibrationConfig
///     Required fit tolerance in normal-vol units, frequency, fixed kappa and initial guess.
///
/// Returns
/// -------
/// tuple[HullWhiteCalibrationParams, CalibrationReport]
///     Fitted parameters and the fit report.
///
/// Raises
/// ------
/// ValueError
///     If no quotes are supplied, a single quote is given without
///     ``fixed_kappa``, or a curve argument is not a ``DiscountCurve``.
/// RuntimeError
///     If the solver fails to converge.
#[pyfunction]
#[pyo3(signature = (discount, quotes, config, forward = None))]
#[pyo3(text_signature = "(discount, quotes, config, forward=None)")]
fn calibrate_hull_white_to_cap_floors(
    py: Python<'_>,
    discount: &Bound<'_, PyAny>,
    quotes: Vec<PyRef<'_, PyCapFloorQuote>>,
    config: PyRef<'_, PyCapFloorCalibrationConfig>,
    forward: Option<&Bound<'_, PyAny>>,
) -> PyResult<(PyHullWhiteCalibrationParams, PyCalibrationReport)> {
    let discount = curve_of(discount, "discount")?;
    let forward = match forward {
        Some(curve) if !curve.is_none() => curve_of(curve, "forward")?,
        _ => Arc::clone(&discount),
    };
    let quotes = cap_floor_quotes(quotes);
    let config = config.inner;
    let (params, report) = py
        .detach(move || {
            let model_discount = finstack_quant_models::rates::clock::ModelDiscountCurve::new(
                discount.as_ref(),
                discount.base_date(),
            )?;
            let model_forward = finstack_quant_models::rates::clock::ModelDiscountCurve::new(
                forward.as_ref(),
                discount.base_date(),
            )?;
            let discount_df = |t| model_discount.get_df(t).unwrap_or(f64::NAN);
            let forward_df = |t| model_forward.get_df(t).unwrap_or(f64::NAN);
            rust_hw::calibrate_hull_white_to_cap_floors(&discount_df, &forward_df, &quotes, config)
        })
        .map_err(core_to_py)?;
    Ok((
        PyHullWhiteCalibrationParams { inner: params },
        PyCalibrationReport::from_inner(report),
    ))
}

/// Bootstrap a piecewise-constant Hull-White sigma schedule to cap/floor quotes.
///
/// Parameters
/// ----------
/// discount : DiscountCurve
///     Discounting curve.
/// quotes : list[CapFloorQuote]
///     Cap/floor quotes with strictly increasing maturities; each maturity
///     adds one sigma interval.
/// config : PiecewiseSigmaCalibrationConfig
///     Fixed kappa, sigma brackets and payment frequency.
/// forward : DiscountCurve | None, default None
///     Curve projecting the caplet forwards; ``discount`` when ``None``.
///
/// Returns
/// -------
/// tuple[HullWhiteParams, CalibrationReport]
///     Piecewise parameters and the fit report.
///
/// Raises
/// ------
/// ValueError
///     If no quotes are supplied, the configuration bounds are invalid, or a
///     curve argument is not a ``DiscountCurve``.
/// RuntimeError
///     If an interval's sigma cannot be bracketed or solved.
#[pyfunction]
#[pyo3(signature = (discount, quotes, config, forward = None))]
#[pyo3(text_signature = "(discount, quotes, config, forward=None)")]
fn bootstrap_hull_white_sigma_schedule_to_cap_floors(
    py: Python<'_>,
    discount: &Bound<'_, PyAny>,
    quotes: Vec<PyRef<'_, PyCapFloorQuote>>,
    config: PyRef<'_, PyPiecewiseSigmaCalibrationConfig>,
    forward: Option<&Bound<'_, PyAny>>,
) -> PyResult<(PyHullWhiteParams, PyCalibrationReport)> {
    let discount = curve_of(discount, "discount")?;
    let forward = match forward {
        Some(curve) if !curve.is_none() => curve_of(curve, "forward")?,
        _ => Arc::clone(&discount),
    };
    let quotes = cap_floor_quotes(quotes);
    let config = config.inner;
    let (params, report) = py
        .detach(move || {
            let model_discount = finstack_quant_models::rates::clock::ModelDiscountCurve::new(
                discount.as_ref(),
                discount.base_date(),
            )?;
            let model_forward = finstack_quant_models::rates::clock::ModelDiscountCurve::new(
                forward.as_ref(),
                discount.base_date(),
            )?;
            let discount_df = |t| model_discount.get_df(t).unwrap_or(f64::NAN);
            let forward_df = |t| model_forward.get_df(t).unwrap_or(f64::NAN);
            rust_hw::bootstrap_hull_white_sigma_schedule_to_cap_floors(
                &discount_df,
                &forward_df,
                &quotes,
                config,
            )
        })
        .map_err(core_to_py)?;
    Ok((
        PyHullWhiteParams { inner: params },
        PyCalibrationReport::from_inner(report),
    ))
}

/// Register the `finstack_quant.calibration.hull_white` namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "hull_white")?;
    m.setattr("__doc__", MODULE_DOC)?;
    m.add_class::<PyCapFloorCalibrationConfig>()?;
    m.add_class::<PyCapFloorQuote>()?;
    m.add_class::<PyHullWhiteCalibrationParams>()?;
    m.add_class::<PyHullWhiteParams>()?;
    m.add_class::<PyPiecewiseSigmaCalibrationConfig>()?;
    m.add_class::<PySwaptionQuote>()?;
    m.add_function(pyo3::wrap_pyfunction!(
        bootstrap_hull_white_sigma_schedule_to_cap_floors,
        &m
    )?)?;
    m.add_function(pyo3::wrap_pyfunction!(
        calibrate_hull_white_to_cap_floors,
        &m
    )?)?;
    m.add_function(pyo3::wrap_pyfunction!(
        calibrate_hull_white_to_swaptions,
        &m
    )?)?;
    m.setattr(
        "__all__",
        PyList::new(
            py,
            [
                "CapFloorCalibrationConfig",
                "CapFloorQuote",
                "HullWhiteCalibrationParams",
                "HullWhiteParams",
                "PiecewiseSigmaCalibrationConfig",
                "SwaptionQuote",
                "bootstrap_hull_white_sigma_schedule_to_cap_floors",
                "calibrate_hull_white_to_cap_floors",
                "calibrate_hull_white_to_swaptions",
            ],
        )?,
    )?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "hull_white",
        "finstack_quant.calibration",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
