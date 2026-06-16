//! Golden tests for serialization stability and external parity.
//!
//! This module contains tests that verify:
//! - Wire format stability (serialization doesn't change)
//! - End-to-end correctness (evaluation produces consistent results)
//! - Parity with external tools (Excel, pandas, QuantLib)
//!
//! # Framework
//!
//! Tests use the unified `finstack_quant_test_utils::golden` framework for consistency.
//!
//! # Suites
//!
//! - `basic_model.json`: Model spec for serialization tests
//! - `basic_model_results.json`: Expected evaluation results
//! - `excel/`: Excel parity test vectors (being migrated to JSON)
//! - `pandas/`: pandas parity test vectors (being migrated to JSON)

mod golden_parity;
mod golden_tests;

// Re-export core golden types for use in tests
#[allow(unused_imports)]
pub use finstack_quant_test_utils::golden::{
    assert_abs, assert_expected_value, load_suite_from_path, load_suite_from_str, Expectation,
    ExpectedValue, GoldenAssert, GoldenSuite, SuiteMeta, Tolerance,
};
