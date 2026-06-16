//! Serialization module integration tests.
//!
//! This test suite verifies:
//! - Wire format stability (golden tests)
//! - Roundtrip serialization correctness
//! - Wire format stability
//!
//! # Test Organization
//!
//! - [`golden`]: Wire format stability tests (serde golden tests)
//! - [`roundtrip`]: Roundtrip serialization tests for various types

#[path = "serde/golden.rs"]
mod golden;

#[path = "serde/roundtrip.rs"]
mod roundtrip;
