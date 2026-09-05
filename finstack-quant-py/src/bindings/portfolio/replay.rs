//! Python binding for portfolio historical replay.

use crate::bindings::extract::extract_portfolio_ref;
use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyString};
use serde::Serialize;
use std::cell::RefCell;

thread_local! {
    /// Per-thread replay JSON scratch space.
    ///
    /// Large `serde_json::to_string` buffers are not reliably returned to the
    /// process RSS allocator between calls on macOS. Retaining only the byte
    /// capacity bounds repeated-call RSS without caching any portfolio,
    /// market, valuation, or replay-result state.
    static REPLAY_JSON_SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

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

    /// Serialize to a compact JSON string.
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let inner = &self.inner;
        py.detach(|| serde_json::to_string(inner))
            .map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(py: Python<'_>, json: &str) -> PyResult<Self> {
        let json = json.to_owned();
        let inner: finstack_quant_portfolio::replay::ReplayResult = py
            .detach(move || serde_json::from_str(&json))
            .map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "ReplayResult(steps={}, start={}, end={})",
            self.inner.steps.len(),
            self.inner.summary.start_date,
            self.inner.summary.end_date,
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json(py)?)
    }
}

/// Resolve the replay configuration from ``config`` (JSON string or dict) or
/// from the ``mode`` shorthand.
fn extract_replay_config(
    py: Python<'_>,
    config: Option<&Bound<'_, PyAny>>,
    mode: Option<&str>,
) -> PyResult<finstack_quant_portfolio::replay::ReplayConfig> {
    let json = match (config, mode) {
        (Some(config), _) => {
            crate::bindings::extract::extract_records_json(py, config, "replay config")?
        }
        (None, Some(mode)) => serde_json::json!({ "mode": mode }).to_string(),
        (None, None) => {
            return Err(crate::errors::value_error(
                "replay_portfolio requires either `config` or `mode`",
            ))
        }
    };
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
                let date = crate::bindings::date_utils::extract_date(&date)?;
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
        .map_err(display_to_py)
}

/// Run the canonical Rust replay engine for both entry points.
fn run_replay_portfolio(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    snapshots: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
    mode: Option<&str>,
) -> PyResult<finstack_quant_portfolio::replay::ReplayResult> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let config = extract_replay_config(py, config, mode)?;
    let timeline = extract_replay_timeline(py, snapshots)?;
    let finstack_config = finstack_quant_core::config::FinstackConfig::default();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    py.detach(|| {
        finstack_quant_portfolio::replay::replay_portfolio(
            portfolio_ref,
            &timeline,
            &config,
            &finstack_config,
        )
    })
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
/// config : dict | str | None
///     ``ReplayConfig`` as a dict or JSON string (``mode``,
///     ``attribution_method``, ``valuation_options``, ``on_error``).
/// mode : str | None
///     Shorthand for ``config={"mode": mode}`` when no other option is
///     needed: ``"pv_only"``, ``"pv_and_pnl"`` or ``"full_attribution"``.
///     Ignored when ``config`` is given.
///
/// Returns
/// -------
/// ReplayResult
///     Typed result with ``steps``, ``summary`` and ``skipped_dates``
///     getters plus ``to_dataframe()``. Use :func:`replay_portfolio_json`
///     for the raw wire string.
///
/// Raises
/// ------
/// ValueError
///     If neither ``config`` nor ``mode`` is supplied, or a snapshot/config
///     payload is malformed.
/// PortfolioError
///     If a snapshot fails to revalue under a strict error policy.
#[pyfunction]
#[pyo3(
    signature = (portfolio, snapshots, config=None, mode=None),
    text_signature = "(portfolio, snapshots, config=None, mode=None)"
)]
fn replay_portfolio(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    snapshots: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
    mode: Option<&str>,
) -> PyResult<PyReplayResult> {
    Ok(PyReplayResult {
        inner: run_replay_portfolio(py, portfolio, snapshots, config, mode)?,
    })
}

/// Replay a portfolio through dated market snapshots and return wire JSON.
///
/// Wire twin of :func:`replay_portfolio`; same inputs, JSON-string output.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``ReplayResult``.
#[pyfunction]
#[pyo3(
    signature = (portfolio, snapshots, config=None, mode=None),
    text_signature = "(portfolio, snapshots, config=None, mode=None)"
)]
fn replay_portfolio_json<'py>(
    py: Python<'py>,
    portfolio: &Bound<'_, PyAny>,
    snapshots: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
    mode: Option<&str>,
) -> PyResult<Bound<'py, PyString>> {
    let result = run_replay_portfolio(py, portfolio, snapshots, config, mode)?;
    py.detach(move || {
        REPLAY_JSON_SCRATCH.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            scratch.clear();
            serde_json::to_writer(&mut *scratch, &result)
        })
    })
    .map_err(display_to_py)?;

    REPLAY_JSON_SCRATCH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        let output = PyString::from_bytes(py, scratch.as_slice());
        scratch.clear();
        output
    })
}

/// Register replay functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyReplayResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(replay_portfolio, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(replay_portfolio_json, m)?)?;
    Ok(())
}
