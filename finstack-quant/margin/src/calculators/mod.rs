//! Margin calculation engines.
//!
//! This module provides calculators for variation margin (VM) and
//! initial margin (IM) following industry standards.
//!
//! # Available Calculators
//!
//! - [`VmCalculator`](crate::calculators::VmCalculator): Variation margin calculation per ISDA CSA rules
//! - [`HaircutImCalculator`](crate::calculators::im::HaircutImCalculator): Haircut-based IM for repos
//! - [`SimmCalculator`](crate::calculators::im::SimmCalculator): ISDA SIMM for OTC derivatives
//! - [`ScheduleImCalculator`](crate::calculators::im::schedule::ScheduleImCalculator): BCBS-IOSCO regulatory schedule
//! - [`ClearingHouseImCalculator`](crate::calculators::im::ClearingHouseImCalculator): CCP-specific IM calculation

/// Initial-margin calculators and methodology-specific helpers.
pub mod im;
mod traits;
mod vm;

pub use traits::{
    ImCalculator, ImResult, SimmBucketDetail, SimmComponentDetail, SimmDetail,
    SimmWeightedSensitivity,
};
pub use vm::{VmCalculator, VmResult};
