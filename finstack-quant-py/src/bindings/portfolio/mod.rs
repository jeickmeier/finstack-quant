//! Python bindings for the `finstack-quant-portfolio` crate.
//!
//! Portfolios are built either through the typed `Portfolio.builder(...)`
//! (wrapping the Rust `PortfolioBuilder`) or from canonical `PortfolioSpec`
//! JSON; results are typed wrappers with `to_json` / `from_json` twins, and
//! end-to-end pipeline functions accept the typed handles directly.

mod allocation;
mod attribution;
mod brinson;
mod excess_return;
mod factor_brinson;
pub(crate) mod factor_model;
mod fi_attribution;
mod grid_attribution;
mod json_bridge;
mod materialization;
pub(crate) mod matrix_input;
mod optimization_spec;
mod performance;
mod pipeline;
mod replay;
mod scenario_pnl;
mod schema;
mod sensitivity;
mod spec;
pub(crate) mod types;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `portfolio` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "portfolio")?;
    m.setattr(
        "__doc__",
        "Portfolio construction, valuation, cashflows, scenarios, and metrics.",
    )?;
    // Common base for every named finstack exception; its canonical home is
    // `finstack_quant.core`, and it is re-exported here alongside the portfolio
    // family so `except FinstackError` is reachable without a second import.
    m.add(
        "FinstackError",
        py.get_type::<crate::errors::FinstackError>(),
    )?;
    m.add(
        "PortfolioError",
        py.get_type::<crate::errors::PortfolioError>(),
    )?;
    m.add(
        "ContractValidationError",
        py.get_type::<crate::errors::ContractValidationError>(),
    )?;
    m.add(
        "UnsupportedContractVersionError",
        py.get_type::<crate::errors::UnsupportedContractVersionError>(),
    )?;
    m.add(
        "MissingContractVersionError",
        py.get_type::<crate::errors::MissingContractVersionError>(),
    )?;
    m.add(
        "MalformedContractSchemaError",
        py.get_type::<crate::errors::MalformedContractSchemaError>(),
    )?;
    m.add(
        "ContractLimitExceededError",
        py.get_type::<crate::errors::ContractLimitExceededError>(),
    )?;

    types::register(py, &m)?;
    materialization::register(py, &m)?;
    spec::register(py, &m)?;
    pipeline::register(py, &m)?;
    attribution::register(py, &m)?;
    optimization_spec::register(py, &m)?;
    allocation::register(py, &m)?;
    replay::register(py, &m)?;
    factor_model::register(py, &m)?;
    sensitivity::register(py, &m)?;
    brinson::register(py, &m)?;
    fi_attribution::register(py, &m)?;
    performance::register(py, &m)?;
    excess_return::register(py, &m)?;
    grid_attribution::register(py, &m)?;
    factor_brinson::register(py, &m)?;

    let exports = vec![
        "FinstackError",
        "PortfolioError",
        "ContractValidationError",
        "UnsupportedContractVersionError",
        "MissingContractVersionError",
        "MalformedContractSchemaError",
        "ContractLimitExceededError",
        "Portfolio",
        "PortfolioBuilder",
        "PositionValue",
        "ReconciliationReport",
        "InstrumentArtifactCache",
        "MaterializationReport",
        "PortfolioValuation",
        "ScenarioPnl",
        "ScenarioPnlBatchItem",
        "PortfolioResult",
        "PortfolioMetrics",
        "PortfolioCashflows",
        "PortfolioAttribution",
        "BrinsonPeriodResult",
        "CarinoLinkedAttribution",
        "FiAttributionResult",
        "FiCarinoLinkedResult",
        "FiReconciliationReport",
        "DurationCellTable",
        "ExcessReturnResult",
        "GridAttributionResult",
        "GridCarinoLinkedResult",
        "FactorBrinsonResult",
        "LinkedReturn",
        "ReplayResult",
        "WeightAllocationResult",
        "parse_portfolio_spec_json",
        "build_portfolio_from_spec_json",
        "aggregate_metrics",
        "aggregate_metrics_json",
        "value_portfolio",
        "aggregate_full_cashflows",
        "net_in_currency_by_date",
        "apply_scenario_and_revalue",
        "scenario_pnl",
        "scenario_pnl_batch",
        "scenario_pnl_batch_json",
        "attribute_portfolio_pnl",
        "allocate_weights",
        "allocate_weights_json",
        "optimize_portfolio",
        "rebalance_from_spec",
        "replay_portfolio",
        "replay_portfolio_json",
        "brinson_fachler",
        "brinson_fachler_json",
        "carino_link",
        "carino_link_json",
        "carino_link_from_sector_periods",
        "carino_link_from_sector_periods_json",
        "campisi_attribution",
        "campisi_attribution_json",
        "campisi_carino_link",
        "campisi_carino_link_json",
        "campisi_carino_link_from_snapshots",
        "campisi_carino_link_from_snapshots_json",
        "campisi_reconciliation_check",
        "campisi_reconciliation_check_json",
        "cell_returns_from_curves",
        "cell_returns_from_curves_json",
        "cell_returns_from_reference",
        "cell_returns_from_reference_json",
        "excess_returns",
        "excess_returns_json",
        "factor_brinson_attribution",
        "factor_brinson_attribution_json",
        "grid_attribution",
        "grid_attribution_json",
        "grid_carino_link",
        "grid_carino_link_json",
        "twrr_modified_dietz",
        "twrr_linked",
        "twrr_linked_json",
        "mwr_xirr",
        "SensitivityMatrix",
        "FactorPnlProfile",
        "compute_factor_sensitivities",
        "compute_pnl_profiles",
        "decompose_factor_risk",
        // portfolio-owned factor workflow result classes
        "FactorContributionDelta",
        "WhatIfResult",
        "StressResult",
        "PositionAssignment",
        "UnmatchedEntry",
        "FactorAssignmentReport",
        "LevelVolContribution",
        "PositionVolContribution",
        "CreditVolReport",
        "factor_stress",
        "position_what_if",
        "build_credit_vol_report",
        "validate_allocation_json",
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
        "schema",
    ];

    schema::register(py, &m)?;
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
