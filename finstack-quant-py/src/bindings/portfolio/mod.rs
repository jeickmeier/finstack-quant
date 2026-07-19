//! Python bindings for the `finstack-quant-portfolio` crate.
//!
//! Portfolio contains `Arc<dyn Instrument>` which cannot be directly wrapped,
//! so this module exposes JSON-based construction via [`PortfolioSpec`],
//! result extraction via serde round-trips, and end-to-end pipeline functions
//! that build the runtime portfolio internally.

mod allocation;
mod attribution;
mod brinson;
mod factor_model;
mod json_bridge;
mod liquidity;
mod matrix_input;
mod optimization;
mod optimization_spec;
mod performance;
mod pipeline;
mod position_risk;
mod replay;
mod sensitivity;
mod spec;
pub(crate) mod types;

use crate::bindings::date_utils::parse_iso_date_py as parse_date;
use pyo3::prelude::*;
use pyo3::types::PyList;

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
    attribution::register(py, &m)?;
    optimization::register(py, &m)?;
    optimization_spec::register(py, &m)?;
    allocation::register(py, &m)?;
    replay::register(py, &m)?;
    position_risk::register(py, &m)?;
    factor_model::register(py, &m)?;
    sensitivity::register(py, &m)?;
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
        "PortfolioMetrics",
        "PortfolioCashflows",
        "PortfolioAttribution",
        "parse_portfolio_spec",
        "build_portfolio_from_spec",
        "portfolio_result_total_value",
        "portfolio_result_get_metric",
        "aggregate_metrics",
        "value_portfolio",
        "value_portfolio_typed",
        "aggregate_full_cashflows",
        "apply_scenario_and_revalue",
        "scenario_pnl",
        "scenario_pnl_batch",
        "attribute_portfolio_pnl",
        "allocate_weights",
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
        "SensitivityMatrix",
        "FactorPnlProfile",
        "FactorRiskDecomposition",
        "compute_factor_sensitivities",
        "compute_pnl_profiles",
        "decompose_factor_risk",
        // factor_model typed result classes (Slice 8)
        "FactorContribution",
        "PositionFactorContribution",
        "PositionResidualContribution",
        "RiskDecomposition",
        "PositionVarContribution",
        "PositionEsContribution",
        "PositionRiskDecomposition",
        "PositionBudgetEntry",
        "RiskBudgetResult",
        "FactorContributionDelta",
        "WhatIfResult",
        "StressResult",
        "StressPositionEntry",
        "TailScenarioBreakdown",
        "StressAttribution",
        "PositionAssignment",
        "UnmatchedEntry",
        "FactorAssignmentReport",
        "LevelVolContribution",
        "PositionVolContribution",
        "CreditVolReport",
        "VolHorizon",
        "DecompositionConfig",
        "parametric_var_decomposition_typed",
        "historical_var_decomposition_typed",
        "evaluate_risk_budget_typed",
        "factor_stress",
        "position_what_if",
        "build_stress_attribution",
        "build_credit_vol_report",
        "validate_allocation_json",
        "position_component_var",
        // optimization spec/result classes (Slice 9)
        "WeightingScheme",
        "MissingMetricPolicy",
        "Inequality",
        "TradeDirection",
        "TradeType",
        "PerPositionMetric",
        "PositionFilter",
        "MetricExpr",
        "Objective",
        "Constraint",
        "CandidatePosition",
        "TradeUniverse",
        "OptimizationStatus",
        "TradeSpec",
        "PortfolioOptimizationSpec",
        "PortfolioOptimizationResult",
        "optimize_portfolio_typed",
    ];

    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "portfolio",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
