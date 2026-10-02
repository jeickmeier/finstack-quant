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
#[path = "finance_accounting_regressions.rs"]
mod finance_accounting_regressions;
#[path = "finance_dimension_regressions.rs"]
mod finance_dimension_regressions;
#[path = "finance_engine_regressions.rs"]
mod finance_engine_regressions;
#[path = "finance_forecast_regressions.rs"]
mod finance_forecast_regressions;
#[path = "finance_model_regressions.rs"]
mod finance_model_regressions;
#[path = "finance_residual_regressions.rs"]
mod finance_residual_regressions;
#[path = "finance_temporal_regressions.rs"]
mod finance_temporal_regressions;
#[path = "finance_waterfall_regressions.rs"]
mod finance_waterfall_regressions;
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
#[path = "senior_review_builder.rs"]
mod senior_review_builder;
#[path = "senior_review_capital.rs"]
mod senior_review_capital;
#[path = "senior_review_dsl.rs"]
mod senior_review_dsl;
#[path = "senior_review_engine_checks.rs"]
mod senior_review_engine_checks;
#[path = "spec_tests.rs"]
mod spec_tests;
