//! FX Spot instrument comprehensive test suite.
//!
//! Test organization follows market-standard practices:
//!
//! - `common`: Shared fixtures and utilities for DRY test setup
//! - Unit tests: Individual component testing
//!   - `construction`: Builder patterns, conventions, validation
//!   - `pricing`: NPV calculation, spot rate, FX matrix usage
//!   - `cashflows`: Settlement date calculation and schedule generation
//! - Metrics tests: All metric calculator validation
//!   - `base_amount`: Base currency notional
//!   - `quote_amount`: Quote currency PV
//!   - `spot_rate`: Realized spot rate calculation
//!   - `inverse_rate`: Inverse spot rate
//!   - `dv01`: FX sensitivity to rates
//!   - `theta`: Time decay
//! - Integration tests: End-to-end workflows
//!   - `pricer`: Pricer registry integration
//!   - `market_standards`: Industry benchmark validation
//! - Edge cases: Boundary conditions and special scenarios

mod common;

// Unit tests
mod cashflows;
mod construction;
mod pricing;

// Metrics tests
mod metrics;

// Integration tests
mod integration;

// Edge cases
mod edge_cases;
