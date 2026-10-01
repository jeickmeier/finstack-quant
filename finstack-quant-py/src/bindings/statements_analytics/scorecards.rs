//! Python bindings for the credit scorecard extension.
//!
//! Wraps [`finstack_quant_statements_analytics::extensions::scorecards`] types:
//!
//! - [`PyScorecardMetric`] — single metric definition (name, formula, weight, thresholds).
//! - [`PyScorecardConfig`] — full scorecard configuration (rating scale, metrics, optional minimum rating).
//! - [`PyCreditScorecardExtension`] — extension wrapper exposing `execute()` against a model + statement results.
//! - [`PyScorecardReport`] — execution report (status, message, structured data, warnings, errors).
//!
//! Reports and configs are JSON round-trippable via `to_json`/`from_json`.

use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::errors::display_to_py;
use finstack_quant_statements_analytics::extensions::scorecards as rust_scorecards;
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

    /// Round-trip via JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
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
    #[pyo3(signature = (rating_scale="S&P", metrics=Vec::new(), min_rating=None, period=None))]
    fn new(
        rating_scale: &str,
        metrics: Vec<PyScorecardMetric>,
        min_rating: Option<&str>,
        period: Option<&str>,
    ) -> Self {
        Self {
            inner: rust_scorecards::ScorecardConfig {
                rating_scale: rating_scale.to_string(),
                metrics: metrics.into_iter().map(|m| m.inner).collect(),
                min_rating: min_rating.map(str::to_string),
                period: period.map(str::to_string),
            },
        }
    }

    /// Rating scale identifier (e.g. ``"S&P"``, ``"Moody's"``, ``"Fitch"``).
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
            .map_err(display_to_py)
    }

    /// Serialize this config to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
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
            "ScorecardConfig(rating_scale='{}', metrics={}, min_rating={:?}, period={:?})",
            self.inner.rating_scale,
            self.inner.metrics.len(),
            self.inner.min_rating,
            self.inner.period
        )
    }
}

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
    /// ``"success"`` or ``"failed"`` — the canonical serde discriminant of
    /// the Rust ``ScorecardStatus`` enum.
    #[getter]
    fn status(&self) -> String {
        super::serde_variant_str(&self.inner.status)
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

    /// Export the report header as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``status``, ``message``, ``rating``, ``rating_scale``,
    /// ``period``, ``total_score``, ``partial``, ``weight_coverage``,
    /// ``warning_count``, ``error_count``.
    ///
    /// ``period`` is the rated period-id string. ``weight_coverage`` is a
    /// decimal fraction in ``[0, 1]``: the included metric weight over the
    /// configured metric weight, so ``0.8`` means a fifth of the configured
    /// weight was excluded. ``partial`` is ``True`` when any metric was
    /// excluded or errored. Fields absent from the report payload are
    /// ``None``. Per-metric detail lives in
    /// ``to_metric_scores_dataframe``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "status": self.status(),
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

    /// Serialize the full report to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
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
            self.status(),
            self.inner.warnings.len(),
            self.inner.errors.len()
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

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
        &mut self,
        model: &Bound<'_, PyAny>,
        results: &Bound<'_, PyAny>,
    ) -> PyResult<PyScorecardReport> {
        let model = extract_model_ref(model)?;
        let results = extract_results_ref(results)?;
        let inner = self
            .inner
            .execute(&model, &results)
            .map_err(display_to_py)?;
        Ok(PyScorecardReport { inner })
    }
}

/// Validate a [`ScorecardConfig`] payload (typed object) without executing.
#[pyfunction]
fn validate_scorecard_config(config: &PyScorecardConfig) -> PyResult<()> {
    rust_scorecards::CreditScorecardExtension::validate_config(&config.inner).map_err(display_to_py)
}

/// Register scorecard types and functions on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyScorecardMetric>()?;
    m.add_class::<PyScorecardConfig>()?;
    m.add_class::<PyScorecardReport>()?;
    m.add_class::<PyCreditScorecardExtension>()?;
    m.add_function(pyo3::wrap_pyfunction!(validate_scorecard_config, m)?)?;
    Ok(())
}
