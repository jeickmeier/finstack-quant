//! Covenant forecasting: `CovenantForecastConfig`, `CovenantForecast`,
//! `FutureBreach` and the DataFrame-backed `forecast_covenant` /
//! `forecast_breaches` entry points.
//!
//! A date-indexed `pandas.DataFrame` is read into the Rust
//! `DatedMetricSeries`, which owns the period mapping (every date is a daily
//! period ending on itself), ordering and duplicate-date checks, so no
//! statement model is needed.

use super::engine::{PyCovenantEngine, extract_metric_frame};
use super::spec::PyCovenantSpec;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::tabular_methods;
use crate::bindings::repr_support::repr_from_serde;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_covenants::{
    CovenantForecast, CovenantForecastConfig, DatedMetricSeries, FutureBreach,
};
use pyo3::prelude::*;

/// Read a date-indexed metrics frame as a Rust `DatedMetricSeries`.
///
/// Each frame row is one forecast period on its index date. Rows carry no
/// period start, so Rust requires `reference_date` for stochastic forecasts.
fn metric_series(frame: &Bound<'_, PyAny>, date_col: Option<&str>) -> PyResult<DatedMetricSeries> {
    DatedMetricSeries::new(extract_metric_frame(frame, date_col)?).map_err(core_to_py)
}

/// Forecast policy: deterministic pass/fail, or a lognormal stochastic
/// overlay (closed-form when ``num_paths == 0``, Monte Carlo otherwise).
///
/// ``volatility`` is the annualized lognormal volatility of the metric and is
/// required when ``stochastic`` is true; ``reference_date`` anchors the
/// ``sqrt(T)`` horizon scaling and is required for a stochastic
/// ``forecast_covenant`` / ``forecast_breaches`` (a date-indexed frame says
/// nothing about where its first reporting period starts, so pass that start,
/// e.g. the day before the first quarter begins);
/// ``breach_probability_threshold`` (decimal, default ``0.05``) is the
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
            reference_date: reference_date.map(py_to_date).transpose()?,
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
        Ok(Self::from_inner(self.inner.clone().with_scope(
            crate::bindings::extract::parse_label::<finstack_quant_covenants::CovenantScope>(
                scope,
            )?,
        )))
    }

    /// Scope selected by batch forecasts: maintenance or hypothetical incurrence capacity.
    #[getter]
    fn scope(&self) -> PyResult<String> {
        crate::bindings::extract::label(&self.inner.scope)
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
    fn comparator(&self) -> PyResult<String> {
        crate::bindings::extract::label(&self.inner.comparator)
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

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.covenants._forecast", "CovenantForecast.headroom_raw")]
    fn headroom_raw(&self) -> Vec<Option<f64>> {
        self.inner.headroom_raw.clone()
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

impl PyCovenantForecast {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let n = self.inner.test_dates.len();
        TableBuilder::new()
            .utf8(
                "test_date",
                self.inner
                    .test_dates
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            )?
            .nullable_f64(
                "projected_value",
                (0..n)
                    .map(|i| self.inner.projected_values.get(i).copied().flatten())
                    .collect(),
            )?
            .nullable_f64(
                "threshold",
                (0..n)
                    .map(|i| self.inner.thresholds.get(i).copied())
                    .collect(),
            )?
            .nullable_f64(
                "headroom",
                (0..n)
                    .map(|i| self.inner.headroom.get(i).copied().flatten())
                    .collect(),
            )?
            .nullable_f64(
                "headroom_raw",
                (0..n)
                    .map(|i| self.inner.headroom_raw.get(i).copied().flatten())
                    .collect(),
            )?
            .nullable_f64(
                "breach_probability",
                (0..n)
                    .map(|i| self.inner.breach_probability.get(i).copied())
                    .collect(),
            )?
            .nullable_f64(
                "breach_probability_stderr",
                (0..n)
                    .map(|i| self.inner.breach_probability_stderr.get(i).copied())
                    .collect(),
            )?
            .build()
    }
}

tabular_methods!(PyCovenantForecast);

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

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.covenants._forecast", "FutureBreach.headroom_raw")]
    fn headroom_raw(&self) -> Option<f64> {
        self.inner.headroom_raw
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
#[pyo3(signature = (spec, metrics, config=None, *, date_col=None), text_signature = "(spec, metrics, config=None, *, date_col=None)")]
pub(crate) fn forecast_covenant(
    py: Python<'_>,
    spec: PyRef<'_, PyCovenantSpec>,
    metrics: &Bound<'_, PyAny>,
    config: Option<PyRef<'_, PyCovenantForecastConfig>>,
    date_col: Option<&str>,
) -> PyResult<PyCovenantForecast> {
    let series = metric_series(metrics, date_col)?;
    let config = config.map_or_else(CovenantForecastConfig::default, |c| c.inner.clone());
    let spec = spec.inner.clone();
    py.detach(|| {
        finstack_quant_covenants::forecast_covenant(&spec, &series, config)
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
#[pyo3(signature = (engine, metrics, config=None, *, date_col=None), text_signature = "(engine, metrics, config=None, *, date_col=None)")]
pub(crate) fn forecast_breaches(
    py: Python<'_>,
    engine: PyRef<'_, PyCovenantEngine>,
    metrics: &Bound<'_, PyAny>,
    config: Option<PyRef<'_, PyCovenantForecastConfig>>,
    date_col: Option<&str>,
) -> PyResult<Vec<PyFutureBreach>> {
    let series = metric_series(metrics, date_col)?;
    let config = config.map_or_else(CovenantForecastConfig::default, |c| c.inner.clone());
    let engine = engine.inner.clone();
    py.detach(|| {
        finstack_quant_covenants::forecast_breaches(&engine, &series, config)
            .map(|breaches| {
                breaches
                    .into_iter()
                    .map(PyFutureBreach::from_inner)
                    .collect()
            })
            .map_err(core_to_py)
    })
}

/// Flatten a list of ``FutureBreach`` into an ``ArrowTable`` with one row per breach.
///
/// Columns: ``covenant_id``, ``covenant_description``, ``breach_date`` (ISO
/// string), ``projected_value``, ``threshold``, ``headroom``, ``headroom_raw``,
/// ``breach_probability``.
#[pyfunction]
#[pyo3(text_signature = "(breaches)")]
pub(crate) fn breaches_table(breaches: Vec<PyRef<'_, PyFutureBreach>>) -> PyResult<PyArrowTable> {
    TableBuilder::new()
        .utf8(
            "covenant_id",
            breaches
                .iter()
                .map(|b| b.inner.covenant_id.clone())
                .collect(),
        )?
        .utf8(
            "covenant_description",
            breaches
                .iter()
                .map(|b| b.inner.covenant_description.clone())
                .collect(),
        )?
        .utf8(
            "breach_date",
            breaches
                .iter()
                .map(|b| b.inner.breach_date.to_string())
                .collect(),
        )?
        .nullable_f64(
            "projected_value",
            breaches.iter().map(|b| b.inner.projected_value).collect(),
        )?
        .f64(
            "threshold",
            breaches.iter().map(|b| b.inner.threshold).collect(),
        )?
        .nullable_f64(
            "headroom",
            breaches.iter().map(|b| b.inner.headroom).collect(),
        )?
        .nullable_f64(
            "headroom_raw",
            breaches.iter().map(|b| b.inner.headroom_raw).collect(),
        )?
        .f64(
            "breach_probability",
            breaches
                .iter()
                .map(|b| b.inner.breach_probability)
                .collect(),
        )?
        .build()
}
