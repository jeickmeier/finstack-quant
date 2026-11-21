//! Python binding for portfolio historical replay.

use super::attribution::PyPortfolioAttribution;
use super::valuation::PyPortfolioValuation;
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::extract::extract_portfolio_ref;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::{display_to_py, portfolio_to_py};
use finstack_quant_portfolio::replay::{ReplayConfig, ReplayStep, ReplaySummary};
use pyo3::prelude::*;
use pyo3::types::PyAny;
use serde::Serialize;

/// Column schema for [`PyReplayResult::to_arrow_table`].
const REPLAY_STEP_COLUMNS: &[(&str, &str)] = &[
    ("date", "str"),
    ("value", "float64"),
    ("daily_mtm_pnl", "float64"),
    ("cumulative_mtm_pnl", "float64"),
];

/// One row of the replay step ladder.
#[derive(Serialize)]
struct ReplayStepRow {
    date: String,
    value: f64,
    daily_mtm_pnl: Option<f64>,
    cumulative_mtm_pnl: Option<f64>,
}

#[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayConfig")]
#[pyclass(name = "ReplayConfig", module = "finstack_quant.portfolio", frozen)]
pub(crate) struct PyReplayConfig {
    inner: ReplayConfig,
}

#[pymethods]
impl PyReplayConfig {
    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayConfig.__init__")]
    #[new]
    #[pyo3(signature = (mode, *, attribution_method="parallel", valuation_options=None, on_error="strict"))]
    fn new(
        mode: &str,
        attribution_method: &str,
        valuation_options: Option<&Bound<'_, PyAny>>,
        on_error: &str,
    ) -> PyResult<Self> {
        use crate::bindings::extract::parse_label;

        let valuation_options = match valuation_options {
            Some(options) => crate::bindings::module_utils::spec_from_py(
                options.py(),
                options,
                "valuation_options",
            )?,
            None => Default::default(),
        };
        Ok(Self {
            inner: ReplayConfig {
                mode: parse_label(mode)?,
                attribution_method: parse_label(attribution_method)?,
                valuation_options,
                on_error: parse_label(on_error)?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayConfig.mode")]
    #[getter]
    fn mode(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.mode).map_err(crate::errors::core_to_py)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayConfig.attribution_method")]
    #[getter]
    fn attribution_method(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.attribution_method)
            .map_err(crate::errors::core_to_py)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayConfig.valuation_options")]
    #[getter]
    fn valuation_options<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.valuation_options)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayConfig.on_error")]
    #[getter]
    fn on_error(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.on_error)
            .map_err(crate::errors::core_to_py)
    }
}

#[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayStep")]
#[pyclass(name = "ReplayStep", module = "finstack_quant.portfolio", frozen)]
pub(crate) struct PyReplayStep {
    inner: ReplayStep,
}

#[pymethods]
impl PyReplayStep {
    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayStep.date")]
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayStep.valuation")]
    #[getter]
    fn valuation(&self) -> PyPortfolioValuation {
        PyPortfolioValuation::from_inner(self.inner.valuation.clone())
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayStep.daily_mtm_pnl")]
    #[getter]
    fn daily_mtm_pnl(&self) -> Option<PyMoney> {
        self.inner.daily_mtm_pnl.map(PyMoney::from_inner)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayStep.cumulative_mtm_pnl")]
    #[getter]
    fn cumulative_mtm_pnl(&self) -> Option<PyMoney> {
        self.inner.cumulative_mtm_pnl.map(PyMoney::from_inner)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayStep.attribution")]
    #[getter]
    fn attribution(&self) -> Option<PyPortfolioAttribution> {
        self.inner
            .attribution
            .clone()
            .map(PyPortfolioAttribution::from_inner)
    }
}

#[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary")]
#[pyclass(name = "ReplaySummary", module = "finstack_quant.portfolio", frozen)]
pub(crate) struct PyReplaySummary {
    inner: ReplaySummary,
}

#[pymethods]
impl PyReplaySummary {
    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.end_date")]
    #[getter]
    fn end_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end_date)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.num_steps")]
    #[getter]
    fn num_steps(&self) -> usize {
        self.inner.num_steps
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.start_value")]
    #[getter]
    fn start_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.start_value)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.end_value")]
    #[getter]
    fn end_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.end_value)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.total_mtm_pnl")]
    #[getter]
    fn total_mtm_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_mtm_pnl)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.max_mtm_drawdown")]
    #[getter]
    fn max_mtm_drawdown(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.max_mtm_drawdown)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.max_mtm_drawdown_pct")]
    #[getter]
    fn max_mtm_drawdown_pct(&self) -> f64 {
        self.inner.max_mtm_drawdown_pct
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.max_mtm_drawdown_peak_date")]
    #[getter]
    fn max_mtm_drawdown_peak_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.max_mtm_drawdown_peak_date)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.max_mtm_drawdown_trough_date")]
    #[getter]
    fn max_mtm_drawdown_trough_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.max_mtm_drawdown_trough_date)
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.max_mtm_drawdown_pct_peak_date")]
    #[getter]
    fn max_mtm_drawdown_pct_peak_date<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .max_mtm_drawdown_pct_peak_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplaySummary.max_mtm_drawdown_pct_trough_date")]
    #[getter]
    fn max_mtm_drawdown_pct_trough_date<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .max_mtm_drawdown_pct_trough_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }
}

