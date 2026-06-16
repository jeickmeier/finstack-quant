//! Comprehensive swaption test suite organized by functionality.
//!
//! Test structure follows market standards for derivatives testing:
//! - pricing/: Model pricing validation (Black, SABR, components)
//! - metrics/: Greeks and risk metrics with numerical validation
//! - market/: Market data handling (vol surfaces, SABR calibration)
//! - integration/: End-to-end workflows and payer/receiver tests
//! - edge_cases/: Expiry conditions and numerical stability
//! - invariants: Mathematical property tests (parity, monotonicity, bounds)

pub mod core;
pub mod edge_cases;
pub mod integration;
pub mod invariants;
pub mod market;
pub mod metrics;
pub mod pricing;

/// Common test utilities and fixtures
pub mod common;
