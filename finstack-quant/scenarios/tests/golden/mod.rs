//! Golden tests for scenario engine.
//!
//! This module validates scenario operations against known reference values
//! using the `finstack_quant_test_utils::golden` framework.

mod curve_shock_tests;

// Re-export core golden types for convenience
#[allow(unused_imports)]
pub use finstack_quant_test_utils::golden::{
    assert_expected_value, assert_range, load_suite_from_path, Expectation, ExpectedValue,
    GoldenAssert, GoldenSuite, SuiteMeta, Tolerance,
};
