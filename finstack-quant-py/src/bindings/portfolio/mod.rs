//! Python bindings for the `finstack-quant-portfolio` crate.
//!
//! Portfolios are built either through the typed `Portfolio.builder(...)`
//! (wrapping the Rust `PortfolioBuilder`) or from canonical `PortfolioSpec`
//! JSON; results are typed wrappers with `to_json` / `from_json` twins, and
//! end-to-end pipeline functions accept the typed handles directly.

mod allocation;
mod attribution;
mod brinson;
pub(crate) mod cashflows;
pub(crate) mod container;
mod excess_return;
mod factor_brinson;
pub(crate) mod factor_model;
mod fi_attribution;
mod grid_attribution;
mod margin;
mod materialization;
mod metrics;
mod optimization;
mod performance;
mod primitive;
mod replay;
mod results;
mod scenarios;
mod schema;
mod sensitivity;
pub(crate) mod valuation;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `portfolio` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "portfolio")?;
    m.setattr(
        "__doc__",
        "Portfolio construction, valuation, cashflows, scenarios, and metrics.",
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

    container::register(py, &m)?;
    valuation::register(py, &m)?;
    results::register(py, &m)?;
    metrics::register(py, &m)?;
    cashflows::register(py, &m)?;
    scenarios::register(py, &m)?;
    materialization::register(py, &m)?;
    attribution::register(py, &m)?;
    optimization::register(py, &m)?;
    allocation::register(py, &m)?;
    replay::register(py, &m)?;
    factor_model::register(py, &m)?;
    primitive::register(py, &m)?;
    sensitivity::register(py, &m)?;
    brinson::register(py, &m)?;
    fi_attribution::register(py, &m)?;
    performance::register(py, &m)?;
    excess_return::register(py, &m)?;
    grid_attribution::register(py, &m)?;
    factor_brinson::register(py, &m)?;
    margin::register(py, &m)?;

    let exports = vec![
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
        "DurationCellTable",
        "ExcessReturnResult",
        "GridAttributionResult",
        "GridCarinoLinkedResult",
        "FactorBrinsonResult",
        "LinkedReturn",
        "ReplayResult",
        "WeightAllocationResult",
        "parse_portfolio_spec_json",
        "aggregate_metrics",
        "value_portfolio",
        "aggregate_full_cashflows",
        "net_in_currency_by_date",
        "apply_scenario_and_revalue",
        "scenario_pnl",
        "scenario_pnl_batch",
        "attribute_portfolio_pnl",
        "allocate_weights",
        "optimize_portfolio",
        "rebalance_from_spec",
        "replay_portfolio",
        "brinson_fachler",
        "carino_link",
        "campisi_attribution",
        "campisi_carino_link",
        "cell_returns_from_curves",
        "cell_returns_from_reference",
        "excess_returns",
        "factor_brinson_attribution",
        "grid_attribution",
        "grid_carino_link",
        "twrr_modified_dietz",
        "twrr_linked",
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
        "StressPnl",
        "FactorModel",
        "PositionAssignment",
        "UnmatchedEntry",
        "FactorAssignmentReport",
        "LevelVolContribution",
        "PositionVolContribution",
        "CreditVolReport",
        "build_credit_vol_report",
        "PortfolioPrimitiveExposureReport",
        "primitive_exposures",
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
        // portfolio margin aggregation
        "NettingSetMargin",
        "PortfolioMarginAggregator",
        "PortfolioMarginResult",
        "schema",
    ];

    schema::register(py, &m)?;
    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
