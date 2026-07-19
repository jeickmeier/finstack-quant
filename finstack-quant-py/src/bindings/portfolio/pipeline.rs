//! End-to-end portfolio pipeline functions.
//!
//! Each function accepts either a typed :class:`Portfolio` object or a JSON
//! ``PortfolioSpec`` string, plus either a typed :class:`MarketContext` or a
//! JSON string. Returning typed wrappers (``PortfolioValuation``) lets
//! downstream calls (``aggregate_metrics``, ``portfolio_result_*``) avoid
//! a JSON round-trip.

use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::bindings::portfolio::types::{PyPortfolioCashflows, PyPortfolioValuation};
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;
use std::str::FromStr;

/// Run the shared valuation engine for JSON- and typed-return entry points.
fn run_portfolio_valuation(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    strict_risk: bool,
    metrics: Option<Vec<String>>,
) -> PyResult<finstack_quant_portfolio::valuation::PortfolioValuation> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market = extract_market_ref(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let options = finstack_quant_portfolio::valuation::PortfolioValuationOptions {
        strict_risk,
        metrics: metrics.map_or(
            finstack_quant_portfolio::valuation::RequestedMetrics::Standard,
            |metrics| {
                finstack_quant_portfolio::valuation::RequestedMetrics::Only(
                    metrics
                        .into_iter()
                        .map(
                            |metric| match finstack_quant_valuations::metrics::MetricId::from_str(
                                &metric,
                            ) {
                                Ok(metric_id) => metric_id,
                                Err(_) => {
                                    finstack_quant_valuations::metrics::MetricId::custom(metric)
                                }
                            },
                        )
                        .collect(),
                )
            },
        ),
    };
    // Release the GIL (PyO3 `detach`) while the CPU-bound Rust valuation runs
    // so other Python threads can execute concurrently. The `*Access` wrappers
    // contain a `PyRef` (not `Ungil`), so we deref to plain Rust references
    // before entering the closure — these are `Send + Sync` and therefore
    // `Ungil`. No Python state is touched inside.
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    py.detach(|| {
        finstack_quant_portfolio::valuation::value_portfolio(
            portfolio_ref,
            market_ref,
            &config,
            &options,
        )
    })
    .map_err(portfolio_to_py)
}

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
/// metrics : list[str] | None
///     Exact risk metrics to compute. ``None`` requests the standard set;
///     an empty list performs PV-only valuation.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``PortfolioValuation``. To avoid a JSON re-parse in
///     downstream calls (``aggregate_metrics``, etc.), wrap the returned
///     string once via :meth:`PortfolioValuation.from_json` and pass the
///     typed object to the next step.
#[pyfunction]
#[pyo3(signature = (portfolio, market, strict_risk=false, metrics=None))]
fn value_portfolio(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    strict_risk: bool,
    metrics: Option<Vec<String>>,
) -> PyResult<String> {
    let valuation = run_portfolio_valuation(py, portfolio, market, strict_risk, metrics)?;
    py.detach(move || serde_json::to_string(&valuation))
        .map_err(display_to_py)
}

/// Value a portfolio and return a typed result without JSON serialization.
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
/// metrics : list[str] | None
///     Exact risk metrics to compute. ``None`` requests the standard set;
///     an empty list performs PV-only valuation.
///
/// Returns
/// -------
/// PortfolioValuation
///     Typed valuation wrapper that can be passed directly to
///     ``aggregate_metrics`` without a JSON round-trip.
#[pyfunction]
#[pyo3(signature = (portfolio, market, strict_risk=false, metrics=None))]
fn value_portfolio_typed(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    strict_risk: bool,
    metrics: Option<Vec<String>>,
) -> PyResult<PyPortfolioValuation> {
    let valuation = run_portfolio_valuation(py, portfolio, market, strict_risk, metrics)?;
    Ok(PyPortfolioValuation::from_inner(valuation))
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
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market = extract_market_ref(py, market)?;
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
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenario_json = scenario_json.to_owned();
    let scenario: finstack_quant_scenarios::ScenarioSpec = py
        .detach(move || serde_json::from_str(&scenario_json))
        .map_err(display_to_py)?;
    let market = extract_market_ref(py, market)?;
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
    py.detach(move || -> Result<(String, String), serde_json::Error> {
        Ok((
            serde_json::to_string(&valuation)?,
            serde_json::to_string(&report)?,
        ))
    })
    .map_err(display_to_py)
}

