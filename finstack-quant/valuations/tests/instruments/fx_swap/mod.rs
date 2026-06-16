#![cfg(test)]

//! Comprehensive FX Swap test suite following market standards.
//!
//! This module provides thorough test coverage for FX swap pricing, metrics,
//! and edge cases, organized into logical submodules for maintainability.
//!
//! Test organization:
//! - `fixtures`: Common test data and market setup
//! - `pricing`: Core valuation tests (PV, contract rates, edge cases)
//! - `metrics`: Individual metric calculator tests
//! - `integration`: Multi-metric and scenario tests
//! - `edge_cases`: Boundary conditions and error handling

mod edge_cases;
mod fixtures;
mod integration;
mod metrics;
mod pricing;
mod test_fx_swap;
