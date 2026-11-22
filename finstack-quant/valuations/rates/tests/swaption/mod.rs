//! Comprehensive swaption test suite (European and Bermudan).

pub mod common;
pub mod core;
pub mod edge_cases;
pub mod integration;
pub mod invariants;
pub mod market;
pub mod metrics;
pub mod pricing;

mod bermudan_pricing;
mod envelope;
