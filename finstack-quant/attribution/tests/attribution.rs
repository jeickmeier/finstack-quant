//! P&L attribution test suite entry point.
//!
//! This module consolidates tests for:
//!
//! - **bond_attribution**: Bond P&L attribution (carry, roll, spread, rate)
//! - **fx_attribution**: FX P&L attribution (spot, forward, basis)
//! - **metrics_based_convexity**: Convexity P&L attribution tests
//! - **scalars_attribution**: Scalar market data attribution tests
//! - **serialization_roundtrip**: JSON roundtrip tests for attribution types
//!
//! Run all attribution tests:
//! ```bash
//! cargo nextest run -p finstack-quant-attribution --test attribution
//! ```

#[allow(dead_code)]
#[path = "support/attribution_test_utils.rs"]
pub(crate) mod attribution_support;

#[path = "attribution/mod.rs"]
#[allow(clippy::module_inception)]
mod attribution;
