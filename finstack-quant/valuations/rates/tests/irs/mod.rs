//! Comprehensive Interest Rate Swap test suite.
//!
//! # Test Organization
//!
//! - `construction`: IRS builder patterns, validation, edge cases
//! - `cashflows`: Fixed/floating leg schedule generation and validation
//! - `pricing`: Core pricing engine, NPV calculation, theta
//! - `metrics`: Individual metric calculator tests (annuity, DV01, par rate, etc.)
//! - `validation`: Market benchmark validation (Hull, ISDA standards)
//! - `integration`: Complex scenarios (basis swaps, off-market swaps, multi-curve)

mod cashflows;
mod construction;
mod integration;
mod metrics;
mod pricing;
mod proptests;
mod test_compounding_accuracy;
mod test_swap_pricing;
mod test_swap_symmetry;
mod validation;
