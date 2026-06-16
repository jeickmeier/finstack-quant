//! Comprehensive unit tests for common instruments module.
//!
//! Organized by:
//! - metrics: Risk metrics and calculations
//! - parameters: Parameter types and conventions
//! - helpers: Utility functions and test fixtures
//! - test_helpers: Shared test utilities and fixtures
//! - parity: Tolerance-based comparison for validating against QuantLib, Bloomberg, etc.

pub mod helpers;
pub mod metrics;
#[macro_use]
pub mod parity;
pub mod parameters;
pub mod pricer;
pub mod test_discountable;
pub mod test_helpers;
pub mod test_pricing;
pub mod test_rates_credit_tree;
