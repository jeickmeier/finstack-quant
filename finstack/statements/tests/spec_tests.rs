//! Spec tests (NodeId, FinancialModelSpec, registry, serialization).
//!
//! Note: Cargo only discovers integration tests that are direct children of `tests/`.
//! This file wires in the nested spec test modules so they run.

#[path = "spec/model_spec_tests.rs"]
mod model_spec_tests;

#[path = "spec/registry_tests.rs"]
mod registry_tests;

#[path = "spec/results_export_tests.rs"]
mod results_export_tests;

#[path = "spec/serialization_tests.rs"]
mod serialization_tests;