/// Full output of a historical replay run.
///
/// Returned by :func:`replay_portfolio`.
#[pyclass(
    name = "ReplayResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyReplayResult {
    pub(crate) inner: finstack_quant_portfolio::replay::ReplayResult,
}

crate::bindings::macros::wire_methods!(
    PyReplayResult,
    finstack_quant_portfolio::replay::ReplayResult,
    "ReplayResult"
);

impl PyReplayResult {
    fn rows(&self) -> Vec<ReplayStepRow> {
        self.inner
            .steps
            .iter()
            .map(|step| ReplayStepRow {
                date: step.date.to_string(),
                value: step.valuation.total_base_currency.amount(),
                daily_mtm_pnl: step.daily_mtm_pnl.map(|m| m.amount()),
                cumulative_mtm_pnl: step.cumulative_mtm_pnl.map(|m| m.amount()),
            })
            .collect()
    }
}

#[pymethods]
impl PyReplayResult {
    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayResult.steps")]
    #[getter]
    fn steps(&self) -> Vec<PyReplayStep> {
        self.inner
            .steps
            .iter()
            .cloned()
            .map(|inner| PyReplayStep { inner })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.portfolio._replay", "ReplayResult.summary")]
    #[getter]
    fn summary(&self) -> PyReplaySummary {
        PyReplaySummary {
            inner: self.inner.summary.clone(),
        }
    }

    /// Snapshots skipped in best-effort mode, as ``(date, reason)`` pairs.
    #[getter]
    fn skipped_dates<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.skipped_dates)
    }

    fn __repr__(&self) -> String {
        format!(
            "ReplayResult(steps={}, start={}, end={})",
            self.inner.steps.len(),
            self.inner.summary.start_date,
            self.inner.summary.end_date,
        )
    }
}

impl PyReplayResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.rows(), REPLAY_STEP_COLUMNS)
    }
}

tabular_methods!(PyReplayResult);

/// Parse the replay configuration from ``config`` (JSON string or dict).
fn extract_replay_config(
    py: Python<'_>,
    config: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_portfolio::replay::ReplayConfig> {
    if let Ok(config) = config.extract::<PyRef<'_, PyReplayConfig>>() {
        return Ok(config.inner.clone());
    }
    let json = crate::bindings::extract::extract_records_json(py, config, "replay config")?;
    py.detach(move || serde_json::from_str(&json))
        .map_err(display_to_py)
}

