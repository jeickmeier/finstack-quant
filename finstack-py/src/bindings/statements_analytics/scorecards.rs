//! Python bindings for the credit scorecard extension.
//!
//! Wraps [`finstack_statements_analytics::extensions::scorecards`] types:
//!
//! - [`PyScorecardMetric`] — single metric definition (name, formula, weight, thresholds).
//! - [`PyScorecardConfig`] — full scorecard configuration (rating scale, metrics, optional minimum rating).
//! - [`PyCreditScorecardExtension`] — extension wrapper exposing `execute()` against a model + statement results.
//! - [`PyScorecardReport`] — execution report (status, message, structured data, warnings, errors).
//!
//! Reports and configs are JSON round-trippable via `to_json`/`from_json`.

use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::errors::display_to_py;
use finstack_statements_analytics::extensions::scorecards as rust_scorecards;
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// ScorecardMetric
// ---------------------------------------------------------------------------

/// A single scorecard metric definition.
///
/// Parameters
/// ----------
/// name : str
///     Metric name.
/// formula : str
///     DSL formula computing the metric value.
/// weight : float
///     Weight in the overall score (default 1.0).
/// thresholds_json : str
///     JSON mapping of rating label to ``[min, max]`` pairs (default ``"{}"``).
/// description : str | None
///     Optional human-readable description.
#[pyclass(
    name = "ScorecardMetric",
    module = "finstack.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScorecardMetric {
    pub(crate) inner: rust_scorecards::ScorecardMetric,
}

#[pymethods]
impl PyScorecardMetric {
    #[new]
    #[pyo3(signature = (name, formula, weight=1.0, thresholds_json="{}", description=None))]
    fn new(
        name: &str,
        formula: &str,
        weight: f64,
        thresholds_json: &str,
        description: Option<&str>,
    ) -> PyResult<Self> {
        let thresholds: indexmap::IndexMap<String, (f64, f64)> =
            serde_json::from_str(thresholds_json).map_err(display_to_py)?;
        Ok(Self {
            inner: rust_scorecards::ScorecardMetric {
                name: name.to_string(),
                formula: formula.to_string(),
                weight,
                thresholds,
                description: description.map(str::to_string),
            },
        })
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn formula(&self) -> &str {
        &self.inner.formula
    }

    #[getter]
    fn weight(&self) -> f64 {
        self.inner.weight
    }

    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    /// JSON-serialized thresholds (`{"AAA": [0.0, 1.0], ...}`).
    fn thresholds_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.thresholds).map_err(display_to_py)
    }

    /// Round-trip via JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Build a metric from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_scorecards::ScorecardMetric =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "ScorecardMetric(name='{}', weight={})",
            self.inner.name, self.inner.weight
        )
    }
}

// ---------------------------------------------------------------------------
// ScorecardConfig
// ---------------------------------------------------------------------------

/// Configuration for credit scorecard analysis.
///
/// Parameters
/// ----------
/// rating_scale : str
///     Rating scale identifier (e.g. ``"S&P"``, ``"Moody's"``, ``"Fitch"``).
/// metrics : list[ScorecardMetric]
///     Scorecard metrics to evaluate.
/// min_rating : str | None
///     Optional minimum acceptable rating.
#[pyclass(
    name = "ScorecardConfig",
    module = "finstack.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScorecardConfig {
    pub(crate) inner: rust_scorecards::ScorecardConfig,
}

#[pymethods]
impl PyScorecardConfig {
    #[new]
    #[pyo3(signature = (rating_scale="S&P", metrics=Vec::new(), min_rating=None))]
    fn new(
        rating_scale: &str,
        metrics: Vec<PyScorecardMetric>,
        min_rating: Option<&str>,
    ) -> Self {
        Self {
            inner: rust_scorecards::ScorecardConfig {
                rating_scale: rating_scale.to_string(),
                metrics: metrics.into_iter().map(|m| m.inner).collect(),
                min_rating: min_rating.map(str::to_string),
            },
        }
    }

    #[getter]
    fn rating_scale(&self) -> &str {
        &self.inner.rating_scale
    }

    #[getter]
    fn min_rating(&self) -> Option<&str> {
        self.inner.min_rating.as_deref()
    }

    #[getter]
    fn metrics(&self) -> Vec<PyScorecardMetric> {
        self.inner
            .metrics
            .iter()
            .cloned()
            .map(|inner| PyScorecardMetric { inner })
            .collect()
    }

