//! Python binding for portfolio historical replay.

use crate::bindings::extract::extract_portfolio_ref;
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;

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
fn replay_portfolio(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    snapshots_json: &str,
    config_json: &str,
) -> PyResult<String> {
    let portfolio = extract_portfolio_ref(portfolio)?;
    let config: finstack_quant_portfolio::replay::ReplayConfig =
        serde_json::from_str(config_json).map_err(display_to_py)?;
    let timeline =
        finstack_quant_portfolio::replay::ReplayTimeline::from_json_snapshots(snapshots_json)
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
    serde_json::to_string(&result).map_err(display_to_py)
}

/// Register replay functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(replay_portfolio, m)?)?;
    Ok(())
}
