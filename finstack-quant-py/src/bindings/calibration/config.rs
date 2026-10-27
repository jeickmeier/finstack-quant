//! Typed calibration settings: `CalibrationConfig`, `SolverConfig`, `ValidationConfig`, `RateBounds`.

use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_calibration::validation::{RateBounds, ValidationConfig};
use finstack_quant_calibration::{CalibrationConfig, SolverConfig};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

/// Extract settings from `CalibrationConfig | dict | None` (None → Rust defaults).
pub(crate) fn extract_config(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<CalibrationConfig> {
    let Some(obj) = obj else {
        return Ok(CalibrationConfig::default());
    };
    if obj.is_none() {
        return Ok(CalibrationConfig::default());
    }
    if let Ok(config) = obj.cast::<PyCalibrationConfig>() {
        return Ok(config.borrow().inner.clone());
    }
    let overrides = py_to_json_value(py, obj, "calibration settings")?;
    CalibrationConfig::default()
        .with_json_overrides(overrides)
        .map_err(core_to_py)
}

/// Numerical convergence settings shared by calibration solvers.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import SolverConfig
/// >>> cfg = SolverConfig(tolerance=1e-10, max_iterations=200)
/// >>> cfg.tolerance, cfg.max_iterations
/// (1e-10, 200)
#[pyclass(
    name = "SolverConfig",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object,
    eq
)]
#[derive(Clone, PartialEq)]
pub struct PySolverConfig {
    pub(crate) inner: SolverConfig,
}

