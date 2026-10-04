//! JSON round-trip helpers for portfolio specs and results.
//!
//! These entry points complement the typed :class:`Portfolio`,
//! :class:`PortfolioValuation`, and :class:`PortfolioResult` classes (see
//! ``types.rs``). Pipeline functions use the typed objects directly and skip
//! JSON round-trips; scalar reads of a result go through the
//! ``PortfolioResult`` methods (``total_value``, ``get_metric``).

use crate::bindings::core::currency::extract_currency;
use crate::bindings::extract::{extract_market_ref, extract_valuation_ref};
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;

/// Parse a portfolio specification from JSON and return the canonical form.
#[pyfunction]
pub fn parse_portfolio_spec_json(py: Python<'_>, json_str: &str) -> PyResult<String> {
    let json_str = json_str.to_owned();
    py.detach(move || {
        let spec: finstack_quant_portfolio::portfolio::PortfolioSpec =
            serde_json::from_str(&json_str)?;
        serde_json::to_string(&spec)
    })
    .map_err(display_to_py)
}
/// Run canonical Rust metric aggregation after extracting host inputs.
fn run_aggregate_metrics(
    py: Python<'_>,
    valuation: &Bound<'_, PyAny>,
    base_currency: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_portfolio::metrics::PortfolioMetrics> {
    let valuation = extract_valuation_ref(py, valuation)?;
    let ccy = extract_currency(base_currency)?;
    let market = extract_market_ref(py, market)?;
    let date = crate::bindings::date_utils::extract_date(as_of)?;
    let valuation_ref: &finstack_quant_portfolio::valuation::PortfolioValuation = &valuation;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    py.detach(|| {
        finstack_quant_portfolio::metrics::aggregate_metrics(valuation_ref, ccy, market_ref, date)
    })
    .map_err(portfolio_to_py)
}

/// Aggregate portfolio metrics from a valuation.
///
/// Parameters
/// ----------
/// valuation : PortfolioValuation | str
///     A :class:`PortfolioValuation` object (fast path) or JSON string.
/// base_currency : Currency | str
///     Base currency (``Currency`` or ISO-4217 code); an unknown code raises
///     ``ValueError``.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
///
/// Returns
/// -------
/// PortfolioMetrics
///     Typed aggregate-metrics wrapper. Use :meth:`to_json`
///     for the raw wire string.
#[pyfunction]
#[pyo3(text_signature = "(valuation, base_currency, market, as_of)")]
pub fn aggregate_metrics(
    py: Python<'_>,
    valuation: &Bound<'_, PyAny>,
    base_currency: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<crate::bindings::portfolio::types::PyPortfolioMetrics> {
    let metrics = run_aggregate_metrics(py, valuation, base_currency, market, as_of)?;
    Ok(crate::bindings::portfolio::types::PyPortfolioMetrics::from_inner(metrics))
}
/// Register spec functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(parse_portfolio_spec_json, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(aggregate_metrics, m)?)?;
    Ok(())
}
