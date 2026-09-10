#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
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
//! | [`xva`] | CVA/DVA/FVA/MVA and shared XVA types |
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
//!   extension key `margin.registry.v1`.
//! - Factory methods such as `CsaSpec::usd_regulatory()` and
//!   `OtcMarginSpec::usd_bilateral()` resolve defaults from the embedded
//!   registry.
//! - XVA consumes caller-supplied exposure profiles; generating them requires
//!   the pricing stack, which sits above this crate.
//!
//! See the [crate README](../README.md) for detailed workflows and embedded data.
//!
//! # References
//!
//! - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
//! - BCBS-IOSCO uncleared margin: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
//! - SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
//! - XVA: `docs/REFERENCES.md#gregory-xva-challenge`

/// Margin calculation engines.
pub mod calculators;
/// Shared margin constants and heuristics.
pub mod constants;
/// Margin-specific analytics and instrument metrics.
pub mod metrics;
/// Embedded registry data and registry wiring.
pub(crate) mod registry;
/// Generated JSON Schema contract support.
pub mod schema;
pub mod table;
/// Standalone traits used by the margin crate.
pub mod traits;
/// Margin and collateral domain types.
pub mod types;
/// XVA configuration types (`types`) and adjustment formulas.
pub mod xva;

/// Regulatory capital frameworks (FRTB SBA, SA-CCR).
pub mod regulatory;

pub use calculators::im::schedule::{ScheduleAssetClass, BCBS_IOSCO_SCHEDULE_ID};
pub use calculators::im::simm::SimmVersion;
pub use calculators::{
    ClearingHouseImCalculator, HaircutImCalculator, ImCalculator, ImResult, ScheduleImCalculator,
    SimmCalculator, VmCalculator, VmResult,
};
pub use schema::MarginEnvelope;
pub use traits::Marginable;
pub use types::{
    ClearingStatus, CollateralAssetClass, CollateralEligibility, CsaSpec,
    EligibleCollateralSchedule, ImCollateralResult, ImMethodology, ImParameters, MarginCall,
    MarginCallTiming, MarginCallType, MarginTenor, MaturityConstraints, NettingSetId,
    OtcMarginSpec, RepoMarginSpec, RepoMarginType, SimmCreditClassification, SimmCreditSector,
    SimmCurvatureSensitivity, SimmRiskClass, SimmSensitivities, VmParameters, SIMM_TENORS,
};

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