/// Compute the profit and loss attributable to a scenario.
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
///     ``(pnl_json, report_json)`` — JSON for the ``ScenarioPnl`` ladder
///     (``total`` plus ``by_position``, all base-currency ``Money``) and the
///     scenario application report.
#[pyfunction]
fn scenario_pnl(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenario_json: &str,
    market: &Bound<'_, PyAny>,
) -> PyResult<(String, String)> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenario_json = scenario_json.to_owned();
    let scenario: finstack_quant_scenarios::ScenarioSpec = py
        .detach(move || serde_json::from_str(&scenario_json))
        .map_err(display_to_py)?;
    let market = extract_market_ref(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let (pnl, report) = py
        .detach(|| {
            finstack_quant_portfolio::scenarios::scenario_pnl(
                portfolio_ref,
                &scenario,
                market_ref,
                &config,
            )
        })
        .map_err(portfolio_to_py)?;
    py.detach(move || -> Result<(String, String), serde_json::Error> {
        Ok((
            serde_json::to_string(&pnl)?,
            serde_json::to_string(&report)?,
        ))
    })
    .map_err(display_to_py)
}

/// Compute ordered portfolio P&L for a batch of scenarios.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     A built :class:`Portfolio` or canonical JSON-serialized
///     ``PortfolioSpec``. The Rust batch engine values its unstressed base leg
///     once for the complete request.
/// scenarios_json : str
///     Canonical JSON array of ``ScenarioSpec`` objects, in the output order
///     required by the caller. An empty array returns ``"[]"`` without a
///     valuation.
/// market : MarketContext | str
///     The unshocked market snapshot, supplied as a typed object or canonical
///     JSON string.
///
/// Returns
/// -------
/// str
///     A canonical JSON array with one ordered object per input scenario:
///     ``{"scenario_id": ..., "pnl": ..., "report": ...}``. ``pnl`` and
///     ``report`` use the same stable JSON shapes returned separately by
///     :func:`scenario_pnl`.
///
/// Raises
/// ------
/// ValueError
///     If ``scenarios_json`` is not a JSON array of valid ``ScenarioSpec``
///     values.
/// PortfolioError
///     If scenario application, valuation, or base-currency P&L differencing
///     fails. The error identifies the earliest failing input scenario.
#[pyfunction]
fn scenario_pnl_batch(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenarios_json: &str,
    market: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenarios_json = scenarios_json.to_owned();
    let scenarios: Vec<finstack_quant_scenarios::ScenarioSpec> = py
        .detach(move || serde_json::from_str(&scenarios_json))
        .map_err(display_to_py)?;
    let market = extract_market_ref(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();

    // Extract plain Rust references before detaching: the typed access
    // wrappers own PyRefs and are not Ungil, while their Rust inners are
    // Send + Sync. Parsing happens above, so the complete batch evaluation and
    // potentially large result serialization release the GIL without touching
    // Python state.
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let result_json = py
        .detach(|| {
            finstack_quant_portfolio::scenarios::scenario_pnl_batch(
                portfolio_ref,
                &scenarios,
                market_ref,
                &config,
            )
            .map(|results| serde_json::to_string(&results))
        })
        .map_err(portfolio_to_py)?;

    result_json.map_err(display_to_py)
}

/// Register pipeline functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(value_portfolio, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(value_portfolio_typed, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(aggregate_full_cashflows, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(apply_scenario_and_revalue, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_pnl, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_pnl_batch, m)?)?;
    Ok(())
}
