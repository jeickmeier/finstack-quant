//! Equity-option builders for the umbrella test suite.
//!
//! Market-context builders, fixtures, tolerances and assertions live in
//! `finstack_quant_valuations_test_support`; `direct_equity_greek` lives in
//! `equity/tests/support/greeks.rs` and is path-included here.

// Allow unused items in test utilities - they're available for future tests
#![allow(dead_code, unused_imports)]

pub mod builders;
#[path = "../../equity/tests/support/greeks.rs"]
pub mod greeks;

pub use builders::*;
pub use greeks::direct_equity_greek;
