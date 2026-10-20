//! Stochastic pricing engine for structured credit.
//!
//! This module provides Monte Carlo pricing for structured credit instruments
//! with stochastic prepayment and default models: each scenario path draws
//! monthly systematic factors from a seeded Philox substream, optionally
//! paired antithetically, and runs the full deal waterfall.
//!
//! # Features
//!
//! - Path-dependent state tracking (burnout, cumulative losses)
//! - Tranche-level cashflow generation
//! - Multi-currency support via FX rates
//! - Risk metric computation (EL, UL, ES)

mod config;
mod engine;

pub(crate) use config::StochasticPricerConfig;
pub(crate) use engine::StochasticPricer;