impl PySolverConfig {
    pub(crate) fn from_inner(inner: SolverConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PySolverConfig {
    /// Build solver settings, starting from the Rust defaults.
    ///
    /// Parameters
    /// ----------
    /// tolerance : float | None, default None
    ///     Finite, positive root-finder convergence tolerance in absolute
    ///     residual units; ``None`` uses the Rust default.
    /// max_iterations : int | None, default None
    ///     Maximum iterations per knot solve.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``tolerance`` is non-finite or not positive, or
    ///     ``max_iterations`` is zero.
    #[new]
    #[pyo3(signature = (tolerance = None, max_iterations = None))]
    #[pyo3(text_signature = "(tolerance=None, max_iterations=None)")]
    fn new(tolerance: Option<f64>, max_iterations: Option<usize>) -> PyResult<Self> {
        let mut inner = SolverConfig::default();
        if let Some(tolerance) = tolerance {
            inner = inner.with_tolerance(tolerance);
        }
        if let Some(max_iterations) = max_iterations {
            inner = inner.with_max_iterations(max_iterations);
        }
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Convergence tolerance.
    #[getter]
    fn tolerance(&self) -> f64 {
        self.inner.tolerance()
    }

    /// Maximum iterations per solve.
    #[getter]
    fn max_iterations(&self) -> usize {
        self.inner.max_iterations()
    }

    fn __repr__(&self) -> String {
        format!(
            "SolverConfig(tolerance={:e}, max_iterations={})",
            self.inner.tolerance(),
            self.inner.max_iterations()
        )
    }
}

wire_methods!(PySolverConfig, SolverConfig, "SolverConfig", validate);

/// Minimum / maximum admissible zero rates for curve validation.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import RateBounds
/// >>> RateBounds(-0.02, 0.25).max_rate
/// 0.25
/// >>> RateBounds.emerging_markets().max_rate > 1.0
/// True
#[pyclass(
    name = "RateBounds",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object,
    eq
)]
#[derive(Clone, PartialEq)]
pub struct PyRateBounds {
    pub(crate) inner: RateBounds,
}

impl PyRateBounds {
    pub(crate) fn from_inner(inner: RateBounds) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRateBounds {
    /// Explicit rate bounds.
    ///
    /// Parameters
    /// ----------
    /// min_rate : float
    ///     Lowest admissible zero rate as a decimal (negative allowed).
    /// max_rate : float
    ///     Highest admissible zero rate as a decimal; must exceed ``min_rate``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the bounds are not finite or ``min_rate >= max_rate``.
    #[new]
    #[pyo3(text_signature = "(min_rate, max_rate)")]
    fn new(min_rate: f64, max_rate: f64) -> PyResult<Self> {
        RateBounds::new(min_rate, max_rate)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Currency-appropriate default bounds.
    ///
    /// Parameters
    /// ----------
    /// currency : str
    ///     ISO-4217 code.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``currency`` is not a valid ISO-4217 code.
    #[staticmethod]
    #[pyo3(text_signature = "(currency)")]
    fn for_currency(currency: &str) -> PyResult<Self> {
        let currency = crate::bindings::module_utils::parse_currency(currency)?;
        Ok(Self::from_inner(RateBounds::for_currency(currency)))
    }

    /// Wide bounds suitable for emerging-market curves.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn emerging_markets() -> Self {
        Self::from_inner(RateBounds::emerging_markets())
    }

    /// Lowest admissible zero rate (decimal).
    #[getter]
    fn min_rate(&self) -> f64 {
        self.inner.min_rate
    }

    /// Highest admissible zero rate (decimal).
    #[getter]
    fn max_rate(&self) -> f64 {
        self.inner.max_rate
    }

    fn __repr__(&self) -> String {
        format!(
            "RateBounds(min_rate={}, max_rate={})",
            self.inner.min_rate, self.inner.max_rate
        )
    }
}

wire_methods!(PyRateBounds, RateBounds, "RateBounds", validate);

/// Post-solve curve/surface validation thresholds and toggles.
///
/// Fields mirror the Rust ``ValidationConfig`` wire schema exactly
/// (``check_forward_positivity``, ``min_forward_rate``, ``max_forward_rate``,
/// ``check_monotonicity``, ``check_arbitrage``, ``tolerance``,
/// ``max_hazard_rate``, ``min_cpi_growth``, ``max_cpi_growth``,
/// ``min_fwd_inflation``, ``max_fwd_inflation``, ``max_volatility``,
/// ``allow_negative_rates``, ``recovery_rate_abs_tolerance``,
/// ``minimum_lgd_for_hazard_guess``).
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import ValidationConfig
/// >>> cfg = ValidationConfig(check_arbitrage=False)
/// >>> cfg.to_dict()["check_arbitrage"]
/// False
#[pyclass(
    name = "ValidationConfig",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyValidationConfig {
    pub(crate) inner: ValidationConfig,
}

impl PyValidationConfig {
    pub(crate) fn from_inner(inner: ValidationConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyValidationConfig {
    /// Build validation settings from the Rust defaults plus keyword overrides.
    ///
    /// Parameters
    /// ----------
    /// **overrides
    ///     Wire field names with their new values (see the class docstring).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown, has the wrong type, or the merged
    ///     configuration fails validation.
    #[new]
    #[pyo3(signature = (**overrides))]
    #[pyo3(text_signature = "(**overrides)")]
    fn new(py: Python<'_>, overrides: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let value = match overrides {
            Some(overrides) => py_to_json_value(py, overrides.as_any(), "ValidationConfig")?,
            None => serde_json::json!({}),
        };
        let inner: ValidationConfig = serde_json::from_value(value)
            .map_err(|e| serde_json_to_py(e, "invalid ValidationConfig"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// All fields as a dict of wire values.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("ValidationConfig", &self.inner)
    }
}

wire_methods!(
    PyValidationConfig,
    ValidationConfig,
    "ValidationConfig",
    validate
);

#[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig")]
#[pyclass(
    name = "CalibrationConfig",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationConfig {
    pub(crate) inner: CalibrationConfig,
}

impl PyCalibrationConfig {
    pub(crate) fn from_inner(inner: CalibrationConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalibrationConfig {
    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.__init__")]
    #[new]
    #[pyo3(signature = (**overrides))]
    #[pyo3(text_signature = "(**overrides)")]
    fn new(py: Python<'_>, overrides: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let overrides = match overrides {
            Some(dict) => py_to_json_value(py, dict.as_any(), "calibration config overrides")?,
            None => Value::Object(serde_json::Map::new()),
        };
        CalibrationConfig::default()
            .with_json_overrides(overrides)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.solver")]
    #[getter]
    fn solver(&self) -> PySolverConfig {
        PySolverConfig::from_inner(self.inner.solver.clone())
    }

    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.use_parallel")]
    #[getter]
    fn use_parallel(&self) -> bool {
        self.inner.use_parallel
    }

    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.fail_on_bad_fit")]
    #[getter]
    fn fail_on_bad_fit(&self) -> bool {
        self.inner.fail_on_bad_fit
    }

    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.compute_diagnostics")]
    #[getter]
    fn compute_diagnostics(&self) -> bool {
        self.inner.compute_diagnostics
    }

    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.validation_mode")]
    #[getter]
    fn validation_mode(&self) -> String {
        self.inner.validation_mode.as_str().to_string()
    }

    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.rate_bounds")]
    #[getter]
    fn rate_bounds(&self) -> PyRateBounds {
        PyRateBounds::from_inner(self.inner.rate_bounds.clone())
    }

    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.validation")]
    #[getter]
    fn validation(&self) -> PyValidationConfig {
        PyValidationConfig::from_inner(self.inner.validation.clone())
    }

    #[doc = pydoc!("finstack_quant.calibration._config", "CalibrationConfig.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CalibrationConfig", &self.inner)
    }
}

wire_methods!(
    PyCalibrationConfig,
    CalibrationConfig,
    "CalibrationConfig",
    module = "finstack_quant.calibration._config",
    validate
);
