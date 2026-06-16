//! Plan-driven calibration API.
//!
//! Provides the schema-first interface for defining and executing calibration
//! exercises. This is the primary entry point for external systems (e.g.,
//! via JSON/YAML) to interact with the calibration module.
//!
//! # Submodules
//! - `schema`: Definition of calibration plans, steps, and envelopes.
//! - `engine`: Execution logic for processing calibration plans.

pub mod context_builder;
pub mod engine;
pub mod errors;
pub mod market_datum;
pub mod prior_market;
pub mod schema;
pub mod validate;
