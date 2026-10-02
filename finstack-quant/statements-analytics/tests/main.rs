//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "analysis_boundary_regressions.rs"]
mod analysis_boundary_regressions;
#[path = "analysis_corporate.rs"]
mod analysis_corporate;
#[path = "analysis_ecl.rs"]
mod analysis_ecl;
#[path = "analysis_goal_seek.rs"]
mod analysis_goal_seek;
#[path = "analysis_monte_carlo.rs"]
mod analysis_monte_carlo;
#[path = "analysis_orchestrator.rs"]
mod analysis_orchestrator;
#[path = "analysis_scenario_set.rs"]
mod analysis_scenario_set;
#[cfg(feature = "json-schema")]
#[path = "analysis_schema_contract.rs"]
mod analysis_schema_contract;
#[path = "checks_all.rs"]
mod checks_all;
#[path = "extensions_all.rs"]
mod extensions_all;
#[path = "extensions_scorecards.rs"]
mod extensions_scorecards;
#[path = "forecast_all.rs"]
mod forecast_all;
#[path = "integration_all.rs"]
mod integration_all;
#[path = "production_valuation_audit.rs"]
mod production_valuation_audit;
