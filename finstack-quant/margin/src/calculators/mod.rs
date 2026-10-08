//! Margin calculation engines.
//!
//! This module provides calculators for variation margin (VM) and
//! initial margin (IM) following industry standards.
//!
//! # Available Calculators
//!
//! - [`VmCalculator`]: Variation margin calculation per ISDA CSA rules
//! - [`HaircutImCalculator`]: Haircut-based IM for repos
//! - [`SimmCalculator`]: ISDA SIMM for OTC derivatives
//! - [`ScheduleImCalculator`]: BCBS-IOSCO regulatory schedule
//! - [`ClearingHouseImCalculator`]: CCP-specific IM calculation

/// Initial-margin calculators and methodology-specific helpers.
pub mod im;
mod traits;
mod vm;

pub use im::{
    CcpMethodology, ClearingHouseImCalculator, ExternalImSource, HaircutImCalculator,
    ScheduleImCalculator, SimmCalculator,
};
pub use traits::{
    ImCalculator, ImResult, SimmBucketDetail, SimmComponentDetail, SimmDetail,
    SimmWeightedSensitivity,
};
pub use vm::{VmCalculator, VmResult};