/// Resolve the snapshot timeline from a JSON array string, a list of
/// ``{"date", "market"}`` dicts, or a list of ``(date, MarketContext | str)``
/// tuples.
///
/// Tuples are converted to typed `(Date, MarketContext)` pairs and passed to
/// `ReplayTimeline::new`; JSON text and dicts go through the Rust
/// `ReplayTimeline::from_json_snapshots` wire parser.
fn extract_replay_timeline(
    py: Python<'_>,
    snapshots: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_portfolio::replay::ReplayTimeline> {
    use finstack_quant_portfolio::replay::ReplayTimeline;

    if let Ok(json) = snapshots.extract::<String>() {
        return py
            .detach(move || ReplayTimeline::from_json_snapshots(&json))
            .map_err(portfolio_to_py);
    }
    let items: Vec<Bound<'_, PyAny>> = snapshots.try_iter()?.collect::<PyResult<_>>()?;
    let is_tuple = |item: &Bound<'_, PyAny>| {
        item.extract::<(Bound<'_, PyAny>, Bound<'_, PyAny>)>()
            .is_ok()
    };
    if items.first().is_some_and(is_tuple) {
        let mut pairs = Vec::with_capacity(items.len());
        for item in &items {
            let (date, market) = item
                .extract::<(Bound<'_, PyAny>, Bound<'_, PyAny>)>()
                .map_err(|_| {
                    crate::errors::value_error(
                        "replay snapshots must be all (date, market) tuples or all dicts",
                    )
                })?;
            let date = crate::bindings::date_utils::py_to_date(&date)?;
            let market = crate::bindings::extract::extract_market(py, &market)?;
            pairs.push((date, market));
        }
        return ReplayTimeline::new(pairs).map_err(portfolio_to_py);
    }
    let entries = items
        .iter()
        .map(|item| crate::bindings::module_utils::py_to_json_value(py, item, "replay snapshot"))
        .collect::<PyResult<Vec<_>>>()?;
    let json = serde_json::Value::Array(entries).to_string();
    py.detach(move || ReplayTimeline::from_json_snapshots(&json))
        .map_err(portfolio_to_py)
}

/// Replay a portfolio through dated market snapshots.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     A :class:`Portfolio` object (fast path) or a JSON-serialized
///     ``PortfolioSpec`` string.
/// snapshots : list[tuple[date, MarketContext | str]] | list[dict] | str
///     Dated market snapshots in ascending date order: ``(date, market)``
///     pairs (dates as ``datetime.date`` or ISO strings), JSON-shaped
///     ``{"date": "YYYY-MM-DD", "market": {...}}`` dicts, or the canonical
///     JSON array string.
/// config : dict | str
///     ``ReplayConfig`` as a dict or JSON string: required ``mode``
///     (``"pv_only"``, ``"pv_and_pnl"`` or ``"full_attribution"``) and
///     optional ``attribution_method``, ``valuation_options`` and
///     ``on_error``; unknown keys are rejected. The same input the WASM
///     ``replayPortfolio`` takes.
///
/// Returns
/// -------
/// ReplayResult
///     Typed result with ``steps``, ``summary`` and ``skipped_dates``
///     getters plus ``to_pandas()`` / ``to_polars()`` / ``to_arrow()``; ``to_json()`` gives the wire form.
///
/// Raises
/// ------
/// ValueError
///     If a snapshot or config payload is malformed (including a missing
///     ``mode``).
/// PortfolioError
///     If a snapshot fails to revalue under a strict error policy.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, snapshots, config)")]
fn replay_portfolio(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    snapshots: &Bound<'_, PyAny>,
    config: &Bound<'_, PyAny>,
) -> PyResult<PyReplayResult> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let config = extract_replay_config(py, config)?;
    let timeline = extract_replay_timeline(py, snapshots)?;
    let finstack_config = finstack_quant_core::config::FinstackConfig::default();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let inner = py
        .detach(|| {
            finstack_quant_portfolio::replay::replay_portfolio(
                portfolio_ref,
                &timeline,
                &config,
                &finstack_config,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyReplayResult { inner })
}

/// Register replay functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyReplayConfig>()?;
    m.add_class::<PyReplayResult>()?;
    m.add_class::<PyReplayStep>()?;
    m.add_class::<PyReplaySummary>()?;
    m.add_function(pyo3::wrap_pyfunction!(replay_portfolio, m)?)?;
    Ok(())
}
