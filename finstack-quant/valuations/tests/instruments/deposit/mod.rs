//! Deposit instrument comprehensive test suite.
//!
//! Test organization follows market-standard practices:
//!
//! - `common`: Shared fixtures and utilities for DRY test setup
//! - Unit tests: Individual component testing
//!   - `construction`: Builder patterns, conventions, validation
//!   - `pricing`: NPV calculation, valuation scenarios
//! - Metrics tests: All metric calculator validation
//!   - `year_fraction`: Day count calculations
//!   - `discount_factors`: DF(start) and DF(end) calculations
//!   - `par_rate`: Par rate calculation and validation
//!   - `quote_rate`: Quoted rate handling
//!   - `dv01`: Interest rate sensitivity
//!   - `theta`: Time decay calculations
//! - Integration tests: End-to-end workflows
//!   - `cashflows`: Cashflow generation
//!   - `market_standards`: Industry benchmark validation
//! - Edge cases: Boundary conditions and special scenarios

mod common;

// Unit tests
mod construction;
mod pricing;

// Metrics tests
mod metrics;

// Integration tests
mod integration;

// Edge cases
mod edge_cases;
