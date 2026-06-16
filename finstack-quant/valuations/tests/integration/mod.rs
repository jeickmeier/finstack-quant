//! Integration Tests Module
//!
//! This module organizes end-to-end integration tests by category:
//!
//! # Test Categories
//!
//! ## [`e2e`] - End-to-End Workflow Tests
//!
//! Complete workflow tests exercising multiple components:
//! - `bond_portfolio` - Multi-currency 100-bond portfolio pricing with metrics
//! - `fx_settlement` - FX spot date calculations with joint holiday calendars
//!
//! ## [`metrics`] - Metrics Framework Tests
//!
//! Tests for the metrics computation system:
//! - `strict_mode` - Error handling, unknown metric rejection, silent failure prevention
//!
//! ## [`serialization`] - Serialization Tests
//!
//! JSON round-trip tests ensuring data integrity:
//! - `instrument_roundtrip` - All instrument types serialize/deserialize correctly
//! - `result_roundtrip` - ValuationResult serialization (requires `serde` feature)
//!
//! ## [`schema`] - Schema Validation Tests
//!
//! Tests ensuring schema files stay synchronized:
//! - `parity` - JSON schema ↔ Rust type synchronization
//! - `ts_export` - TypeScript type generation (requires `ts_export` feature)
//!
//! # Running Tests
//!
//! ```bash
//! # Run all integration tests
//! cargo test --test integration
//!
//! # Run specific category
//! cargo test --test integration e2e::
//! cargo test --test integration metrics::
//! cargo test --test integration serialization::
//! cargo test --test integration schema::
//! ```

// End-to-end workflow tests
pub mod e2e;

// Metrics framework tests
pub mod metrics;

// Serialization round-trip tests
pub mod serialization;

// Schema validation tests
pub mod schema;
