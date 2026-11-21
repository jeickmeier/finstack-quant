//! Typed calibration report, diagnostics, quote-quality and validation-report wrappers.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use finstack_quant_calibration::api::validate::CalibrationValidationReport;
use finstack_quant_calibration::{CalibrationDiagnostics, CalibrationReport, QuoteQuality};
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[doc = pydoc!("finstack_quant.calibration._report", "QuoteQuality")]
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
    #[doc = pydoc!("finstack_quant.calibration._report", "QuoteQuality.quote_label")]
    #[getter]
    fn quote_label(&self) -> String {
        self.inner.quote_label.clone()
    }

    #[doc = pydoc!("finstack_quant.calibration._report", "QuoteQuality.quote_value")]
    #[getter]
    fn quote_value(&self) -> Option<f64> {
        self.inner.quote_value
    }

    #[doc = pydoc!("finstack_quant.calibration._report", "QuoteQuality.residual")]
    #[getter]
    fn residual(&self) -> f64 {
        self.inner.residual
    }

    #[doc = pydoc!("finstack_quant.calibration._report", "QuoteQuality.sensitivity")]
    #[getter]
    fn sensitivity(&self) -> f64 {
        self.inner.sensitivity
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("QuoteQuality", &self.inner)
    }
}

wire_methods!(
    PyQuoteQuality,
    QuoteQuality,
    "QuoteQuality",
    module = "finstack_quant.calibration._report"
);

/// Per-quote fit quality and conditioning diagnostics for one calibration step.
///
/// Only populated when ``CalibrationConfig.compute_diagnostics`` is ``True``.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationDiagnostics
/// >>> d = CalibrationDiagnostics.from_json('{"per_quote": [], "condition_number": null, "jacobian": null, "max_residual": 0.0, "rms_residual": 0.0}')
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

    /// Finite-difference Jacobian of the residuals at the solution.
    ///
    /// ``jacobian[i][j]`` is the derivative of the residual of quote ``i``
    /// (``per_quote`` order) with respect to solved parameter ``j`` (knot
    /// order), unweighted. ``None`` for sequential bootstraps and when a
    /// parameter bump could not be priced.
    #[getter]
    fn jacobian(&self) -> Option<Vec<Vec<f64>>> {
        self.inner.jacobian.clone()
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

    fn __repr__(&self) -> String {
        format!(
            "CalibrationDiagnostics(quotes={}, max_residual={:.3e}, rms_residual={:.3e})",
            self.inner.per_quote.len(),
            self.inner.max_residual,
            self.inner.rms_residual
        )
    }
}

impl PyCalibrationDiagnostics {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        quote_quality_table(&self.inner.per_quote)
    }
}

tabular_methods!(PyCalibrationDiagnostics);
wire_methods!(
    PyCalibrationDiagnostics,
    CalibrationDiagnostics,
    "CalibrationDiagnostics"
);

/// Build the per-quote quality table shared by diagnostics and residual exports.
pub(crate) fn quote_quality_table(rows: &[QuoteQuality]) -> PyResult<PyArrowTable> {
    let n = rows.len();
    let mut ids = Vec::with_capacity(n);
    let mut quote_values = Vec::with_capacity(n);
    let mut residuals = Vec::with_capacity(n);
    let mut sensitivities = Vec::with_capacity(n);
    for row in rows {
        ids.push(row.quote_label.clone());
        quote_values.push(row.quote_value.unwrap_or(f64::NAN));
        residuals.push(row.residual);
        sensitivities.push(row.sensitivity);
    }
    TableBuilder::new()
        .utf8("quote_id", ids)?
        .f64("quote_value", quote_values)?
        .f64("residual", residuals)?
        .f64("sensitivity", sensitivities)?
        .build()
}

/// Detailed report of one calibration step or of the whole plan.
///
/// Step reports carry raw residuals keyed by quote id. The plan-level report
/// returned by ``CalibrationResultEnvelope.report`` additionally carries
/// ``max_residual_ratio`` / ``rmse_ratio`` (``|residual| / step_tolerance``)
/// while its ``max_residual`` / ``rmse`` summarize the raw step statistics.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, calibrate
/// >>> report = calibrate(CalibrationEnvelope(CalibrationPlan([], id="smoke"))).report
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

    #[doc = pydoc!("finstack_quant.calibration._report", "CalibrationReport.metadata")]
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

    #[doc = pydoc!("finstack_quant.calibration._report", "CalibrationReport.solver_method")]
    #[getter]
    fn solver_method(&self) -> Option<&'static str> {
        self.inner.solver_method.map(|method| method.as_str())
    }

    #[doc = pydoc!("finstack_quant.calibration._report", "CalibrationReport.residual_units")]
    #[getter]
    fn residual_units(&self) -> Option<&'static str> {
        self.inner.residual_units.map(|units| units.as_str())
    }

    /// Smile parameters fitted per quoted expiry, as a list of dicts.
    ///
    /// Each dict has ``expiry`` (Act/365F years from the base date),
    /// ``parameters`` (``{"sabr": {...}}`` or ``{"svi": {...}}``),
    /// ``iterations`` and, for SABR slices, ``max_relative_quote_error``.
    /// Empty except for equity SABR and SVI surface steps.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the slices cannot be converted to Python objects.
    #[getter]
    fn fitted_slices<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.fitted_slices)
    }

    /// Per-quote diagnostics, when ``compute_diagnostics`` was enabled.
    #[getter]
    fn diagnostics(&self) -> Option<PyCalibrationDiagnostics> {
        self.inner
            .diagnostics
            .clone()
            .map(PyCalibrationDiagnostics::from_inner)
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

impl PyCalibrationReport {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        quote_quality_table(&self.inner.quote_rows())
    }
}

tabular_methods!(PyCalibrationReport);
wire_methods!(PyCalibrationReport, CalibrationReport, "CalibrationReport");

/// Solver-free validation report: every static envelope error plus the step dependency graph.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, dry_run
/// >>> report = dry_run(CalibrationEnvelope(CalibrationPlan([], id="smoke")))
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

    #[doc = pydoc!(
        "finstack_quant.calibration._report",
        "CalibrationValidationReport.dependency_graph"
    )]
    #[getter]
    fn dependency_graph<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.dependency_graph)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationValidationReport(errors={}, steps={})",
            self.inner.errors.len(),
            self.inner.dependency_graph.nodes.len()
        )
    }
}

impl PyCalibrationValidationReport {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut kinds = Vec::with_capacity(self.inner.errors.len());
        let mut steps: Vec<Option<String>> = Vec::with_capacity(self.inner.errors.len());
        let mut messages = Vec::with_capacity(self.inner.errors.len());
        for error in &self.inner.errors {
            kinds.push(error.kind_str().to_string());
            steps.push(error.step_id().map(str::to_string));
            messages.push(error.to_string());
        }
        TableBuilder::new()
            .utf8("kind", kinds)?
            .nullable_utf8("step_id", steps)?
            .utf8("message", messages)?
            .build()
    }
}

tabular_methods!(PyCalibrationValidationReport);
wire_methods!(
    PyCalibrationValidationReport,
    CalibrationValidationReport,
    "CalibrationValidationReport"
);
