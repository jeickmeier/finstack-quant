//! Python wrappers for the financial statement checks framework.

use crate::errors::display_to_py;
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// CheckSuiteSpec
// ---------------------------------------------------------------------------

/// A serializable suite specification describing which checks to run.
#[pyclass(
    name = "CheckSuiteSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCheckSuiteSpec {
    pub(super) inner: finstack_quant_statements::checks::CheckSuiteSpec,
}

#[pymethods]
impl PyCheckSuiteSpec {
    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_statements::checks::CheckSuiteSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    /// Suite name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Number of built-in checks in the suite spec.
    #[getter]
    fn builtin_check_count(&self) -> usize {
        self.inner.builtin_checks.len()
    }

    /// Number of formula checks in the suite spec.
    #[getter]
    fn formula_check_count(&self) -> usize {
        self.inner.formula_checks.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "CheckSuiteSpec(name={:?}, builtins={}, formulas={})",
            self.inner.name,
            self.inner.builtin_checks.len(),
            self.inner.formula_checks.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// CheckReport
// ---------------------------------------------------------------------------

/// Validation check report aggregating results and summary statistics.
#[pyclass(
    name = "CheckReport",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCheckReport {
    pub(super) inner: finstack_quant_statements::checks::CheckReport,
}

#[pymethods]
impl PyCheckReport {
    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_statements::checks::CheckReport =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    /// Whether all checks passed (no error-severity findings).
    #[getter]
    fn passed(&self) -> bool {
        self.inner.summary.errors == 0
    }

    /// Number of individual check results in the report.
    #[getter]
    fn total_checks(&self) -> usize {
        self.inner.results.len()
    }

    /// Total number of findings across all checks.
    #[getter]
    fn total_findings(&self) -> usize {
        self.inner.summary.errors + self.inner.summary.warnings + self.inner.summary.infos
    }

    /// Number of error-severity findings.
    #[getter]
    fn total_errors(&self) -> usize {
        self.inner.summary.errors
    }

    /// Number of warning-severity findings.
    #[getter]
    fn total_warnings(&self) -> usize {
        self.inner.summary.warnings
    }

    fn __repr__(&self) -> String {
        format!(
            "CheckReport(checks={}, passed={}, errors={}, warnings={})",
            self.inner.results.len(),
            self.inner.summary.errors == 0,
            self.inner.summary.errors,
            self.inner.summary.warnings,
        )
    }
}

/// Register check types on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCheckSuiteSpec>()?;
    m.add_class::<PyCheckReport>()?;
    Ok(())
}
