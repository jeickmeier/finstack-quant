//! Loss Given Default modeling primitives.
//!
//! Provides seniority-based recovery distributions, collateral-waterfall
//! workout LGD, downturn LGD adjustments, and EAD computation.
//!
//! # Module Organization
//!
//! - [`SeniorityCalibration`] / [`BetaRecovery`]: Beta-distributed recovery
//!   by debt seniority class.
//! - [`WorkoutLgd`]: Collateral-first recovery
//!   waterfall with costs and time-to-resolution discounting.
//! - [`DownturnLgd`]: Frye-Jacobs and
//!   regulatory-floor downturn LGD adjustments.
//! - [`EadCalculator`]: Exposure at default with Credit
//!   Conversion Factors.

mod downturn;
mod ead;
mod seniority;
mod workout;

pub use downturn::{DownturnLgd, DownturnMethod};
pub use ead::{CreditConversionFactor, EadCalculator};
pub use seniority::{
    BetaRecovery, SeniorityCalibration, SeniorityClass, seniority_calibration_ids,
    seniority_recovery_stats,
};
pub use workout::{
    CollateralPiece, WorkoutCollateralType, WorkoutCosts, WorkoutLgd, WorkoutLgdBuilder,
    WorkoutLgdResult,
};

#[cfg(test)]
mod tests;
