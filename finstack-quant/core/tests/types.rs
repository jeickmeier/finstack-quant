//! Core financial types integration tests.
//!
//! This test suite verifies correctness for:
//! - Rate types and conversions (Rate, Bps)
//! - Arithmetic operations on rate types
//! - Cross-type conversions
//!
//! # Test Organization
//!
//! - `rates`: Rate type tests (Rate, Bps)

#[path = "types/rates.rs"]
mod rates;
