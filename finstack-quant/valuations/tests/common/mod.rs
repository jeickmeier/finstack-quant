//! Equity-option builders for the umbrella test suite.
//!
//! Market-context builders, fixtures, tolerances and assertions live in
//! `finstack_quant_valuations_test_support`; this module keeps only the
//! helpers that construct an `EquityOption`.

// Allow unused items in test utilities - they're available for future tests
#![allow(dead_code, unused_imports)]

pub mod builders;

pub use builders::*;
