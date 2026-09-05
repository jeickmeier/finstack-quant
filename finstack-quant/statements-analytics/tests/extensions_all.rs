//! Cargo only discovers integration tests that are direct children of
//! `tests/`. Nested extension modules are included here so they run.

#[path = "extensions/extensions_full_execution_tests.rs"]
mod extensions_full_execution_tests;
