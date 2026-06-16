//! DSL parser and compiler tests.
//!
//! Note: Cargo only discovers integration tests that are direct children of `tests/`.
//! This file wires in the nested DSL test modules so they run.

#[path = "dsl/dsl_tests.rs"]
mod dsl_tests;

#[path = "dsl/capital_structure_dsl_tests.rs"]
mod capital_structure_dsl_tests;

#[path = "dsl/proptest_dsl.rs"]
mod proptest_dsl;
