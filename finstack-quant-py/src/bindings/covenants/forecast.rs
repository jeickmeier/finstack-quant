//! Covenant forecasting: `CovenantForecastConfig`, `CovenantForecast`,
//! `FutureBreach` and the DataFrame-backed `forecast_covenant` /
//! `forecast_breaches` entry points.
//!
//! The Rust forecaster reads projections through the `ModelTimeSeries` trait,
//! which is keyed by `PeriodId`. `FrameTimeSeries` is a pure data adapter over
//! a date-indexed `pandas.DataFrame`: every index date becomes a daily
//! `PeriodId` (year + ordinal day) whose period end is the date itself, so no
//! statement model is needed.

use super::engine::{extract_metric_frame, PyCovenantEngine};
use super::spec::PyCovenantSpec;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, ColumnSchema};
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, display_to_py, value_error};
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_covenants::{
    forecast_breaches_generic, forecast_covenant_generic, BoundKind, CovenantForecast,
    CovenantForecastConfig, FutureBreach, ModelTimeSeries,
};
use pyo3::prelude::*;
use std::collections::HashMap;

/// Date-indexed metric projections exposed as a `ModelTimeSeries`.
///
/// Each frame row is one forecast period identified by the daily `PeriodId`
/// of its index date; `period_end_date` maps that identifier back to the
/// date. With `reference_date` unset in the config the forecaster anchors
/// stochastic horizons on the day before the first row.
struct FrameTimeSeries {
    periods: Vec<PeriodId>,
    values: HashMap<PeriodId, HashMap<String, f64>>,
}

impl FrameTimeSeries {
    fn from_frame(frame: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut rows = extract_metric_frame(frame)?;
        if rows.is_empty() {
            return Err(value_error("metrics frame must contain at least one row"));
        }
        rows.sort_by_key(|(date, _)| *date);
        let mut periods = Vec::with_capacity(rows.len());
        let mut values = HashMap::with_capacity(rows.len());
        for (date, pairs) in rows {
            let period = PeriodId::day(date.year(), date.ordinal()).map_err(core_to_py)?;
            if values.insert(period, pairs.into_iter().collect()).is_some() {
                return Err(value_error(format!(
                    "metrics frame index contains duplicate date {date}"
                )));
            }
            periods.push(period);
        }
        Ok(Self { periods, values })
    }
}

impl ModelTimeSeries for FrameTimeSeries {
    fn get_scalar(&self, node_id: &str, period: &PeriodId) -> Option<f64> {
        self.values.get(period)?.get(node_id).copied()
    }

    fn period_end_date(&self, period: &PeriodId) -> Date {
        Date::from_ordinal_date(period.year, period.index).unwrap_or(Date::MIN)
    }
}

fn bound_kind_name(kind: BoundKind) -> &'static str {
    match kind {
        BoundKind::AtMost => "at_most",
        BoundKind::AtLeast => "at_least",
    }
}

/// Forecast policy: deterministic pass/fail, or a lognormal stochastic
/// overlay (closed-form when ``num_paths == 0``, Monte Carlo otherwise).
///
/// ``volatility`` is the annualized lognormal volatility of the metric and is
/// required when ``stochastic`` is true; ``reference_date`` anchors the
/// ``sqrt(T)`` horizon scaling (default: the day before the first forecast
/// date); ``breach_probability_threshold`` (decimal, default ``0.05``) is the
/// minimum probability for ``forecast_breaches`` to report a date.
#[pyclass(
    name = "CovenantForecastConfig",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantForecastConfig {
    pub(crate) inner: CovenantForecastConfig,
}

