//! Python bindings for the `finstack-portfolio` crate.
//!
//! Portfolio contains `Arc<dyn Instrument>` which cannot be directly wrapped,
//! so this module exposes JSON-based construction via [`PortfolioSpec`],
//! result extraction via serde round-trips, and end-to-end pipeline functions
//! that build the runtime portfolio internally.

mod brinson;
mod liquidity;
mod optimization;
mod performance;
mod pipeline;
mod position_risk;
mod replay;
mod spec;
pub(crate) mod types;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Parse an ISO 8601 date string into a `time::Date`.
fn parse_date(s: &str) -> PyResult<time::Date> {
    let format = time::format_description::well_known::Iso8601::DEFAULT;
    time::Date::parse(s, &format)
        .map_err(|e| PyValueError::new_err(format!("Invalid date '{s}': {e}")))
}

/// Register the `portfolio` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "portfolio")?;
    m.setattr(
        "__doc__",
        "Portfolio construction, valuation, cashflows, scenarios, and metrics.",
    )?;
    m.add(
        "PortfolioError",
        py.get_type::<crate::errors::PortfolioError>(),
    )?;
    m.add(
        "FinstackValuationError",
        py.get_type::<crate::errors::FinstackValuationError>(),
    )?;
    m.add(
        "FinstackFxError",
        py.get_type::<crate::errors::FinstackFxError>(),
    )?;
    m.add(
        "FinstackOptimizationError",
        py.get_type::<crate::errors::FinstackOptimizationError>(),
    )?;

    types::register(py, &m)?;
    spec::register(py, &m)?;
    pipeline::register(py, &m)?;
    optimization::register(py, &m)?;
    replay::register(py, &m)?;
    position_risk::register(py, &m)?;
    liquidity::register(py, &m)?;
    brinson::register(py, &m)?;
    performance::register(py, &m)?;

    let exports = vec![
        "PortfolioError",
        "FinstackValuationError",
        "FinstackFxError",
        "FinstackOptimizationError",
        "Portfolio",
        "PortfolioValuation",
        "PortfolioResult",
        "PortfolioCashflows",
        "parse_portfolio_spec",
        "build_portfolio_from_spec",
        "portfolio_result_total_value",
        "portfolio_result_get_metric",
        "aggregate_metrics",
        "value_portfolio",
        "aggregate_full_cashflows",
        "apply_scenario_and_revalue",
        "optimize_portfolio",
        "replay_portfolio",
        "parametric_var_decomposition",
        "parametric_es_decomposition",
        "historical_var_decomposition",
        "evaluate_risk_budget",
        "roll_effective_spread",
        "amihud_illiquidity",
        "days_to_liquidate",
        "liquidity_tier",
        "lvar_bangia",
        "almgren_chriss_impact",
        "kyle_lambda",
        "brinson_fachler",
        "carino_link",
        "twrr_modified_dietz",
        "twrr_linked",
        "mwr_xirr",
    ];

    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_by_parent_name(
        py,
        parent,
        &m,
        "portfolio",
        "finstack.finstack",
    )?;

    Ok(())
}
