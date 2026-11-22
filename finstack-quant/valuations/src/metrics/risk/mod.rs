//! Risk metrics: VaR, ES, and historical scenario application.
//!
//! This module provides Historical Value-at-Risk (VaR) calculation capabilities
//! using historical simulation methodology. It supports both full revaluation
//! and Taylor approximation approaches.

mod hvar;
mod market_history;
pub mod var_calculator;

pub use finstack_quant_valuations_core::metrics::risk::{
    MarketHistory, MarketScenario, RiskFactorShift, RiskFactorType, UnshiftedHazardPolicy,
    VarConfig, VarMethod,
};
pub(crate) use hvar::register_risk_metrics;
pub use hvar::{GenericExpectedShortfall, GenericHVar};
pub(crate) use market_history::same_grid_coordinate;
pub use market_history::{apply_market_scenario, market_history_to_table};
