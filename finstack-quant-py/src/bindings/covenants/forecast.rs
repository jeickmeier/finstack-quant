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
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pandas_utils::{ColumnSchema, serde_rows_to_dataframe_with_schema};
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;
use finstack_quant_covenants::{
    CovenantForecast, CovenantForecastConfig, DatedMetricSeries, FutureBreach,
};
use pyo3::prelude::*;

/// Read a date-indexed metrics frame as a Rust `DatedMetricSeries`.
///
/// Each frame row is one forecast period on its index date. Rows carry no
/// period start, so Rust requires `reference_date` for stochastic forecasts.
fn metric_series(frame: &Bound<'_, PyAny>) -> PyResult<DatedMetricSeries> {
    DatedMetricSeries::new(extract_metric_frame(frame)?).map_err(core_to_py)
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

    /// Horizon anchor for stochastic scaling, or ``None`` for the default.
    #[getter]
    fn reference_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .reference_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CovenantForecastConfig", &self.inner)
    }
}

crate::bindings::macros::wire_methods!(
    PyCovenantForecastConfig,
    CovenantForecastConfig,
    "CovenantForecastConfig"
);
crate::bindings::macros::py_getters!(PyCovenantForecastConfig |s| {
    /// Scope selected by batch forecasts: maintenance or hypothetical incurrence capacity.
    scope -> PyResult<String> = crate::bindings::extract::label(&s.inner.scope);

    /// Whether breach probabilities use the stochastic overlay.
    stochastic -> bool = s.inner.stochastic;

    /// Monte Carlo path count; ``0`` selects the closed-form analytic mode.
    num_paths -> usize = s.inner.num_paths;

    /// Annualized lognormal volatility, or ``None`` in deterministic mode.
    volatility -> Option<f64> = s.inner.volatility;

    /// RNG seed for Monte Carlo mode (``None`` uses the crate default ``0``).
    random_seed -> Option<u64> = s.inner.random_seed;

    /// Whether Monte Carlo paths are simulated in antithetic pairs.
    antithetic -> bool = s.inner.antithetic;

    /// Minimum breach probability reported by ``forecast_breaches``.
    breach_probability_threshold -> f64 = s.inner.breach_probability_threshold;
});

/// Columns of `CovenantForecast.to_dataframe`.
const FORECAST_COLUMNS: [ColumnSchema<'static>; 7] = [
    ("test_date", "str"),
    ("projected_value", "float64"),
    ("threshold", "float64"),
    ("headroom", "float64"),
    ("headroom_raw", "float64"),
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

crate::bindings::macros::wire_methods!(PyCovenantForecast, CovenantForecast, "CovenantForecast");
crate::bindings::macros::impl_repr_html_via_dataframe!(PyCovenantForecast);
crate::bindings::macros::py_getters!(PyCovenantForecast |s| {
    /// Instance label of the forecast covenant.
    covenant_id -> &str = &s.inner.covenant_id;

    /// Human-readable covenant description.
    covenant_description -> &str = &s.inner.covenant_description;

    /// Test direction: ``"at_most"`` or ``"at_least"``.
    comparator -> PyResult<String> = crate::bindings::extract::label(&s.inner.comparator);

    /// Projected metric per test date (``None`` when not finite).
    projected_values -> Vec<Option<f64>> = s.inner.projected_values.clone();

    /// Threshold in force per test date.
    thresholds -> Vec<f64> = s.inner.thresholds.clone();

    /// Relative headroom per test date (``None`` when inactive or not
    /// meaningful).
    headroom -> Vec<Option<f64>> = s.inner.headroom.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.covenants._forecast", "CovenantForecast.headroom_raw")]
    headroom_raw -> Vec<Option<f64>> = s.inner.headroom_raw.clone();

    /// Breach probability per test date.
    breach_probability -> Vec<f64> = s.inner.breach_probability.clone();

    /// Monte Carlo standard error of each breach probability (zeros in
    /// deterministic and analytic modes).
    breach_probability_stderr -> Vec<f64> = s.inner.breach_probability_stderr.clone();

    /// Minimum finite headroom across active test dates, or ``None``.
    min_headroom_value -> Option<f64> = s.inner.min_headroom_value;
});

#[pymethods]
impl PyCovenantForecast {
    /// Forecast test dates as ``datetime.date``.
    #[getter]
    fn test_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .test_dates
            .iter()
            .map(|date| date_to_py(py, *date))
            .collect()
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

    /// One row per test date with columns ``test_date`` (ISO string),
    /// ``projected_value``, ``threshold``, ``headroom``, ``headroom_raw``,
    /// ``breach_probability``, ``breach_probability_stderr``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = (0..self.inner.test_dates.len())
            .map(|i| {
                serde_json::json!({
                    "test_date": self.inner.test_dates[i].to_string(),
                    "projected_value": self.inner.projected_values.get(i).copied().flatten(),
                    "threshold": self.inner.thresholds.get(i).copied(),
                    "headroom": self.inner.headroom.get(i).copied().flatten(),
                    "headroom_raw": self.inner.headroom_raw.get(i).copied().flatten(),
                    "breach_probability": self.inner.breach_probability.get(i).copied(),
                    "breach_probability_stderr": self.inner.breach_probability_stderr.get(i).copied(),
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &FORECAST_COLUMNS)
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

crate::bindings::macros::wire_methods!(PyFutureBreach, FutureBreach, "FutureBreach");
crate::bindings::macros::py_getters!(PyFutureBreach |s| {
    /// Instance label of the covenant.
    covenant_id -> &str = &s.inner.covenant_id;

    /// Human-readable covenant description.
    covenant_description -> &str = &s.inner.covenant_description;

    /// Projected metric value, if finite.
    projected_value -> Option<f64> = s.inner.projected_value;

    /// Threshold in force on the breach date.
    threshold -> f64 = s.inner.threshold;

    /// Relative headroom (negative means breach), if meaningful.
    headroom -> Option<f64> = s.inner.headroom;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.covenants._forecast", "FutureBreach.headroom_raw")]
    headroom_raw -> Option<f64> = s.inner.headroom_raw;

    /// Breach probability (``1.0`` in deterministic mode).
    breach_probability -> f64 = s.inner.breach_probability;
});

#[pymethods]
impl PyFutureBreach {
    /// Forecast date of the breach.
    #[getter]
    fn breach_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.breach_date)
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
    let series = metric_series(metrics)?;
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
#[pyo3(signature = (engine, metrics, config=None), text_signature = "(engine, metrics, config=None)")]
pub(crate) fn forecast_breaches(
    py: Python<'_>,
    engine: PyRef<'_, PyCovenantEngine>,
    metrics: &Bound<'_, PyAny>,
    config: Option<PyRef<'_, PyCovenantForecastConfig>>,
) -> PyResult<Vec<PyFutureBreach>> {
    let series = metric_series(metrics)?;
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

/// Flatten a list of ``FutureBreach`` into one frame row per breach.
///
/// Columns: ``covenant_id``, ``covenant_description``, ``breach_date`` (ISO
/// string), ``projected_value``, ``threshold``, ``headroom``, ``headroom_raw``,
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
                "headroom_raw": b.inner.headroom_raw,
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
            ("headroom_raw", "float64"),
            ("breach_probability", "float64"),
        ],
    )
}
