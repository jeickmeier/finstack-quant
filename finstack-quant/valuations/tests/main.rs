//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "canonical_contracts.rs"]
mod canonical_contracts;
#[path = "cashflow_export_schema.rs"]
mod cashflow_export_schema;
#[path = "cashflows.rs"]
mod cashflows;
#[path = "credit_calibration.rs"]
mod credit_calibration;
#[path = "credit_decomposition.rs"]
mod credit_decomposition;
#[path = "cross_factor_metrics_tests.rs"]
mod cross_factor_metrics_tests;
#[path = "default_attribute_consistency.rs"]
mod default_attribute_consistency;
#[path = "golden.rs"]
mod golden;
#[path = "instruments.rs"]
mod instruments;
#[path = "integration.rs"]
mod integration;
#[path = "metric_request_contract.rs"]
mod metric_request_contract;
#[path = "metrics.rs"]
mod metrics;
#[path = "phase2_strictness.rs"]
mod phase2_strictness;
#[path = "production_audit.rs"]
mod production_audit;
#[path = "production_barrier_ndf_audit.rs"]
mod production_barrier_ndf_audit;
#[path = "production_inflation_audit.rs"]
mod production_inflation_audit;
#[path = "production_risk_audit.rs"]
mod production_risk_audit;
#[path = "production_simm_sensitivity_audit.rs"]
mod production_simm_sensitivity_audit;
#[path = "production_structured_audit.rs"]
mod production_structured_audit;
#[path = "production_structured_metrics_audit.rs"]
mod production_structured_metrics_audit;
#[path = "production_waterfall_audit.rs"]
mod production_waterfall_audit;
#[path = "return_floor_example.rs"]
mod return_floor_example;
#[path = "sanity_invariants.rs"]
mod sanity_invariants;
#[path = "schema_audit.rs"]
mod schema_audit;
