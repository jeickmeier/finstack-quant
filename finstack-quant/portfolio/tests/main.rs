//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "common/mod.rs"]
mod common;

mod aggregation_grouping_and_df;
mod aggregation_metrics;
mod attribution_golden;
mod book_hierarchy_test;
mod core_portfolio_and_builder;
mod evaluation_plan;
mod factor_model_engines;
mod factor_model_serialization;
mod integration_scenarios;
mod margin_aggregation;
mod margin_serialization;
mod materialization;
mod materialization_benchmark_contract;
mod materialization_schema;
mod numerical_stability;
mod optimization_basic;
mod replay;
mod report_serialization;
mod result_schema;
mod scenario_view_names;
mod selective_repricing;
mod serialization;
mod test_optimization_fixes;
mod valuation_fallback;
mod valuation_fx;
mod valuations_integration;
mod weight_allocation;
