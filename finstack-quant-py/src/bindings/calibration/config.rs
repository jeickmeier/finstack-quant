//! Typed calibration settings: `CalibrationConfig`, `SolverConfig`, `ValidationConfig`, `RateBounds`.

use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_calibration::{CalibrationConfig, RateBounds, SolverConfig, ValidationConfig};
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

/// Overlay ``**overrides`` (top-level wire fields) onto a serde value.
fn overlay_kwargs<T>(
    py: Python<'_>,
    base: &T,
    overrides: Option<&Bound<'_, PyDict>>,
    label: &str,
) -> PyResult<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let mut value = serde_json::to_value(base)
        .map_err(|e| serde_json_to_py(e, &format!("failed to serialize {label}")))?;
    if let Some(overrides) = overrides {
        let Value::Object(map) = &mut value else {
            return Err(value_error(format!("{label} is not a JSON object")));
        };
        for (key, item) in overrides.iter() {
            let key: String = key.extract()?;
            let item = if item.is_none() {
                Value::Null
            } else {
                py_to_json_value(py, &item, &format!("{label} field '{key}'"))?
            };
            map.insert(key, item);
        }
    }
    serde_json::from_value(value).map_err(|e| serde_json_to_py(e, &format!("invalid {label}")))
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
    ///     Root-finder convergence tolerance (absolute, residual units).
    /// max_iterations : int | None, default None
    ///     Maximum iterations per knot solve.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``tolerance`` is not positive or ``max_iterations`` is zero.
    #[new]
    #[pyo3(signature = (tolerance = None, max_iterations = None))]
    #[pyo3(text_signature = "(tolerance=None, max_iterations=None)")]
    fn new(tolerance: Option<f64>, max_iterations: Option<usize>) -> PyResult<Self> {
        let mut inner = SolverConfig::default();
        if let Some(tolerance) = tolerance {
            if !tolerance.is_finite() || tolerance <= 0.0 {
                return Err(value_error(format!(
                    "solver tolerance must be a positive finite number, got {tolerance}"
                )));
            }
            inner = inner.with_tolerance(tolerance);
        }
        if let Some(max_iterations) = max_iterations {
            if max_iterations == 0 {
                return Err(value_error("solver max_iterations must be at least 1"));
            }
            inner = inner.with_max_iterations(max_iterations);
        }
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

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize SolverConfig"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or contains unknown solver fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|e| serde_json_to_py(e, "invalid SolverConfig JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "SolverConfig(tolerance={:e}, max_iterations={})",
            self.inner.tolerance(),
            self.inner.max_iterations()
        )
    }
}

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

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize RateBounds"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or the bounds are invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: RateBounds = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid RateBounds JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "RateBounds(min_rate={}, max_rate={})",
            self.inner.min_rate, self.inner.max_rate
        )
    }
}

/// Post-solve curve/surface validation thresholds and toggles.
///
/// Fields mirror the Rust ``ValidationConfig`` wire schema exactly
/// (``check_forward_positivity``, ``min_forward_rate``, ``max_forward_rate``,
/// ``check_monotonicity``, ``check_arbitrage``, ``tolerance``,
/// ``max_hazard_rate``, ``min_cpi_growth``, ``max_cpi_growth``,
/// ``min_fwd_inflation``, ``max_fwd_inflation``, ``max_volatility``,
/// ``allow_negative_rates``, ``lenient_arbitrage``, ``butterfly_upper_ratio``,
/// ``butterfly_lower_ratio``, ``recovery_rate_abs_tolerance``,
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
        let inner: ValidationConfig = overlay_kwargs(
            py,
            &ValidationConfig::default(),
            overrides,
            "ValidationConfig",
        )?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// All fields as a dict of wire values.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize ValidationConfig"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: ValidationConfig = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid ValidationConfig JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("ValidationConfig", &self.inner)
    }
}

