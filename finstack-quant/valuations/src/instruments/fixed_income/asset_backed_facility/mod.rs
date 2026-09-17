//! Asset-backed facility (warehouse / forward-flow line): a committed
//! facility lending against a collateral pool under advance rates,
//! concentration limits and a borrowing-base test.
//!
//! The facility reuses the structured-credit engine: [`AssetBackedFacility::synthesized_deal`]
//! builds a two-class deal (facility note + residual) whose
//! `CoverageTestType::BorrowingBase` test enforces the borrowing base every
//! period, whose reinvestment period is the revolving period, and whose
//! early-amortization rules carry the loss / excess-spread amortization
//! events. [`AssetBackedFacility::project`] returns the note's flows, the
//! unused-commitment fee and the residual; the `Instrument` impl prices the
//! lender's flows on the discount curve.
//!
//! # Quick example
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacility;
//!
//! let facility = AssetBackedFacility::example()?;
//! let deal = facility.synthesized_deal()?;
//! assert_eq!(deal.tranches.tranches.len(), 2);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

pub(crate) mod metrics;
mod pricing;
mod types;

pub use metrics::{
    AbfAdvanceRateUtilizationCalculator, AbfBorrowingBaseCalculator,
    AbfBorrowingBaseCushionCalculator, AbfFacilityIrrCalculator, AbfResidualIrrCalculator,
};
pub use pricing::{FacilityProjection, FACILITY_TRANCHE_ID, RESIDUAL_TRANCHE_ID};
pub use types::{AmortizationEvent, AssetBackedFacility, AssetBackedFacilityBuilder, TermOutSpec};

pub use crate::instruments::fixed_income::structured_credit::{
    AdvanceRate, BorrowingBaseReport, BorrowingBaseRules, ConcentrationLimit, ConcentrationScope,
    EligibilityRule,
};
