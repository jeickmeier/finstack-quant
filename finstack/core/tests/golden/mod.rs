//! Golden tests using the unified framework.
//!
//! This module contains tests that load expected values from JSON fixtures
//! and compare them against computed results using the `finstack_test_utils::golden` framework.
//!
//! ## Available Test Suites
//!
//! - `variance_tests`: Realized variance estimator tests (Parkinson, Garman-Klass)
//! - `daycount_quantlib_tests`: QuantLib parity tests for day count conventions
//! - `vol_models_quantlib_tests`: QuantLib parity tests for volatility models
//!   (Black-76, BSM, Bachelier, SABR, Heston, SVI, rough Heston)

mod daycount_quantlib_tests;
mod variance_tests;
mod vol_models_quantlib_tests;
