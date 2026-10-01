use pyo3::prelude::*;

use finstack_quant_portfolio::optimization as opt;

use super::spec_result::{PyPortfolioOptimizationResult, PyPortfolioOptimizationSpec};

/// Run the optimizer against a typed :class:`PortfolioOptimizationSpec`.
#[pyfunction]
#[pyo3(signature = (spec, market))]
pub(super) fn optimize_portfolio(
    py: Python<'_>,
    spec: &PyPortfolioOptimizationSpec,
    market: &Bound<'_, PyAny>,
) -> PyResult<PyPortfolioOptimizationResult> {
    let spec = spec.inner.clone();
    let market = crate::bindings::extract::extract_market(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    // Release the GIL for the (potentially multi-second) LP solve. The owned
    // spec and market move into the closure so it stays `Send` with no pyclass
    // borrow held across the GIL-release boundary. Both clones are negligible
    // next to the optimization itself.
    let result = py
        .detach(move || opt::optimize_from_spec(&spec, &market, &config))
        .map_err(crate::errors::portfolio_to_py)?;
    Ok(PyPortfolioOptimizationResult::from_inner(result))
}

/// Rebalance a spec's portfolio to an optimization result.
///
/// Mirrors Rust ``optimization::rebalance_from_spec``: held positions take the
/// result's implied quantities and trade-universe candidates with a
/// non-negligible target weight and quantity become new positions. Unlike
/// ``PortfolioOptimizationResult.to_rebalanced_portfolio`` it also works on a
/// result rebuilt from JSON or unpickled.
#[pyfunction]
#[pyo3(signature = (spec, result))]
pub(super) fn rebalance_from_spec(
    spec: &PyPortfolioOptimizationSpec,
    result: &PyPortfolioOptimizationResult,
) -> PyResult<crate::bindings::portfolio::types::PyPortfolio> {
    let portfolio = opt::rebalance_from_spec(&spec.inner, &result.inner)
        .map_err(crate::errors::portfolio_to_py)?;
    Ok(crate::bindings::portfolio::types::PyPortfolio {
        inner: std::sync::Arc::new(portfolio),
    })
}
