//! Python binding for portfolio historical replay.

use crate::bindings::extract::extract_portfolio_ref;
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;
use pyo3::types::PyString;
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

/// Replay a portfolio through dated market snapshots.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     A :class:`Portfolio` object (fast path) or a JSON-serialized
///     ``PortfolioSpec`` string.
/// snapshots_json : str
///     JSON array of ``{"date": "YYYY-MM-DD", "market": {...}}`` objects.
///     Markets use the standard ``MarketContextState`` JSON format.
/// config_json : str
///     JSON-serialized ``ReplayConfig``.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``ReplayResult``.
#[pyfunction]
fn replay_portfolio<'py>(
    py: Python<'py>,
    portfolio: &Bound<'_, PyAny>,
    snapshots_json: &str,
    config_json: &str,
) -> PyResult<Bound<'py, PyString>> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let config_json = config_json.to_owned();
    let config: finstack_quant_portfolio::replay::ReplayConfig = py
        .detach(move || serde_json::from_str(&config_json))
        .map_err(display_to_py)?;
    let snapshots_json = snapshots_json.to_owned();
    let timeline = py
        .detach(move || {
            finstack_quant_portfolio::replay::ReplayTimeline::from_json_snapshots(&snapshots_json)
        })
        .map_err(display_to_py)?;
    let finstack_config = finstack_quant_core::config::FinstackConfig::default();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let result = py
        .detach(|| {
            finstack_quant_portfolio::replay::replay_portfolio(
                portfolio_ref,
                &timeline,
                &config,
                &finstack_config,
            )
        })
        .map_err(portfolio_to_py)?;
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
    m.add_function(pyo3::wrap_pyfunction!(replay_portfolio, m)?)?;
    Ok(())
}
