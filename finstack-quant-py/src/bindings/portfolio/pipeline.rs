//! End-to-end portfolio pipeline functions.
//!
//! Each function accepts either a typed :class:`Portfolio` object or a JSON
//! ``PortfolioSpec`` string, plus either a typed :class:`MarketContext` or a
//! JSON string. Returning typed wrappers (``PortfolioValuation``) lets
//! downstream calls (``aggregate_metrics``, ``portfolio_result_*``) avoid
//! a JSON round-trip.

use crate::bindings::core::currency::extract_currency;
use crate::bindings::extract::{
    extract_cashflows_ref, extract_market_ref, extract_portfolio_ref, extract_scenario_spec,
    extract_scenario_specs,
};
use crate::bindings::portfolio::scenario_pnl::PyScenarioPnl;
use crate::bindings::portfolio::types::{PyPortfolioCashflows, PyPortfolioValuation};
use crate::bindings::scenarios::engine::PyApplicationReport;
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;

/// Strictly parse user-supplied metric names into [`RequestedMetrics`].
///
/// Delegates to the canonical
/// `RequestedMetrics::try_from_metric_names` in the portfolio crate (shared
/// with the WASM binding), which rejects unknown standard-metric names —
/// surfaced here as a `ValueError` listing the available identifiers —
/// instead of letting a typo silently degrade to PV-only valuation.
fn parse_requested_metrics(
    metrics: Option<Vec<String>>,
) -> PyResult<finstack_quant_portfolio::valuation::RequestedMetrics> {
    finstack_quant_portfolio::valuation::RequestedMetrics::try_from_metric_names(metrics)
        .map_err(crate::errors::core_to_py)
}

