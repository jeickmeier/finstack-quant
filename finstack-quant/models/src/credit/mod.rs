//! Product-independent credit models and analytics.
//!
//! This module owns structural and reduced-form models, rating migration,
//! scoring, probability-of-default calibration, loss-given-default models,
//! recovery allocation, and liability-management analytics.
//!
//! # Module Organization
//!
//! - [`MertonModel`]: Merton / Black-Cox / CreditGrades structural default models.
//! - [`migration`]: Rating transition matrices and continuous-time simulation.
//! - [`pd`]: Probability-of-default calibration and master scales.
//! - [`lgd`]: Loss-given-default, recovery, and exposure-at-default models.
//! - [`scoring`]: Altman, Ohlson, and Zmijewski credit-scoring models.
//! - [`RatingFactorTable`] / [`moodys_warf_factor`]: Rating-factor tables and Moody's WARF lookup.
//! - [`recovery_waterfall`]: Absolute-priority recovery allocation.
//! - [`liability_management`]: Distressed-exchange and LME analytics.
//! - [`DynamicRecoverySpec`]: notional-dependent recovery curves for PIK accrual.
//! - [`EndogenousHazardSpec`]: leverage-dependent hazard-rate feedback functions.
//! - [`ToggleExerciseModel`]: threshold, stochastic, and nested-MC PIK toggle rules.
//! - [`market_anchored`]: convert market-quoted fractional credit volatility
//!   into the absolute parameters the callable lattice and the
//!   revolving-credit CIR process consume.

mod dynamic_recovery;
mod endogenous_hazard;
pub mod lgd;
pub mod liability_management;
pub mod market_anchored;
mod merton;
pub mod migration;
pub mod pd;
pub mod pool;
mod rating_factors;
pub mod recovery_waterfall;
pub(crate) mod registry;
pub mod scoring;
mod toggle_exercise;

pub use dynamic_recovery::{DynamicRecoveryModel, DynamicRecoverySpec};
pub use endogenous_hazard::{EndogenousHazardSpec, LeverageHazardMap};
pub use merton::{AssetDynamics, MertonBarrierType, MertonModel, SimulatedPaths};
pub use rating_factors::{moodys_warf_factor, RatingFactorTable};
pub use toggle_exercise::{
    CreditState, CreditStateVariable, OptimalToggle, StochasticToggle, ThresholdDirection,
    ThresholdToggle, ToggleExerciseModel,
};
