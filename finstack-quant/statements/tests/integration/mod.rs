//! Integration tests for statements components.
//!
//! This module contains tests for:
//! - Money/AmountOrScalar dual-type support
//! - Term loan capital structure integration
//! - Roll-forward and vintage buildup patterns
//! - ECF sweep and waterfall execution

mod money_integration_tests;
mod patterns_tests;
mod term_loan_integration_tests;
mod waterfall_tests;
