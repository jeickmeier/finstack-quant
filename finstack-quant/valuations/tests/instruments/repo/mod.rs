//! Comprehensive test suite for Repo instruments.
//!
//! This test suite is organized into logical modules to ensure >80% coverage
//! and adherence to market standards for financial instrument testing.
//!
//! Test Organization:
//! - `fixtures`: Common test data and utilities
//! - `construction`: Builder patterns, factory methods, validation
//! - `collateral`: Collateral types, valuation, adequacy checks
//! - `pricing`: Present value, interest calculations, discounting
//! - `cashflows`: Cashflow schedule generation and validation
//! - `metrics`: Comprehensive metric calculator tests
//! - `edge_cases`: Boundary conditions, error handling, corner cases

mod fixtures;

mod cashflows;
mod collateral;
mod construction;
mod edge_cases;
mod margin;
mod metrics;
mod pricing;
