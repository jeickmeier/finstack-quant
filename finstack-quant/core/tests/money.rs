//! Money and currency types integration tests.
//!
//! This test suite verifies correctness for:
//! - Money type operations and currency safety
//! - FX provider implementations
//! - Rounding contexts
//!
//! # Test Organization
//!
//! - `money_fx`: FX conversion tests
//! - `rounding`: Rounding context tests

#[path = "money/money_fx.rs"]
mod money_fx;

#[path = "money/rounding.rs"]
mod rounding;