impl PyCovenantForecastConfig {
    pub(crate) fn from_inner(inner: CovenantForecastConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenantForecastConfig {
    /// Create a forecast configuration; every argument defaults to the Rust
    /// ``CovenantForecastConfig::default()`` value.
    #[new]
    #[pyo3(
        signature = (stochastic=false, num_paths=0, volatility=None, random_seed=None, antithetic=false, reference_date=None, breach_probability_threshold=0.05),
        text_signature = "(stochastic=False, num_paths=0, volatility=None, random_seed=None, antithetic=False, reference_date=None, breach_probability_threshold=0.05)"
    )]
    fn new(
        stochastic: bool,
        num_paths: usize,
        volatility: Option<f64>,
        random_seed: Option<u64>,
        antithetic: bool,
        reference_date: Option<&Bound<'_, PyAny>>,
        breach_probability_threshold: f64,
    ) -> PyResult<Self> {
        Ok(Self::from_inner(CovenantForecastConfig {
            scope: finstack_quant_covenants::CovenantScope::Maintenance,
            stochastic,
            num_paths,
            volatility,
            random_seed,
            antithetic,
            reference_date: reference_date.map(extract_date).transpose()?,
            breach_probability_threshold,
        }))
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return a copy selecting maintenance compliance or hypothetical incurrence capacity.
    /// ``scope`` must be ``maintenance`` or ``incurrence``; other strings raise ``ValueError``.
    #[pyo3(text_signature = "(scope)")]
    fn with_scope(&self, scope: &str) -> PyResult<Self> {
        Ok(Self::from_inner(
            self.inner
                .clone()
                .with_scope(super::spec::parse_scope(scope)?),
        ))
    }

    /// Scope selected by batch forecasts: maintenance or hypothetical incurrence capacity.
    #[getter]
    fn scope(&self) -> &'static str {
        super::spec::scope_name(&self.inner.scope)
    }

    /// Whether breach probabilities use the stochastic overlay.
    #[getter]
    fn stochastic(&self) -> bool {
        self.inner.stochastic
    }

    /// Monte Carlo path count; ``0`` selects the closed-form analytic mode.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Annualized lognormal volatility, or ``None`` in deterministic mode.
    #[getter]
    fn volatility(&self) -> Option<f64> {
        self.inner.volatility
    }

    /// RNG seed for Monte Carlo mode (``None`` uses the crate default ``0``).
    #[getter]
    fn random_seed(&self) -> Option<u64> {
        self.inner.random_seed
    }

    /// Whether Monte Carlo paths are simulated in antithetic pairs.
    #[getter]
    fn antithetic(&self) -> bool {
        self.inner.antithetic
    }

    /// Horizon anchor for stochastic scaling, or ``None`` for the default.
    #[getter]
    fn reference_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .reference_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Minimum breach probability reported by ``forecast_breaches``.
    #[getter]
    fn breach_probability_threshold(&self) -> f64 {
        self.inner.breach_probability_threshold
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CovenantForecastConfig", &self.inner)
    }
}

/// Columns of `CovenantForecast.to_dataframe`.
const FORECAST_COLUMNS: [ColumnSchema<'static>; 6] = [
    ("test_date", "str"),
    ("projected_value", "float64"),
    ("threshold", "float64"),
    ("headroom", "float64"),
    ("breach_probability", "float64"),
    ("breach_probability_stderr", "float64"),
];

/// Forward compliance projection for one covenant across the forecast dates.
///
/// Per date it carries the projected metric, the threshold in force (static
/// or step-down schedule), relative headroom (``None`` while a springing
/// covenant is inactive or the ratio is not meaningful) and the breach
/// probability (``0``/``1`` in deterministic mode).
#[pyclass(
    name = "CovenantForecast",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantForecast {
    pub(crate) inner: CovenantForecast,
}

impl PyCovenantForecast {
    pub(crate) fn from_inner(inner: CovenantForecast) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenantForecast {
    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Instance label of the forecast covenant.
    #[getter]
    fn covenant_id(&self) -> &str {
        &self.inner.covenant_id
    }

    /// Human-readable covenant description.
    #[getter]
    fn covenant_description(&self) -> &str {
        &self.inner.covenant_description
    }

    /// Test direction: ``"at_most"`` or ``"at_least"``.
    #[getter]
    fn comparator(&self) -> &'static str {
        bound_kind_name(self.inner.comparator)
    }

