//! Core margin types and specifications.
//!
//! This module defines the fundamental types for margin and collateral management:
//!
//! - [`CsaSpec`](crate::types::CsaSpec): Credit Support Annex specification
//! - [`VmParameters`](crate::types::VmParameters): Variation margin parameters
//! - [`ImParameters`](crate::types::ImParameters): Initial margin parameters
//! - [`EligibleCollateralSchedule`](crate::types::EligibleCollateralSchedule): Eligible collateral with haircuts
//! - [`MarginCall`](crate::types::MarginCall): Margin call event representation
//! - [`OtcMarginSpec`](crate::types::OtcMarginSpec): OTC derivative margin specification

mod calendar;
mod call;
mod collateral;
mod csa;
mod enums;
/// Netting-set identifiers and per-instrument margin aggregation outputs.
pub(crate) mod netting;
mod otc;
mod repo_cashflows;
/// Repo-specific margining conventions and helper calculations.
pub(crate) mod repo_margin;
mod serde_validation;
pub(crate) mod simm_curvature;
/// SIMM risk-class and sensitivity container types.
pub(crate) mod simm_types;
mod thresholds;

use calendar::default_margin_calendar;
pub use call::{MarginCall, MarginCallType};
pub use collateral::{
    CollateralAssetClass, CollateralEligibility, ConcentrationBreach, EligibleCollateralSchedule,
    MaturityConstraints,
};
pub use csa::{CsaSpec, CsaSpecBuilder, MarginCallTiming};
pub use enums::{ClearingStatus, ImMethodology, MarginTenor};
pub use netting::NettingSetId;
pub use otc::{OtcMarginSpec, SimmCreditClassification};
pub use repo_cashflows::{
    generate_margin_cashflows, generate_margin_interest_cashflows, margin_calls_to_cashflows,
};
pub use repo_margin::{RepoMarginSpec, RepoMarginType};
pub use simm_curvature::SimmCurvatureSensitivity;
pub use simm_types::{
    SIMM_COMMODITY_BUCKET_COUNT, SIMM_TENORS, SimmCreditSector, SimmRiskClass, SimmSensitivities,
    SimmSensitivitiesJson, commodity_bucket_id,
};
pub(crate) use simm_types::{ordered_pair, ordered_tenor_pair};
pub use thresholds::{ImCollateralResult, ImParameters, VmParameters};
