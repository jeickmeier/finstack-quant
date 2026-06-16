//! FRA (Forward Rate Agreement) comprehensive test suite.
//!
//! This module provides >80% coverage of the FRA implementation with
//! market-standard tests organized into logical categories.
//!
//! ## Organization
//!
//! - `common` - Shared fixtures, builders, and assertion helpers
//! - `construction` - FRA creation and builder pattern tests
//! - `pricing` - NPV calculation and settlement adjustment tests
//! - `cashflows` - Cashflow schedule generation tests
//! - `metrics/` - Comprehensive metric calculator tests
//! - `validation/` - Market-standard validation tests
//! - `quantlib_parity` - QuantLib parity tests for cross-validation
//!
//! ## Total Coverage: ~80 tests
//!
//! See README.md for detailed test inventory.

mod cashflows;
mod common;
mod construction;
mod metrics;
mod pricing;
mod quantlib_parity;
mod validation;
