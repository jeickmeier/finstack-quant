//! Risk metrics: VaR, ES, and stress testing.
//!
//! This module provides Historical Value-at-Risk (VaR) calculation capabilities
//! using historical simulation methodology. It supports both full revaluation
//! and Taylor approximation approaches.

pub(crate) mod hvar;
pub(crate) mod market_history;
pub(crate) mod risk_factors;
pub(crate) mod var_calculator;

pub use hvar::{GenericExpectedShortfall, GenericHVar};
pub use market_history::{MarketHistory, MarketScenario, RiskFactorShift};
pub use risk_factors::{extract_risk_factors, RiskFactorType};
pub use var_calculator::{calculate_var, VarConfig, VarMethod, VarResult};
