//! Unified cashflow generation engine for revolving credit facilities.
//!
//! This module consolidates cashflow generation logic for both deterministic and stochastic modes,
//! producing `CashFlowSchedule` objects with optional embedded path data from 3-factor Monte Carlo simulations.
//!
//! # Architecture
//!
//! - **Deterministic Mode**: Generates cashflows from pre-defined draw/repay events
//! - **Stochastic Mode**: Generates cashflows from utilization/rate/spread trajectories
//! - **Unified Output**: Both modes produce `CashFlowSchedule` for consistent downstream processing
//!
//! # Three-Factor Model
//!
//! For stochastic paths, the engine processes trajectories from three correlated factors:
//! - **Utilization**: Mean-reverting usage rate of the facility
//! - **Short Rate**: Interest rate dynamics (fixed or floating)
//! - **Credit Spread**: Default risk premium

mod deterministic;
mod engine;
mod fees;
mod stochastic;
#[cfg(test)]
mod tests;

pub use engine::{
    CashflowEngine, PathAwareCashflowSchedule, ThreeFactorPathData, calculate_drawn_balance_at_date,
};
