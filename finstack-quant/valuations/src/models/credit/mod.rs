//! Structural credit models for default probability estimation.
//!
//! This module provides structural and reduced-form helpers used by credit
//! instruments and PIK-toggle bond pricing.
//!
//! # Module Organization
//!
//! - [`merton`]: Merton / Black-Cox / CreditGrades structural default models.
//! - [`dynamic_recovery`]: notional-dependent recovery curves for PIK accrual.
//! - [`endogenous_hazard`]: leverage-dependent hazard-rate feedback functions.
//! - [`toggle_exercise`]: threshold, stochastic, and nested-MC PIK toggle rules.

pub mod dynamic_recovery;
pub mod endogenous_hazard;
pub mod merton;
pub mod toggle_exercise;

pub use dynamic_recovery::DynamicRecoverySpec;
pub use endogenous_hazard::EndogenousHazardSpec;
pub use merton::{AssetDynamics, BarrierType, MertonModel};
pub use toggle_exercise::{
    CreditState, CreditStateVariable, OptimalToggle, ThresholdDirection, ToggleExerciseModel,
};
