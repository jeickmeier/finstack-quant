//! Integration tests for statements components.
//!
//! Note: Cargo only discovers integration tests that are direct children of `tests/`.
//! This file wires in the nested integration test modules so they run in CI.

#[path = "integration/dated_cashflow_export_tests.rs"]
mod dated_cashflow_export_tests;

#[path = "integration/money_integration_tests.rs"]
mod money_integration_tests;

#[path = "integration/term_loan_integration_tests.rs"]
mod term_loan_integration_tests;

#[path = "integration/waterfall_tests.rs"]
mod waterfall_tests;
