use pyo3::prelude::*;

use finstack_quant_portfolio::optimization as opt;

use super::spec_result::{PyPortfolioOptimizationResult, PyPortfolioOptimizationSpec};

/// Run the optimizer against a typed :class:`PortfolioOptimizationSpec`.
///
/// Candidate IDs must be unique among candidates and absent from the existing
/// portfolio; collisions raise ``PortfolioError`` before solving.
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
