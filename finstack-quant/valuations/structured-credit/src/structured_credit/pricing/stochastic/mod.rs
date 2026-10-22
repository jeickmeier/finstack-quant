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
//! - [`pricer`]: Monte Carlo pricing engine and its scenario configuration

pub(crate) mod calibrations;
pub(crate) mod pricer;
