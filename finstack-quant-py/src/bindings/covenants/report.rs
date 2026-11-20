//! `CovenantReport` Python wrapper.

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_to_py,
};
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

crate::bindings::macros::wire_methods!(
    PyCovenantReport,
    finstack_quant_covenants::CovenantReport,
    "CovenantReport"
);
crate::bindings::macros::impl_repr_html_via_dataframe!(PyCovenantReport);
crate::bindings::macros::py_getters!(PyCovenantReport |s| {
    /// Human-readable description of the covenant being tested
    /// (e.g. ``"Debt/EBITDA <= 5.00x"``).
    covenant_type -> &str = &s.inner.covenant_type;

    /// Stable machine-readable covenant instance identifier, or ``None`` when
    /// the report was produced without one.
    covenant_id -> Option<&str> = s.inner.covenant_id.as_deref();

    /// ``True`` when the covenant passed.
    ///
    /// Inactive covenants, unmet springing conditions, and full waivers also
    /// report ``True``; :attr:`details` carries the reason.
    passed -> bool = s.inner.passed;

    /// Tested metric value, or ``None`` when the covenant was not evaluated
    /// numerically (inactive, waived, or springing condition unmet).
    actual_value -> Option<f64> = s.inner.actual_value;

    /// Threshold the metric was tested against, or ``None`` when no numeric
    /// test was applied.
    threshold -> Option<f64> = s.inner.threshold;

    /// Explanation of the outcome, or ``None``.
    details -> Option<&str> = s.inner.details.as_deref();

    /// Cushion relative to the threshold: positive is a passing buffer,
    /// negative a deficit. ``None`` when no numeric test was applied.
    headroom -> Option<f64> = s.inner.headroom;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.covenants._report", "CovenantReport.headroom_raw")]
    headroom_raw -> Option<f64> = s.inner.headroom_raw;
});

#[pymethods]
impl PyCovenantReport {
    /// Audit stamp as a dict: numeric mode, rounding context, and FX policy in
    /// force for this evaluation.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Export the report as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``covenant_type``, ``covenant_id``, ``passed``,
    /// ``actual_value``, ``threshold``, ``headroom``, ``headroom_raw``, ``details``.
    ///
    /// The optional columns are ``None`` for a covenant that was not evaluated
    /// numerically (inactive, waived, springing condition unmet); pandas then
    /// infers dtype ``object`` for that column, so coerce with
    /// ``pd.to_numeric`` before concatenating mixed reports.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "covenant_type": self.inner.covenant_type,
            "covenant_id": self.inner.covenant_id,
            "passed": self.inner.passed,
            "actual_value": self.inner.actual_value,
            "threshold": self.inner.threshold,
            "headroom": self.inner.headroom,
            "headroom_raw": self.inner.headroom_raw,
            "details": self.inner.details,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "covenant_type",
                "covenant_id",
                "passed",
                "actual_value",
                "threshold",
                "headroom",
                "headroom_raw",
                "details",
            ],
        )
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