    /// Validate the configuration without executing.
    fn validate(&self) -> PyResult<()> {
        rust_scorecards::CreditScorecardExtension::validate_config(&self.inner)
            .map_err(display_to_py)
    }

    /// Serialize this config to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Build a config from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_scorecards::ScorecardConfig =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "ScorecardConfig(rating_scale='{}', metrics={}, min_rating={:?})",
            self.inner.rating_scale,
            self.inner.metrics.len(),
            self.inner.min_rating
        )
    }
}

// ---------------------------------------------------------------------------
// ScorecardReport
// ---------------------------------------------------------------------------

/// Report produced by [`PyCreditScorecardExtension.execute`].
#[pyclass(
    name = "ScorecardReport",
    module = "finstack.statements_analytics",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScorecardReport {
    pub(crate) inner: rust_scorecards::ScorecardReport,
}

#[pymethods]
impl PyScorecardReport {
    /// ``"success"`` or ``"failed"``.
    #[getter]
    fn status(&self) -> String {
        match self.inner.status {
            rust_scorecards::ScorecardStatus::Success => "success".to_string(),
            rust_scorecards::ScorecardStatus::Failed => "failed".to_string(),
        }
    }

    #[getter]
    fn message(&self) -> &str {
        &self.inner.message
    }

    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings.clone()
    }

    #[getter]
    fn errors(&self) -> Vec<String> {
        self.inner.errors.clone()
    }

    /// Return the structured data payload as a JSON string.
    fn data_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.data).map_err(display_to_py)
    }

    /// Serialize the full report to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Build a report from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_scorecards::ScorecardReport =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "ScorecardReport(status='{}', warnings={}, errors={})",
            match self.inner.status {
                rust_scorecards::ScorecardStatus::Success => "success",
                rust_scorecards::ScorecardStatus::Failed => "failed",
            },
            self.inner.warnings.len(),
            self.inner.errors.len()
        )
    }
}

// ---------------------------------------------------------------------------
// CreditScorecardExtension
// ---------------------------------------------------------------------------

/// Credit scorecard extension for rating assignment and stress testing.
#[pyclass(
    name = "CreditScorecardExtension",
    module = "finstack.statements_analytics",
    skip_from_py_object
)]
pub struct PyCreditScorecardExtension {
    pub(crate) inner: rust_scorecards::CreditScorecardExtension,
}

#[pymethods]
impl PyCreditScorecardExtension {
    /// Construct a new extension with no configuration.
    #[new]
    fn new() -> Self {
        Self {
            inner: rust_scorecards::CreditScorecardExtension::new(),
        }
    }

    /// Construct an extension preloaded with a configuration.
    #[staticmethod]
    fn with_config(config: PyScorecardConfig) -> Self {
        Self {
            inner: rust_scorecards::CreditScorecardExtension::with_config(config.inner),
        }
    }

    /// Replace the current configuration.
    fn set_config(&mut self, config: PyScorecardConfig) {
        self.inner.set_config(config.inner);
    }

    /// Return the current configuration, if any.
    fn config(&self) -> Option<PyScorecardConfig> {
        self.inner
            .config()
            .cloned()
            .map(|inner| PyScorecardConfig { inner })
    }

    /// Run the scorecard against a model and pre-computed statement results.
    fn execute(
        &mut self,
        model: &Bound<'_, PyAny>,
        results: &Bound<'_, PyAny>,
    ) -> PyResult<PyScorecardReport> {
        let model = extract_model_ref(model)?;
        let results = extract_results_ref(results)?;
        let inner = self.inner.execute(&model, &results).map_err(display_to_py)?;
        Ok(PyScorecardReport { inner })
    }
}

impl Default for PyCreditScorecardExtension {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free function: validate_scorecard_config
// ---------------------------------------------------------------------------

/// Validate a [`ScorecardConfig`] payload (typed object) without executing.
#[pyfunction]
fn validate_scorecard_config(config: &PyScorecardConfig) -> PyResult<()> {
    rust_scorecards::CreditScorecardExtension::validate_config(&config.inner).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register scorecard types and functions on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyScorecardMetric>()?;
    m.add_class::<PyScorecardConfig>()?;
    m.add_class::<PyScorecardReport>()?;
    m.add_class::<PyCreditScorecardExtension>()?;
    m.add_function(pyo3::wrap_pyfunction!(validate_scorecard_config, m)?)?;
    Ok(())
}
