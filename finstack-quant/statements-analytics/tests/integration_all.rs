//! Cargo only discovers integration tests that are direct children of
//! `tests/`. Nested modules are included here so they run.

#[path = "integration/real_estate_template_tests.rs"]
mod real_estate_template_tests;

#[path = "integration/patterns_tests.rs"]
mod patterns_tests;

#[path = "integration/real_estate_statements_e2e.rs"]
mod real_estate_statements_e2e;
