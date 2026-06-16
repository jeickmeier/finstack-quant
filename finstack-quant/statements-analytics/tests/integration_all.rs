//! Integration all tests for statements-analytics.

// Integration tests for analytics components.
//
// Note: Cargo only discovers integration tests that are direct children of `tests/`.
// This file wires in the nested integration test modules so they run.

#[path = "integration/real_estate_template_tests.rs"]
mod real_estate_template_tests;

#[path = "integration/patterns_tests.rs"]
mod patterns_tests;
