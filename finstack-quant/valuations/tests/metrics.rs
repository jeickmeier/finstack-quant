//! Risk metrics test suite entry point.
//!
//! This module consolidates tests for:
//!
//! - **convergence**: Analytical vs finite difference Greek convergence
//! - **determinism**: Deterministic results for identical inputs
//! - **edge_cases**: Boundary conditions and degenerate cases
//! - **graceful_metrics_test**: Graceful failure handling for metric computation
//! - **greek_relationships**: Mathematical relationships between Greeks
//! - **invariants**: Property-based tests for metric invariants
//! - **sign_conventions**: Correct sign conventions for all Greeks
//! - **vanna_volga_pockets**: Vanna-volga smile interpolation tests
//!
//! Run all metrics tests:
//! ```bash
//! cargo nextest run -p finstack-quant-valuations --test valuations metrics::
//! ```

// Shared Test Utilities

/// Common test utilities: fixtures, tolerances, assertions, builders
#[path = "common/mod.rs"]
mod common;

// `instruments::test_support` already loads these files. A second `mod` in this
// crate is `clippy::duplicate_mod`.
#[allow(unused_imports)]
pub(crate) use crate::instruments::test_support::credit as credit_support;
#[allow(unused_imports)]
pub(crate) use crate::instruments::test_support::date as date_support;
#[allow(unused_imports)]
pub(crate) use crate::instruments::test_support::discount_forward_curves as discount_forward_curve_support;
#[allow(unused_imports)]
pub(crate) use crate::instruments::test_support::equity_fx_options as option_support;
#[allow(unused_imports)]
pub(crate) use crate::instruments::test_support::volatility as volatility_support;

// Metrics Tests

#[path = "metrics/mod.rs"]
#[allow(clippy::module_inception)]
mod metrics;
