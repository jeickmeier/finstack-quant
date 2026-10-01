//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "canonical_contract.rs"]
mod canonical_contract;
#[path = "par_cds_bump.rs"]
mod par_cds_bump;
#[path = "production_historical_roll_audit.rs"]
mod production_historical_roll_audit;
#[path = "report_serialization.rs"]
mod report_serialization;
#[path = "mod.rs"]
mod scenario_modules;
#[path = "schema_contract.rs"]
mod schema_contract;
#[path = "senior_review_market_regressions.rs"]
mod senior_review_market_regressions;
#[path = "spec_validation_test.rs"]
mod spec_validation_test;
#[path = "templates_integration.rs"]
mod templates_integration;