/// Run the shared valuation engine for the typed Python entry point.
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
        metrics: parse_requested_metrics(metrics)?,
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
///     If ``True`` (the default), any risk metric failure aborts the entire
///     valuation. Set ``False`` only for an intentional PV-preserving
///     fallback that records failed metrics as diagnostics.
/// metrics : list[str] | None
///     Exact risk metrics to compute. ``None`` requests the standard set
///     (PV plus ``dv01``, which every rate-sensitive pricer supports);
///     an empty list performs PV-only valuation. Pricer-specific metrics
///     such as ``theta``, ``cs01`` or the Greeks must be listed explicitly.
///     Names are validated strictly against the standard ``MetricId`` set;
///     an unknown name raises ``ValueError`` listing the available metrics.
///
/// Returns
/// -------
/// PortfolioValuation
///     Typed valuation wrapper that can be passed directly to
///     ``aggregate_metrics`` without a JSON round-trip.
#[pyfunction]
#[pyo3(signature = (portfolio, market, strict_risk=true, metrics=None))]
fn value_portfolio(
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
///     A :class:`Portfolio` object (fast path, no rebuild) or a
///     JSON-serialized ``PortfolioSpec`` string.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string used to build
///     instrument schedules.
/// allow_partial : bool
///     If ``False`` (the default), any schedule-construction issue aborts
///     the call. If ``True``, remaining positions still contribute to the
///     ladder and issues are returned on the result.
///
/// Returns
/// -------
/// PortfolioCashflows
///     Typed wrapper around the full cashflow ladder. Use
///     ``to_json()``/``from_json()`` for round-tripping and typed accessors
///     (``events_json``, ``by_date_json``, ``collapse_to_base_by_date_kind``)
///     to drill in without re-parsing.
///
/// Raises
/// ------
/// PortfolioError
///     If any position fails schedule construction while
///     ``allow_partial`` is ``False``, or same-date same-currency
///     same-kind amounts cannot be added.
#[pyfunction]
#[pyo3(signature = (portfolio, market, allow_partial=false))]
fn aggregate_full_cashflows(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    allow_partial: bool,
) -> PyResult<PyPortfolioCashflows> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market = extract_market_ref(py, market)?;
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let options = finstack_quant_portfolio::cashflows::CashflowAggregationOptions { allow_partial };
    let cashflows = py
        .detach(|| {
            finstack_quant_portfolio::cashflows::aggregate_full_cashflows(
                portfolio_ref,
                market_ref,
                &options,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioCashflows::from_inner(cashflows))
}

/// Net same-currency cashflow amounts across kinds for each payment date.
///
/// Parameters
/// ----------
/// cashflows : PortfolioCashflows | str
///     A :class:`PortfolioCashflows` ladder (fast path, no parse), full
///     cashflow-ladder JSON, or a ``{date: {ccy: {kind: money}}}`` object
///     (optionally wrapped as ``{"by_date": ...}``). Kind keys are opaque
///     strings; money objects require a finite decimal-string ``amount`` and
///     a ``currency`` matching their enclosing bucket. Numeric JSON amounts
///     are rejected by the canonical Money wire contract.
///     Every ISO payment date and currency bucket is validated, including
///     currencies other than the requested output currency.
/// currency : Currency | str
///     ISO-4217 code selecting which per-date currency bucket to net.
///
/// Returns
/// -------
/// list[tuple[str, float]]
///     ``(ISO date, net amount)`` pairs sorted by date. Dates with no flows
///     in ``currency`` are omitted.
///
/// Raises
/// ------
/// TypeError
///     If ``cashflows`` is neither a ``PortfolioCashflows`` nor a string.
/// ValueError
///     If the requested ``currency`` is unknown.
/// PortfolioError
///     If cashflow JSON is malformed, a date or currency bucket is invalid,
///     a container is not an object, a money amount is malformed/non-finite,
///     a money currency differs from its bucket, or a bucket total overflows.
///
/// Examples
/// --------
/// >>> from finstack_quant.portfolio import net_in_currency_by_date
/// >>> net_in_currency_by_date(
/// ...     '{"by_date":{"2025-01-15":{"USD":{"notional":{"amount":"-100","currency":"USD"}}}}}',
/// ...     "USD",
/// ... )
/// [('2025-01-15', -100.0)]
#[pyfunction]
#[pyo3(text_signature = "(cashflows, currency)")]
fn net_in_currency_by_date(
    py: Python<'_>,
    cashflows: &Bound<'_, PyAny>,
    currency: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, f64)>> {
    let ccy = extract_currency(currency)?;
    if let Ok(cashflows_json) = cashflows.extract::<String>() {
        let code = ccy.to_string();
        return py
            .detach(move || {
                finstack_quant_portfolio::cashflows::net_in_currency_by_date_json(
                    &cashflows_json,
                    &code,
                )
            })
            .map_err(portfolio_to_py);
    }
    let ladder = extract_cashflows_ref(py, cashflows)?;
    Ok(ladder
        .net_in_currency_by_date(ccy)
        .into_iter()
        .map(|(date, amount)| (date.to_string(), amount))
        .collect())
}

/// Apply a scenario to a portfolio and revalue it.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     Built portfolio or canonical ``PortfolioSpec`` JSON.
/// scenario : ScenarioSpec | str
///     Typed :class:`~finstack_quant.scenarios.ScenarioSpec` or its canonical
///     JSON string.
/// market : MarketContext | str
///     Base market to shock before revaluation.
///
/// Returns
/// -------
/// tuple[PortfolioValuation, ApplicationReport]
///     The revalued portfolio and the scenario application report. Call
///     ``.to_json()`` on either for its wire form.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, scenario, market)")]
fn apply_scenario_and_revalue(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenario: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<(PyPortfolioValuation, PyApplicationReport)> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenario = extract_scenario_spec(py, scenario)?;
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
    Ok((
        PyPortfolioValuation::from_inner(valuation),
        PyApplicationReport { inner: report },
    ))
}

/// Compute the profit and loss attributable to a scenario.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     Built portfolio or canonical ``PortfolioSpec`` JSON.
/// scenario : ScenarioSpec | str
///     Typed :class:`~finstack_quant.scenarios.ScenarioSpec` or its canonical
///     JSON string.
/// market : MarketContext | str
///     Unshocked base market.
///
/// Returns
/// -------
/// tuple[ScenarioPnl, ApplicationReport]
///     The P&L ladder (``total`` plus ``by_position``, all base-currency
///     floats; ``ScenarioPnl.currency`` names the currency) and the scenario
///     application report. ``ScenarioPnl`` offers ``to_dataframe()`` and
///     ``to_series()``; call ``.to_json()`` on either for its wire form.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, scenario, market)")]
fn scenario_pnl(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenario: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<(PyScenarioPnl, PyApplicationReport)> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenario = extract_scenario_spec(py, scenario)?;
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
    Ok((
        PyScenarioPnl { inner: pnl },
        PyApplicationReport { inner: report },
    ))
}

/// Run the canonical Rust batch engine for both batch entry points.
fn run_scenario_pnl_batch(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenarios: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<Vec<finstack_quant_portfolio::scenarios::ScenarioPnlBatchItem>> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenarios = extract_scenario_specs(py, scenarios)?;
    let market = extract_market_ref(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();

    // Extract plain Rust references before detaching: the typed access
    // wrappers own PyRefs and are not Ungil, while their Rust inners are
    // Send + Sync. Parsing happens above, so the complete batch evaluation
    // releases the GIL without touching Python state.
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    py.detach(|| {
        finstack_quant_portfolio::scenarios::scenario_pnl_batch(
            portfolio_ref,
            &scenarios,
            market_ref,
            &config,
        )
    })
    .map_err(portfolio_to_py)
}

/// Compute ordered portfolio P&L for a batch of scenarios.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     A built :class:`Portfolio` or canonical JSON-serialized
///     ``PortfolioSpec``. The Rust batch engine values its unstressed base leg
///     once for the complete request.
/// scenarios : list[ScenarioSpec | str] | str
///     Ordered scenarios: a Python sequence whose items are typed
///     ``ScenarioSpec`` objects or canonical JSON strings, or one canonical
///     JSON array string. Output order matches input order. An empty batch
///     returns ``[]`` without a valuation.
/// market : MarketContext | str
///     The unshocked market snapshot, supplied as a typed object or canonical
///     JSON string.
///
/// Returns
/// -------
/// list[ScenarioPnlBatchItem]
///     One ordered item per input scenario, each carrying ``scenario_id``,
///     a typed ``pnl`` (:class:`ScenarioPnl`) and ``report``
///     (:class:`~finstack_quant.scenarios.ApplicationReport`). Use
///     :func:`scenario_pnl_batch_json` for the raw wire string.
///
/// Raises
/// ------
/// ValueError
///     If ``scenarios`` contains JSON that is not a valid ``ScenarioSpec``.
/// PortfolioError
///     If scenario application, valuation, or base-currency P&L differencing
///     fails. The error identifies the earliest failing input scenario.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, scenarios, market)")]
fn scenario_pnl_batch(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenarios: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<Vec<crate::bindings::portfolio::scenario_pnl::PyScenarioPnlBatchItem>> {
    let results = run_scenario_pnl_batch(py, portfolio, scenarios, market)?;
    Ok(results
        .into_iter()
        .map(|inner| crate::bindings::portfolio::scenario_pnl::PyScenarioPnlBatchItem { inner })
        .collect())
}

/// Compute ordered batch scenario P&L and return wire JSON.
///
/// Wire twin of :func:`scenario_pnl_batch`; same inputs, JSON-string output.
///
/// Returns
/// -------
/// str
///     A canonical JSON array with one ordered object per input scenario:
///     ``{"scenario_id": ..., "pnl": ..., "report": ...}``. ``pnl`` and
///     ``report`` use the same stable JSON shapes returned separately by
///     :func:`scenario_pnl`. An empty scenario array returns ``"[]"``.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, scenarios, market)")]
fn scenario_pnl_batch_json(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenarios: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let results = run_scenario_pnl_batch(py, portfolio, scenarios, market)?;
    py.detach(move || serde_json::to_string(&results))
        .map_err(display_to_py)
}

/// Register pipeline functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(value_portfolio, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(aggregate_full_cashflows, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(net_in_currency_by_date, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(apply_scenario_and_revalue, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_pnl, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_pnl_batch, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_pnl_batch_json, m)?)?;
    Ok(())
}
