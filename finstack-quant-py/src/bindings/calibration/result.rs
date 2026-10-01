//! `CalibrationResult`: calibrated market plus plan-level and per-step reports.

use super::report::{quote_quality_dataframe, residual_rows, PyCalibrationReport};
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::bindings::pickle_support::reduce_via_json;
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use finstack_quant_calibration::api::schema::CalibrationResultEnvelope;
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::market_data::context::MarketContext;
use numpy::PyArray1;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};
use std::sync::OnceLock;

/// Result of a calibration plan execution.
///
/// Provides the calibrated market context, the plan-level report, per-step
/// reports keyed by step id, and per-quote residual frames.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationPlan, calibrate
/// >>> result = calibrate(CalibrationPlan([], id="smoke"))
/// >>> result.success
/// True
#[pyclass(
    name = "CalibrationResult",
    module = "finstack_quant.calibration",
    skip_from_py_object
)]
pub struct PyCalibrationResult {
    pub(crate) inner: CalibrationResultEnvelope,
    cached_json: OnceLock<String>,
    cached_market_json: OnceLock<String>,
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
            cached_market_json: OnceLock::new(),
        }
    }

    fn find_step_report(
        &self,
        step_id: &str,
    ) -> PyResult<&finstack_quant_calibration::CalibrationReport> {
        self.inner.result.step_reports.get(step_id).ok_or_else(|| {
            PyKeyError::new_err(format!(
                "no calibration step '{step_id}'; available steps: {:?}",
                self.inner
                    .result
                    .step_reports
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
            ))
        })
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
        let ctx = MarketContext::try_from(self.inner.result.final_market.clone())
            .map_err(display_to_py)?;
        Ok(PyMarketContext::from_inner(ctx))
    }

    /// The calibrated market serialized as a JSON string.
    ///
    /// Validates the state through the same ``MarketContext`` conversion as
    /// the ``market`` getter before serializing.
    #[getter]
    fn market_json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        if let Some(value) = self.cached_market_json.get() {
            return Ok(PyString::new(py, value));
        }
        MarketContext::try_from(self.inner.result.final_market.clone()).map_err(display_to_py)?;
        cached_json(py, &self.cached_market_json, || {
            serde_json::to_string(&self.inner.result.final_market)
        })
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

    /// The plan-level report as a compact JSON string.
    #[getter]
    fn report_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.result.report)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationReport"))
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

    /// Maximum ``|residual| / step_tolerance`` across every quote of every step.
    ///
    /// A value below 1.0 means every quote repriced within its step's
    /// tolerance. Raw per-step residual statistics live on
    /// ``step_report(step_id).max_residual`` / ``.rmse``.
    #[getter]
    fn max_residual_ratio(&self) -> f64 {
        self.inner
            .result
            .report
            .max_residual_ratio
            .unwrap_or(f64::NAN)
    }

    /// Root-mean-square ``|residual| / step_tolerance`` across every quote of every step.
    #[getter]
    fn rmse_ratio(&self) -> f64 {
        self.inner.result.report.rmse_ratio.unwrap_or(f64::NAN)
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
        Ok(PyCalibrationReport::from_inner(
            self.find_step_report(step_id)?.clone(),
        ))
    }

    /// Per-step calibration report as a compact JSON string.
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
    /// ValueError
    ///     If serialization fails.
    #[pyo3(text_signature = "($self, step_id)")]
    fn step_report_json(&self, step_id: &str) -> PyResult<String> {
        serde_json::to_string(self.find_step_report(step_id)?)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationReport"))
    }

    /// Per-quote residuals of one step as a pandas ``DataFrame``.
    ///
    /// Columns: ``quote_id``, ``target``, ``fitted``, ``residual``,
    /// ``sensitivity``. ``target`` / ``fitted`` / ``sensitivity`` are ``NaN``
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
    fn residuals<'py>(&self, py: Python<'py>, step_id: &str) -> PyResult<Bound<'py, PyAny>> {
        quote_quality_dataframe(py, &residual_rows(self.find_step_report(step_id)?))
    }

    /// Export the per-step summary as a pandas ``DataFrame``.
    ///
    /// Columns: ``step_id``, ``success``, ``iterations``, ``max_residual``,
    /// ``rmse`` (raw residual units of each step), ``worst_quote_id``,
    /// ``convergence_reason``. Rows are ordered lexicographically by step ID.
    /// The plan-level roll-ups (``success``, ``iterations``,
    /// ``max_residual_ratio``, ``rmse_ratio``) are getters on the result.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let n = self.inner.result.step_reports.len();
        let mut ids: Vec<String> = Vec::with_capacity(n);
        let mut successes: Vec<bool> = Vec::with_capacity(n);
        let mut iters: Vec<usize> = Vec::with_capacity(n);
        let mut max_res: Vec<f64> = Vec::with_capacity(n);
        let mut rmses: Vec<f64> = Vec::with_capacity(n);
        let mut worst: Vec<Option<String>> = Vec::with_capacity(n);
        let mut reasons: Vec<String> = Vec::with_capacity(n);

        for (id, report) in &self.inner.result.step_reports {
            ids.push(id.clone());
            successes.push(report.success);
            iters.push(report.iterations);
            max_res.push(report.max_residual);
            rmses.push(report.rmse);
            worst.push(report.worst_quote_id.clone());
            reasons.push(report.convergence_reason.clone());
        }

        let data = PyDict::new(py);
        data.set_item("step_id", ids)?;
        data.set_item("success", successes)?;
        data.set_item("iterations", iters)?;
        data.set_item("max_residual", PyArray1::from_vec(py, max_res).into_any())?;
        data.set_item("rmse", PyArray1::from_vec(py, rmses).into_any())?;
        data.set_item("worst_quote_id", worst)?;
        data.set_item("convergence_reason", reasons)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        let n = self.inner.result.step_reports.len();
        format!(
            "CalibrationResult(success={}, steps={n}, iterations={}, max_residual_ratio={:.2e})",
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
