//! Golden tests for portfolio calculations.
//!
//! This module validates portfolio attribution and valuation calculations
//! using the `finstack_quant_test_utils::golden` framework.
//!
//! # Approach
//!
//! Portfolio tests primarily use **range expectations** since exact P&L values
//! depend on many implementation details. The tests validate:
//!
//! - **Direction**: Positive rate shock -> negative rates P&L
//! - **Magnitude**: Results within reasonable bounds
//! - **Finiteness**: No NaN or infinite values

mod attribution_tests;

// Re-export core golden types
#[allow(unused_imports)]
pub use finstack_quant_test_utils::golden::{
    assert_range, load_suite_from_path, Expectation, GoldenAssert, GoldenSuite, SuiteMeta,
};
