//! Check framework tests.
//!
//! Note: Cargo only discovers integration tests that are direct children of `tests/`.
//! This file wires in the nested check test modules so they run.

#[path = "checks/mod.rs"]
mod checks;
