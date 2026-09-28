//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "audit_accrual.rs"]
mod audit_accrual;
#[path = "audit_regressions.rs"]
mod audit_regressions;
#[path = "builder_all.rs"]
mod builder_all;
#[path = "canonical_contract.rs"]
mod canonical_contract;
#[path = "capital_structure_builder.rs"]
mod capital_structure_builder;
#[path = "capital_structure_integration.rs"]
mod capital_structure_integration;
#[path = "checks_all.rs"]
mod checks_all;
#[path = "dsl_all.rs"]
mod dsl_all;
#[path = "evaluator_engine.rs"]
mod evaluator_engine;
#[path = "evaluator_tests.rs"]
mod evaluator_tests;
#[path = "feature_completeness_tests.rs"]
mod feature_completeness_tests;
#[path = "forecast_all.rs"]
mod forecast_all;
#[path = "functions_all.rs"]
mod functions_all;
#[path = "integration_all.rs"]
mod integration_all;
#[path = "market_standards_tests.rs"]
mod market_standards_tests;
#[path = "production_reporting_audit.rs"]
mod production_reporting_audit;
#[path = "proptest_evaluator.rs"]
mod proptest_evaluator;
#[path = "registry_dynamic.rs"]
mod registry_dynamic;
#[path = "schema_contract.rs"]
mod schema_contract;
#[path = "spec_tests.rs"]
mod spec_tests;
