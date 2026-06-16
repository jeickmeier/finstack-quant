//! Integration test suite entry point.
//!
//! This module consolidates end-to-end integration tests including:
//! - Full regression tests
//! - FX settlement tests
//! - Metrics strict mode tests
//! - Golden test vectors
//! - Serialization roundtrips
//! - Schema parity tests
//! - TypeScript export tests
//!
//! Run all integration tests:
//! ```bash
//! cargo test --test integration
//! ```

#[path = "integration/mod.rs"]
mod integration;
