//! Stochastic models for structured credit.
//!
//! This module provides stochastic prepayment and default models with:
//! - Factor-driven CPR/CDR models with correlation
//! - Industry-standard calibrations (RMBS, CLO, CMBS)
//! - Stochastic pricing with tree and Monte Carlo modes
//!
//! # Module Organization
//!
//! - [`calibrations`]: Standard calibration constants for RMBS, CLO, CMBS
//! - [`prepayment`]: Stochastic prepayment models (factor-correlated, Richard-Roll)
//! - [`default`]: Stochastic default models (copula-based, intensity process)
//! - [`correlation`]: Correlation structures for structured credit
//! - [`tree`]: Configuration for the stochastic pricer's tree mode
//! - [`pricer`]: Stochastic pricing engine with tree and Monte Carlo modes

pub(crate) mod calibrations;
pub(crate) mod correlation;
pub(crate) mod default;
pub(crate) mod prepayment;
pub(crate) mod pricer;
pub(crate) mod tree;

// Re-export main types (may be used by external bindings or tests)
pub use correlation::CorrelationStructure;
pub use default::{PoolGranularity, StochasticDefaultSpec};
pub use prepayment::StochasticPrepaySpec;
pub use pricer::{PricingMode, StochasticPricingResult, TranchePricingResult};
