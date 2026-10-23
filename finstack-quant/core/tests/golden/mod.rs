//! Golden tests using the unified framework.
//!
//! This module contains tests that load expected values from JSON fixtures
//! and compare them against computed results using the `finstack_quant_test_utils::golden` framework.
//!
//! ## Available Test Suites
//!
//! - `variance_tests`: Realized variance estimator tests (Parkinson,
//!   Garman-Klass, Rogers-Satchell, Yang-Zhang)
//! - `daycount_quantlib_tests`: QuantLib parity tests for day count conventions

mod daycount_quantlib_tests;
mod variance_tests;
