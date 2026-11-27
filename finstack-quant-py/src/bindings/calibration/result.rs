//! `CalibrationResultEnvelope`: calibrated market plus plan-level and per-step reports.

use super::report::{PyCalibrationReport, quote_quality_table};
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::tabular_methods;
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_calibration::api::schema::CalibrationResultEnvelope;
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::market_data::context::MarketContext;
use pyo3::prelude::*;
use pyo3::types::PyString;
use std::sync::OnceLock;

/// Result of a calibration plan execution.
///
/// Provides the calibrated market context, the plan-level report, per-step
/// reports keyed by step id, and per-quote residual frames.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, calibrate
/// >>> result = calibrate(CalibrationEnvelope(CalibrationPlan([], id="smoke")))
/// >>> result.success
/// True
#[pyclass(
    name = "CalibrationResultEnvelope",
    module = "finstack_quant.calibration",
    skip_from_py_object
)]
pub struct PyCalibrationResult {
    pub(crate) inner: CalibrationResultEnvelope,
    cached_json: OnceLock<String>,
}

impl Clone for PyCalibrationResult {
    fn clone(&self) -> Self {
        Self::from_inner(self.inner.clone())
    }
}

impl PyCalibrationResult {
    pub(crate) fn from_inner(inner: CalibrationResultEnvelope) -> Self {
        Self {
            inner,
            cached_json: OnceLock::new(),
        }
    }
}

fn cached_json<'py, F>(
    py: Python<'py>,
    cache: &OnceLock<String>,
    serialize: F,
) -> PyResult<Bound<'py, PyString>>
where
    F: FnOnce() -> serde_json::Result<String>,
{
    if let Some(value) = cache.get() {
        return Ok(PyString::new(py, value));
    }
    let value = serialize().map_err(|e| serde_json_to_py(e, "failed to serialize"))?;
    let py_value = PyString::new(py, &value);
    let _ = cache.set(value);
    Ok(py_value)
}

