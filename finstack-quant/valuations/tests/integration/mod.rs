//! Integration Tests Module
//!
//! This module organizes end-to-end integration tests by category:
//!
//! # Test Categories
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
//!
//! # Running Tests
//!
//! ```bash
//! # Run all integration tests
//! cargo nextest run -p finstack-quant-valuations --test valuations integration::
//!
//! # Run specific category
//! cargo nextest run -p finstack-quant-valuations --test valuations integration::metrics::
//! cargo nextest run -p finstack-quant-valuations --test valuations integration::serialization::
//! cargo nextest run -p finstack-quant-valuations --test valuations integration::schema::
//! ```

// Metrics framework tests
pub mod metrics;

// Serialization round-trip tests
pub mod serialization;

// Schema validation tests
pub mod schema;
