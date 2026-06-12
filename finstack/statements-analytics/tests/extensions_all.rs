//! Extensions all tests for statements-analytics.

// Extension tests.
//
// Note: Cargo only discovers integration tests that are direct children of `tests/`.
// This file wires in the nested extensions test modules so they run.

#[path = "extensions/extensions_full_execution_tests.rs"]
mod extensions_full_execution_tests;
