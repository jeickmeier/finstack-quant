//! Core financial types integration tests.
//!
//! This test suite verifies correctness for:
//! - Rate types and conversions (Rate, Bps, Percentage)
//! - Arithmetic operations on rate types
//! - Cross-type conversions
//!
//! # Test Organization
//!
//! - `rates`: Rate type tests (Rate, Bps, Percentage)

#[path = "types/rates.rs"]
mod rates;
