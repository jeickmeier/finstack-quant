//! Python bindings for the credit scorecard extension.
//!
//! Wraps [`finstack_quant_statements_analytics::extensions`] types:
//!
//! - [`PyScorecardMetric`] — single metric definition (name, formula, weight, thresholds).
//! - [`PyScorecardConfig`] — full scorecard configuration (rating scale, metrics, optional minimum rating).
//! - [`PyCreditScorecardExtension`] — extension wrapper exposing `execute()` against a model + statement results.
//! - [`PyScorecardReport`] — execution report (status, message, structured data, warnings, errors).
//!
//! Reports and configs are JSON round-trippable via `to_json`/`from_json`.

use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::bindings::pydoc::pydoc;
use crate::errors::display_to_py;
use finstack_quant_statements_analytics::extensions as rust_scorecards;
use pyo3::prelude::*;

/// Column schema for [`PyScorecardReport::to_metric_scores_dataframe`].
const METRIC_SCORE_COLUMNS: [ColumnSchema<'static>; 5] = [
    ("metric", "str"),
    ("value", "float64"),
    ("score", "float64"),
    ("weight", "float64"),
    ("weighted_score", "float64"),
];

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
/// thresholds : dict[str, tuple[float, float]] | str
///     Rating label to ``(min, max)`` band, or the same mapping as a JSON
///     string (default ``{}``).
/// description : str | None
///     Optional human-readable description.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import ScorecardMetric
/// >>> ScorecardMetric("leverage", "total_debt / ebitda", 1.0, {"A": (0.0, 2.0)}).thresholds
/// {'A': (0.0, 2.0)}
#[pyclass(
    name = "ScorecardMetric",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScorecardMetric {
    pub(crate) inner: rust_scorecards::ScorecardMetric,
}

#[pymethods]
impl PyScorecardMetric {
    #[new]
    #[pyo3(signature = (name, formula, weight=1.0, thresholds=None, description=None))]
    fn new(
        py: Python<'_>,
        name: &str,
        formula: &str,
        weight: f64,
        thresholds: Option<&Bound<'_, PyAny>>,
        description: Option<&str>,
    ) -> PyResult<Self> {
        let thresholds: indexmap::IndexMap<String, (f64, f64)> = match thresholds {
            Some(value) => {
                crate::bindings::statements_analytics::extract_serde_any(py, value, "thresholds")?
            }
            None => indexmap::IndexMap::new(),
        };
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

    /// Metric name, used as the key in the report's metric scores.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// DSL formula evaluated to produce the metric value.
    #[getter]
    fn formula(&self) -> &str {
        &self.inner.formula
    }

    /// Weight of this metric in the overall score.
    ///
    /// Weights are relative and need not sum to 1; the report divides the
    /// included weight by the configured weight to report
    /// ``weight_coverage``.
    #[getter]
    fn weight(&self) -> f64 {
        self.inner.weight
    }

    /// Optional human-readable description of the metric.
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    /// Rating label to ``(min, max)`` band, sorted by rating label.
    #[getter]
    fn thresholds(&self) -> std::collections::BTreeMap<String, (f64, f64)> {
        self.inner
            .thresholds
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// JSON-serialized thresholds (`{"AAA": [0.0, 1.0], ...}`).
    fn thresholds_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.thresholds).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "ScorecardMetric(name='{}', weight={})",
            self.inner.name, self.inner.weight
        )
    }
}

wire_methods!(
    PyScorecardMetric,
    rust_scorecards::ScorecardMetric,
    "ScorecardMetric"
);

/// Configuration for credit scorecard analysis.
///
/// Parameters
/// ----------
/// rating_scale : str | None
///     Canonical id (``"sp"`` or ``"moodys"``); None uses the Rust default (``"sp"``).
/// metrics : list[ScorecardMetric]
///     Scorecard metrics to evaluate.
/// min_rating : str | None
///     Optional minimum acceptable rating.
/// period : str | None
///     Optional period to rate, as a parseable period string (e.g.
///     ``"2025Q4"``). When ``None``, the scorecard rates the last actual
///     period in the model if any exists, otherwise the last model period.
#[pyclass(
    name = "ScorecardConfig",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScorecardConfig {
    pub(crate) inner: rust_scorecards::ScorecardConfig,
}

#[pymethods]
impl PyScorecardConfig {
    #[new]
    #[pyo3(signature = (rating_scale=None, metrics=Vec::new(), min_rating=None, period=None))]
    fn new(
        rating_scale: Option<&str>,
        metrics: Vec<PyScorecardMetric>,
        min_rating: Option<&str>,
        period: Option<&str>,
    ) -> Self {
        Self {
            inner: rust_scorecards::ScorecardConfig {
                rating_scale: rating_scale.map_or_else(
                    || rust_scorecards::ScorecardConfig::default().rating_scale,
                    str::to_string,
                ),
                metrics: metrics.into_iter().map(|m| m.inner).collect(),
                min_rating: min_rating.map(str::to_string),
                period: period.map(str::to_string),
            },
        }
    }

    /// Canonical rating-scale identifier (``"sp"`` or ``"moodys"``).
    #[getter]
    fn rating_scale(&self) -> &str {
        &self.inner.rating_scale
    }

    /// Minimum acceptable rating on ``rating_scale``, or ``None`` when the
    /// scorecard imposes no floor.
    #[getter]
    fn min_rating(&self) -> Option<&str> {
        self.inner.min_rating.as_deref()
    }

    /// Period to rate, as a period-id string (e.g. ``"2025Q4"``).
    ///
    /// ``None`` means the last actual period in the model if any exists,
    /// otherwise the last model period.
    #[getter]
    fn period(&self) -> Option<&str> {
        self.inner.period.as_deref()
    }

