#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Margin, collateral, XVA configuration, and regulatory capital helpers.
//!
//! This crate is standalone from `finstack-quant-valuations` so consumers can
//! share agreement terms, IM/VM engines, registry-backed defaults, and
//! regulatory capital helpers without pulling the full instrument stack.
//!
//! # Module Guide
//!
//! | Module | Role |
//! |--------|------|
//! | [`types`] | CSA, collateral, repo, SIMM, netting identifiers |
//! | [`calculators`] | VM and IM engines (SIMM, schedule, haircut, CCP proxy) |
//! | [`traits`] | `Marginable` for consumer-crate integration |
//! | [`metrics`] | IM/VM metrics, utilization, excess collateral, funding cost, Haircut01 |
//! | [`regulatory`] | FRTB sensitivity-based approach and SA-CCR EAD |
//! | [`constants`] | Shared heuristics |
//! | [`xva`] | Deterministic exposure, netting, CVA/DVA/FVA, and shared XVA types |
//!
//! # Quick Start
//!
//! ```no_run
//! use finstack_quant_margin::{CsaSpec, OtcMarginSpec};
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let csa = CsaSpec::usd_regulatory()?;
//! let spec = OtcMarginSpec::bilateral_simm(csa);
//!
//! assert!(spec.csa.requires_im());
//! # Ok(())
//! # }
//! ```
//!
//! # Conventions
//!
//! - Registry JSON is embedded at build time. Overlays use the Finstack config
//!   extension key `valuations.margin_registry.v1`.
//! - Factory methods such as `CsaSpec::usd_regulatory()` and
//!   `OtcMarginSpec::usd_bilateral()` resolve defaults from the embedded
//!   registry.
//! - XVA exposure engines are deterministic (roll-forward); Monte Carlo
//!   exposure paths live in `finstack-quant-monte-carlo`.
//!
//! See the [crate README](../README.md) for detailed workflows and embedded data.

/// Margin calculation engines.
pub mod calculators;
/// Shared margin constants and heuristics.
pub mod constants;
/// Margin-specific analytics and instrument metrics.
pub mod metrics;
/// Embedded registry data and registry wiring.
pub(crate) mod registry;
/// Standalone traits used by the margin crate.
pub mod traits;
/// Margin and collateral domain types.
pub mod types;
/// XVA configuration types (`types`); exposure and adjustment engines are crate-internal.
pub mod xva;

/// Regulatory capital frameworks (FRTB SBA, SA-CCR).
pub mod regulatory;

pub use calculators::im::schedule::{ScheduleAssetClass, BCBS_IOSCO_SCHEDULE_ID};
pub use calculators::im::simm::SimmVersion;
pub use calculators::{
    CcpMethodology, ClearingHouseImCalculator, ExternalImSource, HaircutImCalculator, ImCalculator,
    ImResult, InternalModelImCalculator, ScheduleImCalculator, SimmCalculator, VmCalculator,
    VmResult,
};
pub use traits::Marginable;
pub use types::{
    generate_margin_cashflows, generate_margin_interest_cashflows, margin_calls_to_cashflows,
    ClearingStatus, CollateralAssetClass, CollateralEligibility, ConcentrationBreach, CsaSpec,
    EligibleCollateralSchedule, ImMethodology, ImParameters, InstrumentMarginResult, MarginCall,
    MarginCallTiming, MarginCallType, MarginTenor, MaturityConstraints, NettingSetId,
    OtcMarginSpec, RepoMarginSpec, RepoMarginType, SimmCreditSector, SimmRiskClass,
    SimmSensitivities, SimmSensitivitiesJson, VmParameters,
};
