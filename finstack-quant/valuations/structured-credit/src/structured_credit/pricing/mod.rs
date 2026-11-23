//! Pricing and cashflow projection for structured credit instruments.
//!
//! This module contains pure functions for:
//! - Deterministic cashflow simulation
//! - Waterfall execution
//! - Coverage test evaluation
//! - Diversion rule processing
//! - Stochastic pricing

pub(crate) mod coverage;
pub(crate) mod resolve;
mod run;
pub(crate) mod simulation_engine;
pub(crate) use simulation_engine::SimulationRun;
pub(crate) mod stochastic;
pub(crate) mod waterfall;

#[cfg(test)]
pub(crate) use run::SIMULATION_RUNS;