/// Plan-level calibration settings (solver, validation, parallelism, diagnostics).
///
/// Every field defaults to the Rust default; nested wire fields not exposed
/// as keyword arguments (``discount_curve``, ``hazard_curve``, ``fx``,
/// ``market_freshness``, ...) can be overridden through ``**overrides``
/// with partial dicts that merge into the defaults.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationConfig
/// >>> cfg = CalibrationConfig(tolerance=1e-10, compute_diagnostics=True, use_parallel=False)
/// >>> cfg.tolerance, cfg.compute_diagnostics, cfg.use_parallel
/// (1e-10, True, False)
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
    /// Build settings from the Rust defaults plus keyword overrides.
    ///
    /// Parameters
    /// ----------
    /// tolerance : float | None, default None
    ///     Solver convergence tolerance (shorthand for ``solver.tolerance``).
    /// max_iterations : int | None, default None
    ///     Solver iteration cap (shorthand for ``solver.max_iterations``).
    /// solver : SolverConfig | None, default None
    ///     Full solver settings (applied before ``tolerance`` / ``max_iterations``).
    /// use_parallel : bool | None, default None
    ///     Run independent steps in parallel batches.
    /// fail_on_bad_fit : bool | None, default None
    ///     Raise instead of returning ``success=False`` when a step misses tolerance.
    /// compute_diagnostics : bool | None, default None
    ///     Populate per-quote diagnostics (target / fitted / sensitivity).
    /// validation_mode : str | None, default None
    ///     ``"warn"`` or ``"error"`` for post-solve validation findings.
    /// rate_bounds : RateBounds | None, default None
    ///     Explicit admissible-rate bounds (sets ``rate_bounds_policy`` to
    ///     ``"explicit"``).
    /// validation : ValidationConfig | None, default None
    ///     Post-solve validation thresholds.
    /// **overrides
    ///     Any other wire field of ``CalibrationConfig`` (partial nested dicts
    ///     merge into the defaults).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or the merged configuration fails validation.
    #[new]
    #[pyo3(signature = (*, tolerance = None, max_iterations = None, solver = None, use_parallel = None, fail_on_bad_fit = None, compute_diagnostics = None, validation_mode = None, rate_bounds = None, validation = None, **overrides))]
    #[pyo3(
        text_signature = "(*, tolerance=None, max_iterations=None, solver=None, use_parallel=None, fail_on_bad_fit=None, compute_diagnostics=None, validation_mode=None, rate_bounds=None, validation=None, **overrides)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        tolerance: Option<f64>,
        max_iterations: Option<usize>,
        solver: Option<PyRef<'_, PySolverConfig>>,
        use_parallel: Option<bool>,
        fail_on_bad_fit: Option<bool>,
        compute_diagnostics: Option<bool>,
        validation_mode: Option<&str>,
        rate_bounds: Option<PyRef<'_, PyRateBounds>>,
        validation: Option<PyRef<'_, PyValidationConfig>>,
        overrides: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut inner = CalibrationConfig::default();
        if let Some(solver) = solver {
            inner.solver = solver.inner.clone();
        }
        if let Some(tolerance) = tolerance {
            inner = inner.with_tolerance(tolerance);
        }
        if let Some(max_iterations) = max_iterations {
            inner = inner.with_max_iterations(max_iterations);
        }
        if let Some(flag) = use_parallel {
            inner.use_parallel = flag;
        }
        if let Some(flag) = fail_on_bad_fit {
            inner.fail_on_bad_fit = flag;
        }
        if let Some(flag) = compute_diagnostics {
            inner = inner.with_compute_diagnostics(flag);
        }
        if let Some(mode) = validation_mode {
            inner.validation_mode = serde_json::from_value(Value::String(mode.to_string()))
                .map_err(|e| serde_json_to_py(e, "invalid validation_mode"))?;
        }
        if let Some(bounds) = rate_bounds {
            inner = inner.with_rate_bounds(bounds.inner.clone());
        }
        if let Some(validation) = validation {
            inner.validation = validation.inner.clone();
        }
        let overrides = match overrides {
            Some(dict) if !dict.is_empty() => {
                py_to_json_value(py, dict.as_any(), "calibration config overrides")?
            }
            _ => Value::Object(serde_json::Map::new()),
        };
        inner
            .with_json_overrides(overrides)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Solver convergence tolerance.
    #[getter]
    fn tolerance(&self) -> f64 {
        self.inner.solver.tolerance()
    }

    /// Solver iteration cap.
    #[getter]
    fn max_iterations(&self) -> usize {
        self.inner.solver.max_iterations()
    }

    /// Solver settings.
    #[getter]
    fn solver(&self) -> PySolverConfig {
        PySolverConfig::from_inner(self.inner.solver.clone())
    }

    /// Whether independent steps run in parallel batches.
    #[getter]
    fn use_parallel(&self) -> bool {
        self.inner.use_parallel
    }

    /// Whether a missed tolerance raises instead of reporting ``success=False``.
    #[getter]
    fn fail_on_bad_fit(&self) -> bool {
        self.inner.fail_on_bad_fit
    }

    /// Whether per-quote diagnostics are computed.
    #[getter]
    fn compute_diagnostics(&self) -> bool {
        self.inner.compute_diagnostics
    }

    /// Post-solve validation mode (``"warn"`` or ``"error"``).
    #[getter]
    fn validation_mode(&self) -> PyResult<String> {
        let value = serde_json::to_value(&self.inner.validation_mode)
            .map_err(|e| serde_json_to_py(e, "failed to serialize validation_mode"))?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    /// Admissible-rate bounds.
    #[getter]
    fn rate_bounds(&self) -> PyRateBounds {
        PyRateBounds::from_inner(self.inner.rate_bounds.clone())
    }

    /// Post-solve validation thresholds.
    #[getter]
    fn validation(&self) -> PyValidationConfig {
        PyValidationConfig::from_inner(self.inner.validation.clone())
    }

    /// All fields as a nested dict of wire values.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationConfig"))
    }

    /// Rebuild from JSON produced by ``to_json`` (missing fields take defaults).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CalibrationConfig = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CalibrationConfig JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationConfig(tolerance={:e}, max_iterations={}, use_parallel={}, fail_on_bad_fit={}, compute_diagnostics={})",
            self.inner.solver.tolerance(),
            self.inner.solver.max_iterations(),
            if self.inner.use_parallel { "True" } else { "False" },
            if self.inner.fail_on_bad_fit { "True" } else { "False" },
            if self.inner.compute_diagnostics { "True" } else { "False" },
        )
    }
}
