//! End-to-end portfolio pipeline functions.
//!
//! Each function accepts either a typed :class:`Portfolio` object or a JSON
//! ``PortfolioSpec`` string, plus either a typed :class:`MarketContext` or a
//! JSON string. Returning typed wrappers (``PortfolioValuation``) lets
//! downstream calls (``aggregate_metrics``, ``portfolio_result_*``) avoid
//! a JSON round-trip.

use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::bindings::portfolio::types::PyPortfolioCashflows;
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;

/// Value a portfolio.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     A :class:`Portfolio` object (fast path, no rebuild) or a
///     JSON-serialized ``PortfolioSpec`` string.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// strict_risk : bool
///     If ``True``, any risk metric failure aborts the entire valuation.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``PortfolioValuation``. To avoid a JSON re-parse in
///     downstream calls (``aggregate_metrics``, etc.), wrap the returned
///     string once via :meth:`PortfolioValuation.from_json` and pass the
///     typed object to the next step.
#[pyfunction]
#[pyo3(signature = (portfolio, market, strict_risk=false))]
fn value_portfolio(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    strict_risk: bool,
) -> PyResult<String> {
    let portfolio = extract_portfolio_ref(portfolio)?;
    let market = extract_market_ref(market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let options = finstack_quant_portfolio::valuation::PortfolioValuationOptions {
        strict_risk,
        ..Default::default()
    };
    // Release the GIL (PyO3 `detach`) while the CPU-bound Rust valuation runs
    // so other Python threads can execute concurrently. The `*Access` wrappers
    // contain a `PyRef` (not `Ungil`), so we deref to plain Rust references
    // before entering the closure — these are `Send + Sync` and therefore
    // `Ungil`. No Python state is touched inside.
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let valuation = py
        .detach(|| {
            finstack_quant_portfolio::valuation::value_portfolio(
                portfolio_ref,
                market_ref,
                &config,
                &options,
            )
        })
        .map_err(portfolio_to_py)?;
    serde_json::to_string(&valuation).map_err(display_to_py)
}

/// Aggregate the full classified cashflow ladder.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
/// market : MarketContext | str
///
/// Returns
/// -------
/// PortfolioCashflows
///     Typed wrapper around the full cashflow ladder. Use
///     ``to_json()``/``from_json()`` for round-tripping and typed accessors
///     (``events_json``, ``by_date_json``, ``collapse_to_base_by_date_kind``)
///     to drill in without re-parsing.
#[pyfunction]
fn aggregate_full_cashflows(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<PyPortfolioCashflows> {
    let portfolio = extract_portfolio_ref(portfolio)?;
    let market = extract_market_ref(market)?;
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let cashflows = py
        .detach(|| {
            finstack_quant_portfolio::cashflows::aggregate_full_cashflows(portfolio_ref, market_ref)
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioCashflows::from_inner(cashflows))
}

/// Apply a scenario to a portfolio and revalue it.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
/// scenario_json : str
///     JSON-serialized ``ScenarioSpec``.
/// market : MarketContext | str
///
/// Returns
/// -------
/// tuple[str, str]
///     ``(valuation_json, report_json)`` — JSON for the revalued portfolio
///     and the scenario application report.
#[pyfunction]
fn apply_scenario_and_revalue(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenario_json: &str,
    market: &Bound<'_, PyAny>,
) -> PyResult<(String, String)> {
    let portfolio = extract_portfolio_ref(portfolio)?;
    let scenario: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(display_to_py)?;
    let market = extract_market_ref(market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let (valuation, report) = py
        .detach(|| {
            finstack_quant_portfolio::scenarios::apply_and_revalue(
                portfolio_ref,
                &scenario,
                market_ref,
                &config,
            )
        })
        .map_err(portfolio_to_py)?;
    let val_json = serde_json::to_string(&valuation).map_err(display_to_py)?;
    let report_json = serde_json::to_string(&report).map_err(display_to_py)?;
    Ok((val_json, report_json))
}

/// Register pipeline functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(value_portfolio, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(aggregate_full_cashflows, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(apply_scenario_and_revalue, m)?)?;
    Ok(())
}
