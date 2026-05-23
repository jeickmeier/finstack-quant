//! P&L attribution test suite entry point.
//!
//! This module consolidates tests for:
//!
//! - **bond_attribution**: Bond P&L attribution (carry, roll, spread, rate)
//! - **fx_attribution**: FX P&L attribution (spot, forward, basis)
//! - **metrics_based_convexity**: Convexity P&L attribution tests
//! - **model_params_attribution**: Model parameter change attribution
//! - **scalars_attribution**: Scalar market data attribution tests
//! - **serialization_roundtrip**: JSON roundtrip tests for attribution types
//!
//! Run all attribution tests:
//! ```bash
//! cargo test --test attribution
//! ```

#[path = "../../valuations/tests/common/mod.rs"]
mod common;

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod finstack_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../valuations/tests/support/test_utils.rs"
    ));
}

#[path = "attribution/mod.rs"]
mod attribution;
