//! Formula function tests.
//!
//! Note: Cargo only discovers integration tests that are direct children of `tests/`.
//! This file wires in the nested functions test modules so they run.

#[path = "functions/custom_functions_tests.rs"]
mod custom_functions_tests;

#[path = "functions/min_max_tests.rs"]
mod min_max_tests;

#[path = "functions/nan_handling_tests.rs"]
mod nan_handling_tests;
