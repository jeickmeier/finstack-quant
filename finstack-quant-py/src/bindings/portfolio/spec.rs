//! JSON round-trip helpers for portfolio specs and results.
//!
//! These entry points retain the historical JSON-only API for compatibility;
//! prefer the typed :class:`Portfolio`, :class:`PortfolioValuation`, and
//! :class:`PortfolioResult` classes (see ``types.rs``) and the pipeline
//! functions, which skip the JSON round-trip entirely.

use crate::bindings::extract::{
    extract_market_ref, extract_portfolio_result_ref, extract_valuation_ref,
};
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;

/// Parse a portfolio specification from JSON and return the canonical form.
#[pyfunction]
pub fn parse_portfolio_spec(py: Python<'_>, json_str: &str) -> PyResult<String> {
    let json_str = json_str.to_owned();
    py.detach(move || {
        let spec: finstack_quant_portfolio::portfolio::PortfolioSpec =
            serde_json::from_str(&json_str)?;
        serde_json::to_string(&spec)
    })
    .map_err(display_to_py)
}

/// Build a runtime portfolio from a JSON spec and round-trip the spec.
///
/// Returns the JSON form after `Portfolio::from_spec` → `Portfolio::to_spec`.
/// Prefer :meth:`Portfolio.from_spec` for real work — it returns the typed
/// object that pipeline functions reuse without rebuilding.
#[pyfunction]
pub fn build_portfolio_from_spec(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    let spec_json = spec_json.to_owned();
    let spec: finstack_quant_portfolio::portfolio::PortfolioSpec = py
        .detach(move || serde_json::from_str(&spec_json))
        .map_err(display_to_py)?;
    let portfolio = py
        .detach(move || finstack_quant_portfolio::Portfolio::from_spec(spec))
        .map_err(portfolio_to_py)?;
    py.detach(move || serde_json::to_string(&portfolio.to_spec()))
        .map_err(display_to_py)
}

/// Extract total portfolio value from a ``PortfolioResult``.
///
/// Accepts either a :class:`PortfolioResult` object (no parse) or a JSON
/// string. The typed path is O(1); the JSON path parses the full result.
#[pyfunction]
pub fn portfolio_result_total_value(py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<f64> {
    let result = extract_portfolio_result_ref(py, result)?;
    Ok(result.total_value().amount())
}

/// Extract a specific metric from a ``PortfolioResult``.
///
/// Accepts either a :class:`PortfolioResult` object (no parse) or a JSON
/// string.
#[pyfunction]
pub fn portfolio_result_get_metric(
    py: Python<'_>,
    result: &Bound<'_, PyAny>,
    metric_id: &str,
) -> PyResult<Option<f64>> {
    let result = extract_portfolio_result_ref(py, result)?;
    Ok(result.get_metric(metric_id))
}

/// Aggregate portfolio metrics from a valuation.
///
/// Parameters
/// ----------
/// valuation : PortfolioValuation | str
///     A :class:`PortfolioValuation` object (fast path) or JSON string.
/// base_ccy : str
///     Base currency code.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : str
///     Valuation date in ISO 8601 format.
#[pyfunction]
pub fn aggregate_metrics(
    py: Python<'_>,
    valuation: &Bound<'_, PyAny>,
    base_ccy: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
) -> PyResult<String> {
    let valuation = extract_valuation_ref(py, valuation)?;
    let ccy: finstack_quant_core::currency::Currency = base_ccy.parse().map_err(display_to_py)?;
    let market = extract_market_ref(py, market)?;
    let date = super::parse_date(as_of)?;
    let valuation_ref: &finstack_quant_portfolio::valuation::PortfolioValuation = &valuation;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let metrics = py
        .detach(|| {
            finstack_quant_portfolio::metrics::aggregate_metrics(
                valuation_ref,
                ccy,
                market_ref,
                date,
            )
        })
        .map_err(portfolio_to_py)?;
    py.detach(move || serde_json::to_string(&metrics))
        .map_err(display_to_py)
}

/// Register spec functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(parse_portfolio_spec, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(build_portfolio_from_spec, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(portfolio_result_total_value, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(portfolio_result_get_metric, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(aggregate_metrics, m)?)?;
    Ok(())
}
