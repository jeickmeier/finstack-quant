//! Comprehensive inflation swap test suite.
//!
//! Test organization:
//! - `fixtures`: Shared test data, market builders, helpers
//! - `construction`: Builder validation, field constraints
//! - `pricing`: Core pricing methodology (legs, par rate, NPV)
//! - `edge_cases`: Boundary conditions, extreme values, matured swaps
//! - `metrics`: Individual metric calculator tests
//! - `integration`: End-to-end workflows, lag policies

mod construction;
mod edge_cases;
pub mod fixtures;
mod integration;
mod metrics;
mod pricing;
