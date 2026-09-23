//! Merton Monte Carlo engine for PIK bonds with structural credit risk.
//!
//! Orchestrates [`finstack_quant_models::credit::MertonModel`],
//! [`finstack_quant_models::credit::EndogenousHazardSpec`],
//! [`finstack_quant_models::credit::DynamicRecoverySpec`], and
//! [`finstack_quant_models::credit::ToggleExerciseModel`] into a
//! unified Monte Carlo simulation for pricing bonds with PIK (payment-in-kind)
//! features.
//!
//! # Algorithm
//!
//! For each Monte Carlo path:
//! 1. Evolve asset value via GBM (or jump-diffusion) time steps.
//! 2. Determine the hazard rate (endogenous or Merton-implied).
//! 3. Check for default via first-passage barrier breach.
//! 4. At coupon dates, apply PIK/cash toggle logic.
//! 5. Compute terminal payment for surviving paths.
//!
//! Aggregate across paths to produce clean price, expected/unexpected loss,
//! expected shortfall, and path statistics.
//!
//! # Runtime Contract
//!
//! This module is part of the standard valuations build and is selected through
//! `ModelKey::MertonMc`.

mod engine;
mod pricer;
mod types;

pub mod calibration;

pub(crate) use engine::MertonBondTerms;
pub use engine::MertonMcEngine;
pub use types::{
    BarrierCrossing, CalibrationParameter, MertonMcCalibrationSpec, MertonMcConfig, MertonMcResult,
    PathStatistics, PikMode, PikSchedule,
};

pub(crate) use pricer::SimpleBondMertonMcPricer;