    /// Metric definitions evaluated by the scorecard, in configured order.
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
            .map_err(crate::errors::statements_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "ScorecardConfig(rating_scale='{}', metrics={}, min_rating={:?}, period={:?})",
            self.inner.rating_scale,
            self.inner.metrics.len(),
            self.inner.min_rating,
            self.inner.period
        )
    }
}

wire_methods!(
    PyScorecardConfig,
    rust_scorecards::ScorecardConfig,
    "ScorecardConfig"
);

/// Report produced by [`PyCreditScorecardExtension.execute`].
#[pyclass(
    name = "ScorecardReport",
    module = "finstack_quant.statements_analytics",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScorecardReport {
    pub(crate) inner: rust_scorecards::ScorecardReport,
}

#[pymethods]
impl PyScorecardReport {
    #[doc = pydoc!("finstack_quant.statements_analytics._scorecards", "ScorecardReport.status")]
    #[getter]
    fn status(&self) -> PyResult<String> {
        crate::bindings::extract::label(&self.inner.status)
    }

    /// Human-readable summary of the run.
    #[getter]
    fn message(&self) -> &str {
        &self.inner.message
    }

    /// Non-fatal warnings raised while scoring (e.g. an excluded metric).
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings.clone()
    }

    /// Per-metric failures. A non-empty list means ``status`` is
    /// ``"failed"``.
    #[getter]
    fn errors(&self) -> Vec<String> {
        self.inner.errors.clone()
    }

    /// Structured data payload as a dict.
    ///
    /// Includes the rated ``period``, the ``partial`` flag, and
    /// ``weight_coverage`` alongside the per-metric scores and rating.
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.data)
    }

    /// Return the structured data payload as a JSON string (the wire form of
    /// ``data``).
    fn data_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.data).map_err(display_to_py)
    }

    #[doc = pydoc!("finstack_quant.statements_analytics._scorecards", "ScorecardReport.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "status": self.status()?,
            "message": self.inner.message,
            "rating": self.data_field("rating"),
            "rating_scale": self.data_field("rating_scale"),
            "period": self.data_field("period"),
            "total_score": self.data_field("total_score"),
            "partial": self.data_field("partial"),
            "weight_coverage": self.data_field("weight_coverage"),
            "warning_count": self.inner.warnings.len(),
            "error_count": self.inner.errors.len(),
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "status",
                "message",
                "rating",
                "rating_scale",
                "period",
                "total_score",
                "partial",
                "weight_coverage",
                "warning_count",
                "error_count",
            ],
        )
    }

    /// Export the per-metric scores as a pandas ``DataFrame``.
    ///
    /// Columns: ``metric``, ``value``, ``score``, ``weight``,
    /// ``weighted_score``. One row per scored metric, in configured order;
    /// a report with no scored metrics still carries the full column schema.
    ///
    /// ``value`` is the metric's evaluated value in its own units, ``score``
    /// its mapped rating score, ``weight`` the configured weight, and
    /// ``weighted_score`` is ``score * weight``. Metrics that errored or were
    /// excluded do not appear here — see ``errors`` and ``weight_coverage``.
    fn to_metric_scores_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self
            .inner
            .data
            .get("metric_scores")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        serde_rows_to_dataframe_with_schema(py, &rows, &METRIC_SCORE_COLUMNS)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "ScorecardReport(status='{}', warnings={}, errors={})",
            self.status()?,
            self.inner.warnings.len(),
            self.inner.errors.len()
        ))
    }
}

wire_methods!(
    PyScorecardReport,
    rust_scorecards::ScorecardReport,
    "ScorecardReport"
);
impl_repr_html_via_dataframe!(PyScorecardReport);

impl PyScorecardReport {
    /// Read one scalar entry out of the structured `data` payload.
    ///
    /// Returns JSON `null` when the key is absent, which pandas renders as a
    /// missing value rather than raising.
    fn data_field(&self, key: &str) -> serde_json::Value {
        self.inner
            .data
            .get(key)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }
}

/// Credit scorecard extension for rating assignment and stress testing.
#[pyclass(
    name = "CreditScorecardExtension",
    module = "finstack_quant.statements_analytics",
    skip_from_py_object
)]
pub struct PyCreditScorecardExtension {
    pub(crate) inner: rust_scorecards::CreditScorecardExtension,
}

#[pymethods]
impl PyCreditScorecardExtension {
    /// Construct an extension with its configuration.
    #[new]
    fn new(config: PyScorecardConfig) -> Self {
        Self {
            inner: rust_scorecards::CreditScorecardExtension::new(config.inner),
        }
    }

    /// Return the current configuration.
    fn config(&self) -> PyScorecardConfig {
        PyScorecardConfig {
            inner: self.inner.config().clone(),
        }
    }

    /// Run the scorecard against a model and pre-computed statement results.
    fn execute(
        &self,
        model: &Bound<'_, PyAny>,
        results: &Bound<'_, PyAny>,
    ) -> PyResult<PyScorecardReport> {
        let model = extract_model_ref(model)?;
        let results = extract_results_ref(results)?;
        let inner = self
            .inner
            .execute(&model, &results)
            .map_err(crate::errors::statements_to_py)?;
        Ok(PyScorecardReport { inner })
    }
}

/// Register scorecard types and functions on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyScorecardMetric>()?;
    m.add_class::<PyScorecardConfig>()?;
    m.add_class::<PyScorecardReport>()?;
    m.add_class::<PyCreditScorecardExtension>()?;
    Ok(())
}
