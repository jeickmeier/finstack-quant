//! Core margin types and specifications.
//!
//! This module defines the fundamental types for margin and collateral management:
//!
//! - [`CsaSpec`]: Credit Support Annex specification
//! - [`VmParameters`]: Variation margin parameters
//! - [`ImParameters`]: Initial margin parameters
//! - [`EligibleCollateralSchedule`]: Eligible collateral with haircuts
//! - [`MarginCall`]: Margin call event representation
//! - [`OtcMarginSpec`]: OTC derivative margin specification

mod call;
mod collateral;
mod csa;
mod enums;
/// Netting-set identifiers and per-instrument margin aggregation outputs.
pub mod netting;
mod otc;
mod repo_cashflows;
/// Repo-specific margining conventions and helper calculations.
pub mod repo_margin;
/// SIMM risk-class and sensitivity container types.
pub mod simm_types;
mod thresholds;

// Re-export all types
pub use call::{MarginCall, MarginCallType};
pub use collateral::{
    CollateralAssetClass, CollateralEligibility, ConcentrationBreach, EligibleCollateralSchedule,
    MaturityConstraints,
};
pub use csa::{CsaSpec, MarginCallTiming};
pub use enums::{ClearingStatus, ImMethodology, MarginTenor};
pub use netting::{InstrumentMarginResult, NettingSetId};
pub use otc::OtcMarginSpec;
pub use repo_cashflows::{
    generate_margin_cashflows, generate_margin_interest_cashflows, margin_calls_to_cashflows,
};
pub use repo_margin::{RepoMarginSpec, RepoMarginType};
pub use simm_types::{
    ordered_credit_sector_pair, ordered_risk_class_pair, ordered_tenor_pair, SimmCreditSector,
    SimmRiskClass, SimmSensitivities,
};
pub use thresholds::{ImParameters, VmParameters};
