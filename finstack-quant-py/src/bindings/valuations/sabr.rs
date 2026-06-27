//! SABR (Stochastic Alpha Beta Rho) volatility bindings.
//!
//! Exposes four classes from `finstack_quant_valuations::models::volatility::sabr`:
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
//! Rust uses the all-caps acronym `SABR*`; Python and JS bindings surface the
//! PascalCase forms (`SabrParameters`, `SabrModel`, `SabrSmile`,
//! `SabrCalibrator`) to match standard Python class-naming conventions. The
//! Rust-to-Python name alignment rule from `AGENTS.md` targets snake_case
//! functions; class-name casing is left to the binding layer.

use crate::errors::display_to_py;
use finstack_quant_valuations::models::volatility::sabr::{
    SABRCalibrator, SABRModel, SABRParameters, SABRSmile,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

// ---------------------------------------------------------------------------
// SabrParameters
// ---------------------------------------------------------------------------

/// SABR model parameters ``(alpha, beta, nu, rho)`` with optional ``shift``.
///
/// Constructed with validation: ``alpha > 0``, ``beta in [0, 1]``,
/// ``nu >= 0``, ``rho in [-1, 1]``, and when supplied ``shift > 0``.
#[pyclass(
    name = "SabrParameters",
    module = "finstack_quant.valuations",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PySabrParameters {
    pub(crate) inner: SABRParameters,
}

#[pymethods]
impl PySabrParameters {
    /// Create a new ``SabrParameters`` instance.
    ///
    /// Parameters
    /// ----------
    /// alpha : float
    ///     Initial volatility. Must be strictly positive.
    /// beta : float
    ///     CEV exponent. Must be in ``[0, 1]``.
    /// nu : float
    ///     Volatility of volatility. Must be non-negative.
    /// rho : float
    ///     Correlation between asset and volatility. Must be in ``[-1, 1]``.
    /// shift : float, optional
    ///     Shift parameter for negative-rate support. When provided must be
    ///     strictly positive.
    #[new]
    #[pyo3(signature = (alpha, beta, nu, rho, shift=None))]
    fn new(alpha: f64, beta: f64, nu: f64, rho: f64, shift: Option<f64>) -> PyResult<Self> {
        let inner = match shift {
            Some(s) => SABRParameters::new_with_shift(alpha, beta, nu, rho, s),
            None => SABRParameters::new(alpha, beta, nu, rho),
        }
        .map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Equity-standard defaults: ``alpha=0.20, beta=1.0, nu=0.30, rho=-0.20``.
    #[staticmethod]
    fn equity_default() -> Self {
        Self {
            inner: SABRParameters::equity_default(),
        }
    }

    /// Rates-standard defaults: ``alpha=0.02, beta=0.5, nu=0.30, rho=0.0``.
    #[staticmethod]
    fn rates_default() -> Self {
        Self {
            inner: SABRParameters::rates_default(),
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

// ---------------------------------------------------------------------------
// SabrModel
// ---------------------------------------------------------------------------

/// Hagan-2002 SABR model wrapping a :class:`SabrParameters` instance.
#[pyclass(name = "SabrModel", module = "finstack_quant.valuations")]
pub struct PySabrModel {
    pub(crate) inner: SABRModel,
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
            inner: SABRModel::new(params.inner),
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
    ///     Black-style implied volatility (annualised decimal).
    fn implied_vol(&self, forward: f64, strike: f64, t: f64) -> PyResult<f64> {
        self.inner
            .implied_volatility(forward, strike, t)
            .map_err(display_to_py)
    }

    /// Parameters used by this model.
    #[getter]
    fn params(&self) -> PySabrParameters {
        PySabrParameters {
            inner: self.inner.parameters().clone(),
        }
    }

    /// ``True`` when the underlying parameters include a non-zero shift.
    fn supports_negative_rates(&self) -> bool {
        self.inner.supports_negative_rates()
    }

    fn __repr__(&self) -> String {
        let p = self.inner.parameters();
        format!(
            "SabrModel(alpha={}, beta={}, nu={}, rho={})",
            p.alpha, p.beta, p.nu, p.rho
        )
    }
}

// ---------------------------------------------------------------------------
// SabrSmile
// ---------------------------------------------------------------------------

/// Volatility smile generator for a fixed ``(forward, t)`` pair.
#[pyclass(name = "SabrSmile", module = "finstack_quant.valuations")]
pub struct PySabrSmile {
    inner: SABRSmile,
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
        let model = SABRModel::new(params.inner);
        Self {
            inner: SABRSmile::new(model, forward, t),
        }
    }

    /// At-the-money implied volatility.
    fn atm_vol(&self) -> PyResult<f64> {
        self.inner.atm_vol().map_err(display_to_py)
    }

    /// Implied volatility at a single strike.
    fn implied_vol(&self, strike: f64) -> PyResult<f64> {
        self.inner
            .generate_smile(&[strike])
            .map(|v| v[0])
            .map_err(display_to_py)
    }

    /// Generate implied volatilities for a vector of strikes.
    ///
    /// Returns
    /// -------
    /// list[float]
    ///     Implied volatilities in strike order.
    fn generate_smile(&self, strikes: Vec<f64>) -> PyResult<Vec<f64>> {
        self.inner.generate_smile(&strikes).map_err(display_to_py)
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
            .map_err(display_to_py)?;

        let butterflies = PyList::empty(py);
        for v in &result.butterfly_violations {
            let item = PyDict::new(py);
            item.set_item("strike", v.strike)?;
            item.set_item("butterfly_value", v.butterfly_value)?;
            item.set_item("severity_pct", v.severity_pct)?;
            butterflies.append(item)?;
        }
        let mono = PyList::empty(py);
        for v in &result.monotonicity_violations {
            let item = PyDict::new(py);
            item.set_item("strike_low", v.strike_low)?;
            item.set_item("strike_high", v.strike_high)?;
            item.set_item("price_low", v.price_low)?;
            item.set_item("price_high", v.price_high)?;
            mono.append(item)?;
        }

        let out = PyDict::new(py);
        out.set_item("arbitrage_free", result.is_arbitrage_free())?;
        out.set_item("butterfly_violations", butterflies)?;
        out.set_item("monotonicity_violations", mono)?;
        Ok(out)
    }

    fn __repr__(&self) -> String {
        "SabrSmile".to_string()
    }
}

// ---------------------------------------------------------------------------
// SabrCalibrator
// ---------------------------------------------------------------------------

/// SABR calibrator using Levenberg-Marquardt with beta fixed.
#[pyclass(name = "SabrCalibrator", module = "finstack_quant.valuations")]
pub struct PySabrCalibrator {
    inner: SABRCalibrator,
}

impl Default for PySabrCalibrator {
    fn default() -> Self {
        Self {
            inner: SABRCalibrator::new(),
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
            inner: SABRCalibrator::high_precision(),
        }
    }

    /// Override the convergence tolerance.
    ///
    /// Preserves all other previously-set fields (e.g. the ``max_iter=200``
    /// from :meth:`high_precision`) by cloning the existing calibrator and
    /// only adjusting the tolerance.
    fn with_tolerance(&self, tolerance: f64) -> Self {
        Self {
            inner: self.inner.clone().with_tolerance(tolerance),
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
        check_smile_lengths(&strikes, &market_vols)?;
        py.detach(|| {
            self.inner
                .calibrate(forward, &strikes, &market_vols, t, beta)
        })
        .map(|inner| PySabrParameters { inner })
        .map_err(display_to_py)
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
        check_smile_lengths(&strikes, &market_vols)?;
        py.detach(|| {
            self.inner
                .calibrate_auto_shift(forward, &strikes, &market_vols, t, beta)
        })
        .map(|inner| PySabrParameters { inner })
        .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        "SabrCalibrator".to_string()
    }
}

fn check_smile_lengths(strikes: &[f64], market_vols: &[f64]) -> PyResult<()> {
    if strikes.len() != market_vols.len() {
        return Err(crate::errors::value_error(format!(
            "strikes length ({}) must match market_vols length ({})",
            strikes.len(),
            market_vols.len()
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the SABR classes on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySabrParameters>()?;
    m.add_class::<PySabrModel>()?;
    m.add_class::<PySabrSmile>()?;
    m.add_class::<PySabrCalibrator>()?;
    Ok(())
}
