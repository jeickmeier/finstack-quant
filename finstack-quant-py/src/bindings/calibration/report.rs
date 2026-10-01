//! Typed calibration report, diagnostics, quote-quality and validation-report wrappers.

use crate::bindings::pandas_utils::{dict_to_dataframe, serde_to_py};
use crate::bindings::pickle_support::reduce_via_json;
use crate::errors::serde_json_to_py;
use finstack_quant_calibration::api::validate::CalibrationValidationReport;
use finstack_quant_calibration::{CalibrationDiagnostics, CalibrationReport, QuoteQuality};
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Fitted-versus-target quality record for one calibration quote.
///
/// Attributes
/// ----------
/// quote_label : str
///     Quote identifier as supplied in the envelope (``QuoteId``).
/// target_value : float
///     Market target the solver tried to reprice.
/// fitted_value : float
///     Value implied by the calibrated object.
/// residual : float
///     ``fitted_value - target_value`` in the calibrator's residual units.
/// sensitivity : float
///     Absolute local sensitivity of the residual to the solved knot.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import QuoteQuality
/// >>> q = QuoteQuality.from_json('{"quote_label": "USD-OIS-SWAP-5Y", "target_value": 0.045, "fitted_value": 0.045, "residual": 0.0, "sensitivity": 1.0}')
/// >>> q.quote_label
/// 'USD-OIS-SWAP-5Y'
#[pyclass(
    name = "QuoteQuality",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyQuoteQuality {
    pub(crate) inner: QuoteQuality,
}

impl PyQuoteQuality {
    pub(crate) fn from_inner(inner: QuoteQuality) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyQuoteQuality {
    /// Quote identifier as supplied in the envelope.
    #[getter]
    fn quote_label(&self) -> String {
        self.inner.quote_label.clone()
    }

    /// Market target the solver tried to reprice.
    #[getter]
    fn target_value(&self) -> f64 {
        self.inner.target_value
    }

    /// Value implied by the calibrated object.
    #[getter]
    fn fitted_value(&self) -> f64 {
        self.inner.fitted_value
    }

    /// Signed residual (fitted minus target).
    #[getter]
    fn residual(&self) -> f64 {
        self.inner.residual
    }

    /// Absolute local sensitivity of the residual to the solved parameter.
    #[getter]
    fn sensitivity(&self) -> f64 {
        self.inner.sensitivity
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize QuoteQuality"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|e| serde_json_to_py(e, "invalid QuoteQuality JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("QuoteQuality", &self.inner)
    }
}

/// Per-quote fit quality and conditioning diagnostics for one calibration step.
///
/// Only populated when ``CalibrationConfig.compute_diagnostics`` is ``True``.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationDiagnostics
/// >>> d = CalibrationDiagnostics.from_json('{"per_quote": [], "condition_number": null, "singular_values": null, "max_residual": 0.0, "rms_residual": 0.0, "r_squared": null}')
/// >>> d.max_residual
/// 0.0
#[pyclass(
    name = "CalibrationDiagnostics",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationDiagnostics {
    pub(crate) inner: CalibrationDiagnostics,
}

impl PyCalibrationDiagnostics {
    pub(crate) fn from_inner(inner: CalibrationDiagnostics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalibrationDiagnostics {
    /// Per-quote quality records in solve order.
    #[getter]
    fn per_quote(&self) -> Vec<PyQuoteQuality> {
        self.inner
            .per_quote
            .iter()
            .cloned()
            .map(PyQuoteQuality::from_inner)
            .collect()
    }

    /// Jacobian condition number, when a global solve produced one.
    #[getter]
    fn condition_number(&self) -> Option<f64> {
        self.inner.condition_number
    }

    /// Jacobian singular values (descending), when available.
    #[getter]
    fn singular_values(&self) -> Option<Vec<f64>> {
        self.inner.singular_values.clone()
    }

    /// Maximum absolute per-quote residual.
    #[getter]
    fn max_residual(&self) -> f64 {
        self.inner.max_residual
    }

    /// Root-mean-square per-quote residual.
    #[getter]
    fn rms_residual(&self) -> f64 {
        self.inner.rms_residual
    }

    /// Coefficient of determination of the fit, when available.
    #[getter]
    fn r_squared(&self) -> Option<f64> {
        self.inner.r_squared
    }

    /// Per-quote diagnostics as a pandas ``DataFrame``.
    ///
    /// Columns: ``quote_id``, ``target``, ``fitted``, ``residual``,
    /// ``sensitivity`` (one row per quote, solve order).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If pandas cannot build the frame.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        quote_quality_dataframe(py, &self.inner.per_quote)
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationDiagnostics"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|e| serde_json_to_py(e, "invalid CalibrationDiagnostics JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationDiagnostics(quotes={}, max_residual={:.3e}, rms_residual={:.3e})",
            self.inner.per_quote.len(),
            self.inner.max_residual,
            self.inner.rms_residual
        )
    }
}

/// Build the per-quote quality frame shared by diagnostics and residual exports.
pub(crate) fn quote_quality_dataframe<'py>(
    py: Python<'py>,
    rows: &[QuoteQuality],
) -> PyResult<Bound<'py, PyAny>> {
    let n = rows.len();
    let mut ids = Vec::with_capacity(n);
    let mut targets = Vec::with_capacity(n);
    let mut fitted = Vec::with_capacity(n);
    let mut residuals = Vec::with_capacity(n);
    let mut sensitivities = Vec::with_capacity(n);
    for row in rows {
        ids.push(row.quote_label.clone());
        targets.push(row.target_value);
        fitted.push(row.fitted_value);
        residuals.push(row.residual);
        sensitivities.push(row.sensitivity);
    }
    let data = PyDict::new(py);
    data.set_item("quote_id", ids)?;
    data.set_item("target", PyArray1::from_vec(py, targets).into_any())?;
    data.set_item("fitted", PyArray1::from_vec(py, fitted).into_any())?;
    data.set_item("residual", PyArray1::from_vec(py, residuals).into_any())?;
    data.set_item(
        "sensitivity",
        PyArray1::from_vec(py, sensitivities).into_any(),
    )?;
    dict_to_dataframe(py, &data, None)
}

