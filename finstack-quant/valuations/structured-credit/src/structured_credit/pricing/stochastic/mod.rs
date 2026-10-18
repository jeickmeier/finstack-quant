//! Structured-credit stochastic pricing orchestration and calibration presets.
//!
//! This module provides stochastic prepayment and default models with:
//! - Factor-driven CPR/CDR models with correlation
//! - Industry-standard calibrations (RMBS, CLO, CMBS)
//! - Monte Carlo pricing through the deal waterfall
//!
//! # Module Organization
//!
//! - [`calibrations`]: Standard calibration constants for RMBS, CLO, CMBS
//! - [`tree`]: Scenario configuration shared by setup and the pricer
//! - [`pricer`]: Monte Carlo pricing engine

pub(crate) mod calibrations;
pub(crate) mod pricer;
pub(crate) mod tree;

pub use pricer::{StochasticPricingResult, StructuredCreditPricingMode, TranchePricingResult};
