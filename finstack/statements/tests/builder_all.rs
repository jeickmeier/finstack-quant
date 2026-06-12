//! Builder tests (ModelBuilder, MixedBuilder).
//!
//! Note: Cargo only discovers integration tests that are direct children of `tests/`.
//! This file wires in the nested builder test modules so they run.

#[path = "builder/builder_tests.rs"]
mod builder_tests;

#[path = "builder/mixed_builder_tests.rs"]
mod mixed_builder_tests;
