//! `CovenantReport` Python wrapper.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::TableBuilder;
use pyo3::prelude::*;

/// Result of a single covenant evaluation.
///
/// Carries pass/fail status, the tested value against its threshold, the
/// headroom (positive is cushion, negative is deficit), an optional
/// human-readable explanation, and the audit stamp (numeric mode, rounding
/// context, FX policy) in force when the covenant was evaluated.
///
/// Construct via ``CovenantEngine.evaluate`` or
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
