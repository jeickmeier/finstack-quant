//! DSL parser and compiler tests.
//!
//! Registered by the single integration-test target in `main.rs`.

#[path = "dsl/dsl_tests.rs"]
mod dsl_tests;

#[path = "dsl/capital_structure_dsl_tests.rs"]
mod capital_structure_dsl_tests;

#[path = "dsl/proptest_dsl.rs"]
mod proptest_dsl;
