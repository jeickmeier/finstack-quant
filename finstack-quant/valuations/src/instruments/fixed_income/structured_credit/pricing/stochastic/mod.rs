//! Structured-credit stochastic pricing orchestration and calibration presets.
//!
//! This module provides stochastic prepayment and default models with:
//! - Factor-driven CPR/CDR models with correlation
//! - Industry-standard calibrations (RMBS, CLO, CMBS)
//! - Stochastic pricing with tree and Monte Carlo modes
//!
//! # Module Organization
//!
//! - [`calibrations`]: Standard calibration constants for RMBS, CLO, CMBS
//! - [`tree`]: Configuration for the stochastic pricer's tree mode
//! - [`pricer`]: Stochastic pricing engine with tree and Monte Carlo modes

pub(crate) mod calibrations;
pub(crate) mod pricer;
pub(crate) mod tree;

pub use pricer::{StochasticPricingResult, StructuredCreditPricingMode, TranchePricingResult};
