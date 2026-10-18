//! Comprehensive inflation swap test suite.
//!
//! Test organization:
//! - fixtures: `finstack_quant_valuations_test_support::inflation` (curves, index, markets)
//! - `construction`: Builder validation, field constraints
//! - `pricing`: Core pricing methodology (legs, par rate, NPV)
//! - `edge_cases`: Boundary conditions, extreme values, matured swaps
//! - `metrics`: Individual metric calculator tests
//! - `integration`: End-to-end workflows, lag policies

mod construction;
mod edge_cases;
mod integration;
mod metrics;
mod pricing;
