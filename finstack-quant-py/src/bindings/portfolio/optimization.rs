//! Python bindings for portfolio optimization.
//!
//! Accepts either a typed :class:`Portfolio` object (fast path) or a JSON
//! string containing a ``PortfolioOptimizationSpec``. When a typed
//! ``Portfolio`` is supplied, the objective, constraints, and weighting
//! scheme must be provided through ``opt_spec_json``.

use crate::bindings::extract::extract_market;
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;

/// Optimize a portfolio from a JSON specification.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-serialized ``PortfolioOptimizationSpec`` containing the portfolio,
///     objective, constraints, and weighting scheme.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``PortfolioOptimizationResult`` with optimal weights,
///     trade list, dual values, and diagnostics. Compact JSON — use
///     :func:`json.dumps(json.loads(result), indent=2)` to pretty-print.
#[pyfunction]
fn optimize_portfolio(
    py: Python<'_>,
    spec_json: &str,
    market: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let spec: finstack_quant_portfolio::optimization::PortfolioOptimizationSpec =
        serde_json::from_str(spec_json).map_err(display_to_py)?;
    let market = extract_market(market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    // The LP solve (portfolio build + valuation + simplex/interior-point) can
    // run for seconds on large books; release the GIL for it. The owned spec
    // and market are moved into the closure so it stays `Send` with no
    // pyclass borrow held across the GIL-release boundary.
    let result = py
        .detach(move || {
            finstack_quant_portfolio::optimization::optimize_from_spec(&spec, &market, &config)
        })
        .map_err(portfolio_to_py)?;
    serde_json::to_string(&result).map_err(display_to_py)
}

/// Register optimization functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(optimize_portfolio, m)?)?;
    Ok(())
}