    /// Forecast test dates as ``datetime.date``.
    #[getter]
    fn test_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .test_dates
            .iter()
            .map(|date| date_to_py(py, *date))
            .collect()
    }

    /// Projected metric per test date (``None`` when not finite).
    #[getter]
    fn projected_values(&self) -> Vec<Option<f64>> {
        self.inner.projected_values.clone()
    }

    /// Threshold in force per test date.
    #[getter]
    fn thresholds(&self) -> Vec<f64> {
        self.inner.thresholds.clone()
    }

    /// Relative headroom per test date (``None`` when inactive or not
    /// meaningful).
    #[getter]
    fn headroom(&self) -> Vec<Option<f64>> {
        self.inner.headroom.clone()
    }

    /// Breach probability per test date.
    #[getter]
    fn breach_probability(&self) -> Vec<f64> {
        self.inner.breach_probability.clone()
    }

    /// Monte Carlo standard error of each breach probability (zeros in
    /// deterministic and analytic modes).
    #[getter]
    fn breach_probability_stderr(&self) -> Vec<f64> {
        self.inner.breach_probability_stderr.clone()
    }

    /// First projected breach date, or ``None``.
    #[getter]
    fn first_breach_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .first_breach_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Date of minimum finite headroom, or ``None``.
    #[getter]
    fn min_headroom_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .min_headroom_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Minimum finite headroom across active test dates, or ``None``.
    #[getter]
    fn min_headroom_value(&self) -> Option<f64> {
        self.inner.min_headroom_value
    }

    /// One row per test date with columns ``test_date`` (ISO string),
    /// ``projected_value``, ``threshold``, ``headroom``,
    /// ``breach_probability``, ``breach_probability_stderr``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = (0..self.inner.test_dates.len())
            .map(|i| {
                serde_json::json!({
                    "test_date": self.inner.test_dates[i].to_string(),
                    "projected_value": self.inner.projected_values.get(i).copied().flatten(),
                    "threshold": self.inner.thresholds.get(i).copied(),
                    "headroom": self.inner.headroom.get(i).copied().flatten(),
                    "breach_probability": self.inner.breach_probability.get(i).copied(),
                    "breach_probability_stderr": self.inner.breach_probability_stderr.get(i).copied(),
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &FORECAST_COLUMNS)
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`. Returns `None` if the frame
    /// cannot be built, which makes IPython fall back to `__repr__`.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    fn __len__(&self) -> usize {
        self.inner.test_dates.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "CovenantForecast(covenant_id={:?}, periods={}, first_breach_date={}, min_headroom_value={})",
            self.inner.covenant_id,
            self.inner.test_dates.len(),
            self.inner
                .first_breach_date
                .map_or("None".to_string(), |d| format!("\"{d}\"")),
            self.inner
                .min_headroom_value
                .map_or("None".to_string(), |v| v.to_string()),
        )
    }
}

/// A projected covenant breach on one forecast date.
#[pyclass(
    name = "FutureBreach",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyFutureBreach {
    pub(crate) inner: FutureBreach,
}

impl PyFutureBreach {
    pub(crate) fn from_inner(inner: FutureBreach) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFutureBreach {
    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Instance label of the covenant.
    #[getter]
    fn covenant_id(&self) -> &str {
        &self.inner.covenant_id
    }

    /// Human-readable covenant description.
    #[getter]
    fn covenant_description(&self) -> &str {
        &self.inner.covenant_description
    }

    /// Forecast date of the breach.
    #[getter]
    fn breach_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.breach_date)
    }

    /// Projected metric value, if finite.
    #[getter]
    fn projected_value(&self) -> Option<f64> {
        self.inner.projected_value
    }

    /// Threshold in force on the breach date.
    #[getter]
    fn threshold(&self) -> f64 {
        self.inner.threshold
    }

    /// Relative headroom (negative means breach), if meaningful.
    #[getter]
    fn headroom(&self) -> Option<f64> {
        self.inner.headroom
    }

    /// Breach probability (``1.0`` in deterministic mode).
    #[getter]
    fn breach_probability(&self) -> f64 {
        self.inner.breach_probability
    }

    fn __repr__(&self) -> String {
        repr_from_serde("FutureBreach", &self.inner)
    }
}

/// Forecast one numeric covenant across a date-indexed projection frame.
///
/// ``metrics`` is a ``pandas.DataFrame`` indexed by forecast date with one
/// unique column per metric id; required cells must be finite and every row is a test date.
/// ``config`` defaults to a deterministic forecast.
///
/// Raises ``KeyError`` when the covenant's metric is missing on any date,
/// and ``ValueError`` for an empty frame, a non-numeric covenant, a
/// non-finite threshold, or an invalid config (for example stochastic mode
/// without ``volatility``).
#[pyfunction]
#[pyo3(signature = (spec, metrics, config=None), text_signature = "(spec, metrics, config=None)")]
pub(crate) fn forecast_covenant(
    py: Python<'_>,
    spec: PyRef<'_, PyCovenantSpec>,
    metrics: &Bound<'_, PyAny>,
    config: Option<PyRef<'_, PyCovenantForecastConfig>>,
) -> PyResult<PyCovenantForecast> {
    let series = FrameTimeSeries::from_frame(metrics)?;
    let config = config.map_or_else(CovenantForecastConfig::default, |c| c.inner.clone());
    let spec = spec.inner.clone();
    py.detach(|| {
        forecast_covenant_generic(&spec, &series, &series.periods, config)
            .map(PyCovenantForecast::from_inner)
            .map_err(core_to_py)
    })
}

/// Forecast every active numeric covenant in ``engine`` and return the dates
/// whose breach probability reaches ``config.breach_probability_threshold``.
///
/// ``metrics`` is a date-indexed ``pandas.DataFrame`` as for
/// ``forecast_covenant``. Effective windows and waivers are honored. Missing
/// required metrics raise ``KeyError``; non-finite observations and duplicate
/// columns raise ``ValueError``. Default scope is maintenance; ``with_scope``
/// selects hypothetical incurrence capacity. Non-numeric covenants are skipped.
///
/// Raises ``ValueError`` for an empty frame, an invalid engine, or an
/// invalid config.
#[pyfunction]
#[pyo3(signature = (engine, metrics, config=None), text_signature = "(engine, metrics, config=None)")]
pub(crate) fn forecast_breaches(
    py: Python<'_>,
    engine: PyRef<'_, PyCovenantEngine>,
    metrics: &Bound<'_, PyAny>,
    config: Option<PyRef<'_, PyCovenantForecastConfig>>,
) -> PyResult<Vec<PyFutureBreach>> {
    let series = FrameTimeSeries::from_frame(metrics)?;
    let config = config.map_or_else(CovenantForecastConfig::default, |c| c.inner.clone());
    let engine = engine.inner.clone();
    py.detach(|| {
        forecast_breaches_generic(&engine, &series, &series.periods, config)
            .map(|breaches| {
                breaches
                    .into_iter()
                    .map(PyFutureBreach::from_inner)
                    .collect()
            })
            .map_err(core_to_py)
    })
}

/// Flatten a list of ``FutureBreach`` into one frame row per breach.
///
/// Columns: ``covenant_id``, ``covenant_description``, ``breach_date`` (ISO
/// string), ``projected_value``, ``threshold``, ``headroom``,
/// ``breach_probability``.
#[pyfunction]
#[pyo3(text_signature = "(breaches)")]
pub(crate) fn breaches_to_dataframe<'py>(
    py: Python<'py>,
    breaches: Vec<PyRef<'py, PyFutureBreach>>,
) -> PyResult<Bound<'py, PyAny>> {
    let rows: Vec<serde_json::Value> = breaches
        .iter()
        .map(|b| {
            serde_json::json!({
                "covenant_id": b.inner.covenant_id,
                "covenant_description": b.inner.covenant_description,
                "breach_date": b.inner.breach_date.to_string(),
                "projected_value": b.inner.projected_value,
                "threshold": b.inner.threshold,
                "headroom": b.inner.headroom,
                "breach_probability": b.inner.breach_probability,
            })
        })
        .collect();
    serde_rows_to_dataframe_with_schema(
        py,
        &rows,
        &[
            ("covenant_id", "str"),
            ("covenant_description", "str"),
            ("breach_date", "str"),
            ("projected_value", "float64"),
            ("threshold", "float64"),
            ("headroom", "float64"),
            ("breach_probability", "float64"),
        ],
    )
}