/// Detailed report of one calibration step or of the whole plan.
///
/// Step reports carry raw residuals keyed by quote id. The plan-level report
/// returned by ``CalibrationResult.report`` additionally carries
/// ``max_residual_ratio`` / ``rmse_ratio`` (``|residual| / step_tolerance``)
/// while its ``max_residual`` / ``rmse`` summarize the raw step statistics.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationPlan, calibrate
/// >>> report = calibrate(CalibrationPlan([], id="smoke")).report
/// >>> report.success, report.iterations
/// (True, 0)
#[pyclass(
    name = "CalibrationReport",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationReport {
    pub(crate) inner: CalibrationReport,
}

impl PyCalibrationReport {
    pub(crate) fn from_inner(inner: CalibrationReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalibrationReport {
    /// ``True`` only if both fitting and validation passed.
    #[getter]
    fn success(&self) -> bool {
        self.inner.success
    }

    /// Final residual per quote id (raw units for step reports, ratio units
    /// for the plan report).
    #[getter]
    fn residuals<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.residuals {
            dict.set_item(key, *value)?;
        }
        Ok(dict)
    }

    /// Solver iterations or function evaluations.
    #[getter]
    fn iterations(&self) -> usize {
        self.inner.iterations
    }

    /// Final objective value (usually the RMSE).
    #[getter]
    fn objective_value(&self) -> f64 {
        self.inner.objective_value
    }

    /// Maximum absolute residual in raw residual units.
    #[getter]
    fn max_residual(&self) -> f64 {
        self.inner.max_residual
    }

    /// Root-mean-square residual in raw residual units.
    #[getter]
    fn rmse(&self) -> f64 {
        self.inner.rmse
    }

    /// Plan-level maximum ``|residual| / step_tolerance``; ``None`` on step reports.
    #[getter]
    fn max_residual_ratio(&self) -> Option<f64> {
        self.inner.max_residual_ratio
    }

    /// Plan-level RMS of ``|residual| / step_tolerance``; ``None`` on step reports.
    #[getter]
    fn rmse_ratio(&self) -> Option<f64> {
        self.inner.rmse_ratio
    }

    /// Whether the calibrated object passed post-solve validation.
    #[getter]
    fn validation_passed(&self) -> bool {
        self.inner.validation_passed
    }

    /// Validation failure detail, when validation failed.
    #[getter]
    fn validation_error(&self) -> Option<String> {
        self.inner.validation_error.clone()
    }

    /// Human-readable convergence or failure reason.
    #[getter]
    fn convergence_reason(&self) -> String {
        self.inner.convergence_reason.clone()
    }

    /// Domain metadata (``type``, ``method``, ``residual_units``, ...).
    #[getter]
    fn metadata<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.metadata {
            dict.set_item(key, value)?;
        }
        Ok(dict)
    }

