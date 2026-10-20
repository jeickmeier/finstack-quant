//! Python binding for portfolio historical replay.

use crate::bindings::extract::extract_portfolio_ref;
use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;
use pyo3::types::PyAny;
use serde::Serialize;

/// Column schema for [`PyReplayResult::to_dataframe`].
const REPLAY_STEP_COLUMNS: &[ColumnSchema<'static>] = &[
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
    /// Per-step output as a list of dicts (full valuations included).
    #[getter]
    fn steps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.steps)
    }

    /// Aggregate statistics across the full replay, as a dict.
    #[getter]
    fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.summary)
    }

    /// Snapshots skipped in best-effort mode, as ``(date, reason)`` pairs.
    #[getter]
    fn skipped_dates<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.skipped_dates)
    }

    /// Per-step value and P&L ladder as a :class:`pandas.DataFrame`.
    ///
    /// One row per replay step; ``daily_mtm_pnl`` and ``cumulative_mtm_pnl`` exclude paid cashflows and are
    /// null at step 0. Full per-step valuations remain available from the
    /// ``steps`` getter.
    ///
    /// Columns: ``date``, ``value``, ``daily_mtm_pnl``, ``cumulative_mtm_pnl``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.rows(), REPLAY_STEP_COLUMNS)
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

/// Parse the replay configuration from ``config`` (JSON string or dict).
fn extract_replay_config(
    py: Python<'_>,
    config: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_portfolio::replay::ReplayConfig> {
    let json = crate::bindings::extract::extract_records_json(py, config, "replay config")?;
    py.detach(move || serde_json::from_str(&json))
        .map_err(display_to_py)
}

/// Resolve the snapshot timeline from a JSON array string, a list of
/// ``{"date", "market"}`` dicts, or a list of ``(date, MarketContext | str)``
/// tuples.
fn extract_replay_timeline(
    py: Python<'_>,
    snapshots: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_portfolio::replay::ReplayTimeline> {
    let json = if let Ok(json) = snapshots.extract::<String>() {
        json
    } else {
        let mut entries: Vec<serde_json::Value> = Vec::new();
        for item in snapshots.try_iter()? {
            let item = item?;
            if let Ok((date, market)) = item.extract::<(Bound<'_, PyAny>, Bound<'_, PyAny>)>() {
                let date = crate::bindings::date_utils::py_to_date(&date)?;
                let market = crate::bindings::extract::extract_market(py, &market)?;
                entries.push(serde_json::json!({
                    "date": date.to_string(),
                    "market": serde_json::to_value(&market).map_err(display_to_py)?,
                }));
            } else {
                entries.push(crate::bindings::module_utils::py_to_json_value(
                    py,
                    &item,
                    "replay snapshot",
                )?);
            }
        }
        serde_json::Value::Array(entries).to_string()
    };
    py.detach(move || finstack_quant_portfolio::replay::ReplayTimeline::from_json_snapshots(&json))
        .map_err(crate::errors::portfolio_to_py)
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
///     getters plus ``to_dataframe()``; ``to_json()`` gives the wire form.
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
    m.add_class::<PyReplayResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(replay_portfolio, m)?)?;
    Ok(())
}
