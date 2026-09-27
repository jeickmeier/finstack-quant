//! Product-independent credit models and analytics.
//!
//! This module owns structural and reduced-form models, rating migration,
//! scoring, probability-of-default calibration, loss-given-default models,
//! recovery allocation, and liability-management analytics.
//!
//! # Module Organization
//!
//! - [`merton`]: Merton / Black-Cox / CreditGrades structural default models.
//! - [`migration`]: Rating transition matrices and continuous-time simulation.
//! - [`pd`]: Probability-of-default calibration and master scales.
//! - [`lgd`]: Loss-given-default, recovery, and exposure-at-default models.
//! - [`scoring`]: Altman, Ohlson, and Zmijewski credit-scoring models.
//! - [`rating_factors`]: Rating-factor tables and Moody's WARF lookup.
//! - [`recovery_waterfall`]: Absolute-priority recovery allocation.
//! - [`liability_management`]: Distressed-exchange and LME analytics.
//! - [`dynamic_recovery`]: notional-dependent recovery curves for PIK accrual.
//! - [`endogenous_hazard`]: leverage-dependent hazard-rate feedback functions.
//! - [`toggle_exercise`]: threshold, stochastic, and nested-MC PIK toggle rules.
//! - [`market_anchored`]: convert market-quoted fractional credit volatility
//!   into the absolute parameters the callable lattice and the
//!   revolving-credit CIR process consume.

pub mod dynamic_recovery;
pub mod endogenous_hazard;
pub mod lgd;
pub mod liability_management;
pub mod market_anchored;
pub mod merton;
pub mod migration;
pub mod pd;
pub mod pool;
pub mod rating_factors;
pub mod recovery_waterfall;
pub mod registry;
pub mod scoring;
pub mod toggle_exercise;

pub use dynamic_recovery::DynamicRecoverySpec;
pub use endogenous_hazard::EndogenousHazardSpec;
pub use market_anchored::CreditVolatilityConversion;
pub use merton::{AssetDynamics, MertonBarrierType, MertonModel, SimulatedPaths};
pub use rating_factors::{moodys_warf_factor, RatingFactorTable};
pub use toggle_exercise::{
    CreditState, CreditStateVariable, OptimalToggle, ThresholdDirection, ToggleExerciseModel,
};
