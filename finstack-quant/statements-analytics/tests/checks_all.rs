//! Cargo only discovers integration tests that are direct children of
//! `tests/`. Nested check modules are included here so they run.

#[path = "checks/mod.rs"]
mod checks;
