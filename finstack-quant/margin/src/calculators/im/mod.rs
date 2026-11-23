//! Initial margin calculators.
//!
//! This module provides different IM calculation methodologies:
//!
//! - [`HaircutImCalculator`](crate::calculators::im::HaircutImCalculator): For repos and securities financing
//! - [`SimmCalculator`](crate::calculators::im::SimmCalculator): ISDA SIMM for OTC derivatives
//! - [`ScheduleImCalculator`](crate::calculators::im::schedule::ScheduleImCalculator): BCBS-IOSCO regulatory schedule fallback
//! - [`ClearingHouseImCalculator`](crate::calculators::im::ClearingHouseImCalculator): CCP-specific methodologies

mod clearing;
mod exposure;
mod haircut;
/// BCBS-IOSCO schedule-based IM fallback calculator.
pub mod schedule;
/// ISDA SIMM calculator and supporting types.
pub(crate) mod simm;

pub use clearing::{CcpMethodology, ClearingHouseImCalculator};
pub use haircut::{HAIRCUT_MPOR_DAYS, HaircutImCalculator};
pub use simm::calculator::SimmCalculator;

pub(crate) use exposure::{conservative_im, require_im_exposure_base};
