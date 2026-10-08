//! Scenario configuration for stochastic structured-credit pricing.
//!
//! The production stochastic pricer owns path generation and valuation. This
//! module contains only the configuration shared by instrument setup and that
//! pricing engine.

mod config;

pub(crate) use config::ScenarioTreeConfig;