    /// Solver convergence tolerance used for this step.
    #[getter]
    fn solver_tolerance(&self) -> f64 {
        self.inner.solver_config.tolerance()
    }

    /// Solver iteration cap used for this step.
    #[getter]
    fn solver_max_iterations(&self) -> usize {
        self.inner.solver_config.max_iterations()
    }

    /// Model/methodology version stamp, when set.
    #[getter]
    fn model_version(&self) -> Option<String> {
        self.inner.model_version.clone()
    }

    /// Identifier of the worst-fitting quote, when residuals exist.
    #[getter]
    fn worst_quote_id(&self) -> Option<String> {
        self.inner.worst_quote_id.clone()
    }

    /// Signed residual of ``worst_quote_id``.
    #[getter]
    fn worst_quote_residual(&self) -> Option<f64> {
        self.inner.worst_quote_residual
    }

    /// Success-gate tolerance applied to the residuals, when set.
    #[getter]
    fn success_tolerance(&self) -> Option<f64> {
        self.inner.success_tolerance
    }

    /// Per-quote diagnostics, when ``compute_diagnostics`` was enabled.
    #[getter]
    fn diagnostics(&self) -> Option<PyCalibrationDiagnostics> {
        self.inner
            .diagnostics
            .clone()
            .map(PyCalibrationDiagnostics::from_inner)
    }

    /// Residuals per quote as a pandas ``DataFrame``.
    ///
    /// Columns: ``quote_id``, ``target``, ``fitted``, ``residual``,
    /// ``sensitivity``. ``target`` / ``fitted`` / ``sensitivity`` come from
    /// ``diagnostics`` and are ``NaN`` when diagnostics were not computed.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If pandas cannot build the frame.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        quote_quality_dataframe(py, &self.inner.quote_rows())
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationReport"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|e| serde_json_to_py(e, "invalid CalibrationReport JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationReport(success={}, quotes={}, iterations={}, max_residual={:.3e}, rmse={:.3e})",
            if self.inner.success { "True" } else { "False" },
            self.inner.residuals.len(),
            self.inner.iterations,
            self.inner.max_residual,
            self.inner.rmse
        )
    }
}

/// Solver-free validation report: every static envelope error plus the step dependency graph.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationPlan, dry_run
/// >>> report = dry_run(CalibrationPlan([], id="smoke"))
/// >>> report.is_valid, report.errors
/// (True, [])
#[pyclass(
    name = "CalibrationValidationReport",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationValidationReport {
    pub(crate) inner: CalibrationValidationReport,
}

impl PyCalibrationValidationReport {
    pub(crate) fn from_inner(inner: CalibrationValidationReport) -> Self {
        Self { inner }
    }

    pub(crate) fn to_json_pretty(&self, py: Python<'_>) -> PyResult<String> {
        self.inner
            .to_json_pretty()
            .map_err(|e| super::envelope_error_to_py(py, e))
    }
}

#[pymethods]
impl PyCalibrationValidationReport {
    /// ``True`` when no static error was found.
    #[getter]
    fn is_valid(&self) -> bool {
        self.inner.errors.is_empty()
    }

    /// Static errors as dicts (``kind`` plus the variant's fields, e.g.
    /// ``step_id``, ``ref_name``, ``suggestion``, ``missing_id``).
    #[getter]
    fn errors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.errors)
    }

    /// Dependency graph as a dict with ``initial_ids`` and ``nodes``
    /// (``step_index``, ``step_id``, ``kind``, ``reads``, ``writes``).
    #[getter]
    fn dependency_graph<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.dependency_graph)
    }

    /// One row per static error as a pandas ``DataFrame``.
    ///
    /// Columns: ``kind``, ``step_id``, ``message``; an empty frame carries
    /// the same columns.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If pandas cannot build the frame.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut kinds = Vec::with_capacity(self.inner.errors.len());
        let mut steps: Vec<Option<String>> = Vec::with_capacity(self.inner.errors.len());
        let mut messages = Vec::with_capacity(self.inner.errors.len());
        for error in &self.inner.errors {
            kinds.push(error.kind_str().to_string());
            steps.push(error.step_id().map(str::to_string));
            messages.push(error.to_string());
        }
        let data = PyDict::new(py);
        data.set_item("kind", kinds)?;
        data.set_item("step_id", steps)?;
        data.set_item("message", messages)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationValidationReport"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|e| serde_json_to_py(e, "invalid CalibrationValidationReport JSON"))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationValidationReport(errors={}, steps={})",
            self.inner.errors.len(),
            self.inner.dependency_graph.nodes.len()
        )
    }
}
