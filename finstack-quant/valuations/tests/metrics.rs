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
//! cargo test --test metrics
//! ```

// ============================================================================
// Shared Test Utilities
// ============================================================================

/// Common test utilities: fixtures, tolerances, assertions, builders
#[path = "common/mod.rs"]
mod common;

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod finstack_quant_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

// ============================================================================
// Metrics Tests
// ============================================================================

#[path = "metrics/mod.rs"]
mod metrics;
