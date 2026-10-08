//! Portfolio-level factor risk decomposition outputs and engines.
//!
//! This module lifts instrument-level market dependencies and sensitivities into
//! portfolio-level factor analytics. Typical usage is:
//!
//! 1. Build a [`crate::factor_model::FactorModel`] from a declarative
//!    [`finstack_quant_models::factor::FactorModelConfig`].
//! 2. Use [`crate::factor_model::FactorModel::assign_factors`] to inspect how
//!    portfolio positions map to configured factors.
//! 3. Use [`crate::factor_model::FactorModel::compute_sensitivities`] to produce
//!    a weighted sensitivity matrix.
//! 4. Use [`crate::factor_model::FactorModel::analyze`] to decompose portfolio risk.
//!
//! Risk is decomposed with the closed-form covariance-based
//! [`finstack_quant_models::factor::risk::decompose_factors`], which assumes
//! the upstream sensitivity engine has already scaled rows by position
//! quantity, so decomposition works on portfolio exposures directly.
//!
//! # Conventions
//!
//! - Factor IDs and covariance axes must match exactly in content and order.
//! - Risk outputs are reported in the units implied by the configured
//!   [`finstack_quant_models::factor::RiskMeasure`].
//! - Strict unmatched-dependency handling should be used when factor coverage is
//!   treated as part of the model contract rather than a best-effort mapping.
//!
//! # References
//!
//! - Meucci, factor risk and covariance aggregation: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//!
//! - Parametric VaR conventions: `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
//!
//! - Coherent/tail-risk measures: `docs/REFERENCES.md#artzner1999CoherentRisk`
//!

mod assignment;
mod credit_vol_report;
mod dependencies;
mod model;
mod weight_allocation;
mod whatif;

pub use assignment::{FactorAssignmentReport, PositionAssignment, UnmatchedEntry};
pub use credit_vol_report::{
    build_credit_vol_report, CreditVolReport, LevelVolContribution, PositionVolContribution,
};
pub use model::FactorModel;
pub use weight_allocation::{
    allocate_weights, allocate_weights_json, validate_allocation_json, AllocationDiagnostics,
    AllocationScheme, StrategyAllocation, StrategyAllocationInput, WeightAllocationResult,
    WeightAllocationSpec,
};
pub use whatif::{FactorContributionDelta, PositionChange, StressPnl, StressResult, WhatIfResult};
