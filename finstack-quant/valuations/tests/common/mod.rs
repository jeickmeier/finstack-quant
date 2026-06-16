//! Shared test utilities for risk tests.
//!
//! This module provides:
//! - **`tolerances`**: Canonical tolerance constants for numerical comparisons
//! - **`assertions`**: Assertion helpers with better error messages
//! - **`builders`**: Market context and option builders for common test scenarios
//! - **`fixtures`**: Common test fixtures (dates, curves, contexts)
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::common::{
//!     tolerances, assertions::assert_approx_eq,
//!     builders::{test_market, test_option},
//!     fixtures::base_date,
//! };
//!
//! let as_of = base_date();
//! let market = test_market(as_of).spot(100.0).vol(0.25).rate(0.05).build();
//! let option = test_option(as_of.saturating_add(time::Duration::days(365)))
//!     .strike(100.0)
//!     .build();
//!
//! assert_approx_eq(result, expected, tolerances::STANDARD);
//! ```
//!
//! # Module Organization
//!
//! - For tolerance values, always use `tolerances::*` (the canonical source)
//! - For custom assertions, use `assertions::*`
//! - For building test markets/instruments, use `builders::*`
//! - For standard dates and curves, use `fixtures::*`

// Allow unused items in test utilities - they're available for future tests
#![allow(dead_code, unused_imports)]

pub mod assertions;
pub mod builders;
pub mod fixtures;
pub mod test_utils;
pub mod tolerances;

pub use assertions::*;
pub use builders::*;
pub use fixtures::*;
pub use test_utils::*;
pub use tolerances::*;
