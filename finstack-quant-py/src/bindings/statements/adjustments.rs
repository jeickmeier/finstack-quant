//! Python wrappers for EBITDA normalization and adjustments.

use super::evaluator::PyStatementResult;
use crate::errors::display_to_py;
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// NormalizationConfig — JSON wrapper
// ---------------------------------------------------------------------------

/// Configuration for normalizing a financial metric.
#[pyclass(
    name = "NormalizationConfig",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyNormalizationConfig {
    pub(super) inner: finstack_quant_statements::adjustments::types::NormalizationConfig,
}

#[pymethods]
impl PyNormalizationConfig {
    /// Create a new normalization configuration for a target node.
    #[new]
    #[pyo3(text_signature = "(target_node)")]
    fn new(target_node: &str) -> Self {
        Self {
            inner: finstack_quant_statements::adjustments::types::NormalizationConfig::new(
                target_node,
            ),
        }
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_statements::adjustments::types::NormalizationConfig =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    /// Target node being normalized.
    #[getter]
    fn target_node(&self) -> &str {
        &self.inner.target_node
    }

    /// Number of adjustments configured.
    #[getter]
    fn adjustment_count(&self) -> usize {
        self.inner.adjustments.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "NormalizationConfig(target={:?}, adjustments={})",
            self.inner.target_node,
            self.inner.adjustments.len()
        )
    }
}

// ---------------------------------------------------------------------------
// normalize() function
// ---------------------------------------------------------------------------

/// Run normalization on statement results.
///
/// Parameters
/// ----------
/// results : StatementResult
///     Evaluated statement results.
/// config : NormalizationConfig
///     Normalization configuration (target node + adjustments).
///
/// Returns
/// -------
/// str
///     JSON-serialized list of ``NormalizationResult`` objects.
#[pyfunction]
#[pyo3(text_signature = "(results, config, /)")]
fn normalize(results: &PyStatementResult, config: &PyNormalizationConfig) -> PyResult<String> {
    let norm_results =
        finstack_quant_statements::adjustments::engine::NormalizationEngine::normalize(
            &results.inner,
            &config.inner,
        )
        .map_err(display_to_py)?;

    serde_json::to_string(&norm_results).map_err(display_to_py)
}

/// Register adjustment classes and functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyNormalizationConfig>()?;
    m.add_function(pyo3::wrap_pyfunction!(normalize, m)?)?;
    Ok(())
}
