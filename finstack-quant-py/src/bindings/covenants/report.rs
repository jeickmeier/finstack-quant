//! `CovenantReport` Python wrapper.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::display_to_py;
use pyo3::prelude::*;

/// Result of a single covenant evaluation.
///
/// Carries pass/fail status, the tested value against its threshold, the
/// headroom (positive is cushion, negative is deficit), an optional
/// human-readable explanation, and the audit stamp (numeric mode, rounding
/// context, FX policy) in force when the covenant was evaluated.
///
/// Construct via ``CovenantEngine.evaluate``, ``evaluate_engine`` or
/// ``from_json``. Two reports compare equal when every field, including the
/// audit stamp, is equal.
#[pyclass(
    name = "CovenantReport",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantReport {
    pub(crate) inner: finstack_quant_covenants::CovenantReport,
}

#[pymethods]
impl PyCovenantReport {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_covenants::CovenantReport =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Human-readable description of the covenant being tested
    /// (e.g. ``"Debt/EBITDA <= 5.00x"``).
    #[getter]
    fn covenant_type(&self) -> &str {
        &self.inner.covenant_type
    }

    /// Stable machine-readable covenant instance identifier, or ``None`` when
    /// the report was produced without one.
    #[getter]
    fn covenant_id(&self) -> Option<&str> {
        self.inner.covenant_id.as_deref()
    }

    /// ``True`` when the covenant passed.
    ///
    /// Inactive covenants, unmet springing conditions, and full waivers also
    /// report ``True``; :attr:`details` carries the reason.
    #[getter]
    fn passed(&self) -> bool {
        self.inner.passed
    }

    /// Tested metric value, or ``None`` when the covenant was not evaluated
    /// numerically (inactive, waived, or springing condition unmet).
    #[getter]
    fn actual_value(&self) -> Option<f64> {
        self.inner.actual_value
    }

    /// Threshold the metric was tested against, or ``None`` when no numeric
    /// test was applied.
    #[getter]
    fn threshold(&self) -> Option<f64> {
        self.inner.threshold
    }

    /// Explanation of the outcome, or ``None``.
    #[getter]
    fn details(&self) -> Option<&str> {
        self.inner.details.as_deref()
    }

    /// Cushion relative to the threshold: positive is a passing buffer,
    /// negative a deficit. ``None`` when no numeric test was applied.
    #[getter]
    fn headroom(&self) -> Option<f64> {
        self.inner.headroom
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.covenants._report", "CovenantReport.headroom_raw")]
    fn headroom_raw(&self) -> Option<f64> {
        self.inner.headroom_raw
    }

    /// Audit stamp as a dict: numeric mode, rounding context, and FX policy in
    /// force for this evaluation.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    fn __repr__(&self) -> String {
        fn opt(value: Option<f64>) -> String {
            value.map_or("None".to_string(), |v| v.to_string())
        }
        format!(
            "CovenantReport(covenant_id={}, covenant_type={:?}, passed={}, actual_value={}, threshold={}, headroom={})",
            self.inner
                .covenant_id
                .as_deref()
                .map_or("None".to_string(), |id| format!("{id:?}")),
            self.inner.covenant_type,
            if self.inner.passed { "True" } else { "False" },
            opt(self.inner.actual_value),
            opt(self.inner.threshold),
            opt(self.inner.headroom),
        )
    }
}

impl PyCovenantReport {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8("covenant_type", vec![self.inner.covenant_type.clone()])?
            .nullable_utf8("covenant_id", vec![self.inner.covenant_id.clone()])?
            .bool("passed", vec![self.inner.passed])?
            .nullable_f64("actual_value", vec![self.inner.actual_value])?
            .nullable_f64("threshold", vec![self.inner.threshold])?
            .nullable_f64("headroom", vec![self.inner.headroom])?
            .nullable_f64("headroom_raw", vec![self.inner.headroom_raw])?
            .nullable_utf8("details", vec![self.inner.details.clone()])?
            .build()
    }
}

tabular_methods!(PyCovenantReport);