#[pymethods]
impl PyCalibrationResult {
    /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// ``to_json`` / ``from_json`` with the canonical persistence limits.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_json(py)?.to_string();
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, payload)
    }

    /// Rebuild a result from JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Result envelope JSON (schema ``finstack_quant.calibration/1``),
    ///     bounded to 64 MiB and 96 nested JSON containers. The loader retains
    ///     at most 256 diagnostic findings.
    ///
    /// Raises
    /// ------
    /// ContractValidationError
    ///     If the JSON, result, or nested final market fails structural
    ///     validation, or its schema marker is missing, malformed, or unsupported. The
    ///     ``report`` attribute lists pointer-level diagnostic messages.
    /// ContractLimitExceededError
    ///     If the JSON exceeds the canonical input-size or nesting-depth limit.
    /// ValueError
    ///     If an invalid nested value cannot be restored.
    #[staticmethod]
    fn from_json(py: Python<'_>, json: &str) -> PyResult<Self> {
        let (inner, _report) =
            CalibrationResultEnvelope::from_slice_strict(json.as_bytes(), &LoadLimits::default())
                .map_err(|e| crate::errors::contract_to_py(py, e))?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to a compact JSON string.
    ///
    /// Returns a cached Python ``str``: the JSON is rendered once and reused on
    /// subsequent calls.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        cached_json(py, &self.cached_json, || serde_json::to_string(&self.inner))
    }

    /// Versioned SHA-256 content hash of the canonical result JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result contains a non-finite number.
    fn content_hash(&self) -> PyResult<String> {
        self.inner.content_hash().map_err(core_to_py)
    }

    /// Whether the overall calibration succeeded (all steps passed fitting and validation).
    #[getter]
    fn success(&self) -> bool {
        self.inner.result.report.success
    }

    /// The calibrated ``MarketContext`` containing all produced curves and surfaces.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the stored market snapshot cannot be rehydrated.
    #[getter]
    fn market(&self) -> PyResult<PyMarketContext> {
        let ctx =
            MarketContext::try_from(self.inner.result.final_market.clone()).map_err(core_to_py)?;
        Ok(PyMarketContext::from_inner(ctx))
    }

    /// Plan-level aggregated report.
    ///
    /// Its ``max_residual_ratio`` / ``rmse_ratio`` are in
    /// ``|residual| / step_tolerance`` units; its ``max_residual`` / ``rmse``
    /// summarize the raw per-step statistics.
    #[getter]
    fn report(&self) -> PyCalibrationReport {
        PyCalibrationReport::from_inner(self.inner.result.report.clone())
    }

    /// List of step identifiers in lexicographic step-ID order.
    #[getter]
    fn step_ids(&self) -> Vec<String> {
        self.inner.result.step_reports.keys().cloned().collect()
    }

    /// Number of solver iterations across all steps.
    #[getter]
    fn iterations(&self) -> usize {
        self.inner.result.report.iterations
    }

    /// Per-step calibration report.
    ///
    /// Parameters
    /// ----------
    /// step_id : str
    ///     Identifier of the calibration step.
    ///
    /// Returns
    /// -------
    /// CalibrationReport
    ///     Typed step report with raw residuals keyed by quote id.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no step with the given ``step_id`` exists.
    #[pyo3(text_signature = "($self, step_id)")]
    fn step_report(&self, step_id: &str) -> PyResult<PyCalibrationReport> {
        let report = self.inner.result.step_report(step_id).map_err(core_to_py)?;
        Ok(PyCalibrationReport::from_inner(report.clone()))
    }

    /// Per-quote residuals of one step as an ``ArrowTable``.
    ///
    /// Columns: ``quote_id``, ``quote_value``, ``residual``,
    /// ``sensitivity``. ``quote_value`` / ``sensitivity`` are ``NaN``
    /// unless ``CalibrationConfig.compute_diagnostics`` was enabled.
    ///
    /// Parameters
    /// ----------
    /// step_id : str
    ///     Identifier of the calibration step.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no step with the given ``step_id`` exists.
    #[pyo3(text_signature = "($self, step_id)")]
    fn residuals(&self, step_id: &str) -> PyResult<PyArrowTable> {
        let report = self.inner.result.step_report(step_id).map_err(core_to_py)?;
        quote_quality_table(&report.quote_rows())
    }

    fn __repr__(&self) -> String {
        let n = self.inner.result.step_reports.len();
        format!(
            "CalibrationResultEnvelope(success={}, steps={n}, iterations={}, max_residual_ratio={:.2e})",
            if self.inner.result.report.success {
                "True"
            } else {
                "False"
            },
            self.inner.result.report.iterations,
            self.inner
                .result
                .report
                .max_residual_ratio
                .unwrap_or(f64::NAN),
        )
    }
}

impl PyCalibrationResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let n = self.inner.result.step_reports.len();
        let mut ids: Vec<String> = Vec::with_capacity(n);
        let mut successes: Vec<bool> = Vec::with_capacity(n);
        let mut iters: Vec<i64> = Vec::with_capacity(n);
        let mut max_res: Vec<f64> = Vec::with_capacity(n);
        let mut rmses: Vec<f64> = Vec::with_capacity(n);
        let mut worst: Vec<Option<String>> = Vec::with_capacity(n);
        let mut reasons: Vec<String> = Vec::with_capacity(n);

        for (id, report) in &self.inner.result.step_reports {
            ids.push(id.clone());
            successes.push(report.success);
            iters.push(i64::try_from(report.iterations).unwrap_or(i64::MAX));
            max_res.push(report.max_residual);
            rmses.push(report.rmse);
            worst.push(report.worst_quote_id.clone());
            reasons.push(report.convergence_reason.clone());
        }

        TableBuilder::new()
            .utf8("step_id", ids)?
            .bool("success", successes)?
            .i64("iterations", iters)?
            .f64("max_residual", max_res)?
            .f64("rmse", rmses)?
            .nullable_utf8("worst_quote_id", worst)?
            .utf8("convergence_reason", reasons)?
            .build()
    }
}

tabular_methods!(PyCalibrationResult);
