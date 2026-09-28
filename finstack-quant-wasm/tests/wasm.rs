//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "return_shapes.rs"]
mod return_shapes;
#[path = "wasm_analytics.rs"]
mod wasm_analytics;
#[path = "wasm_attribution.rs"]
mod wasm_attribution;
#[path = "wasm_cashflows.rs"]
mod wasm_cashflows;
#[path = "wasm_core_market_data.rs"]
mod wasm_core_market_data;
#[path = "wasm_credit_factor_hierarchy.rs"]
mod wasm_credit_factor_hierarchy;
#[path = "wasm_features.rs"]
mod wasm_features;
#[path = "wasm_fixed_income.rs"]
mod wasm_fixed_income;
#[path = "wasm_implied_vol.rs"]
mod wasm_implied_vol;
#[path = "wasm_margin.rs"]
mod wasm_margin;
#[path = "wasm_math.rs"]
mod wasm_math;
#[path = "wasm_metric_keys.rs"]
mod wasm_metric_keys;
#[path = "wasm_models_liquidity.rs"]
mod wasm_models_liquidity;
#[path = "wasm_portfolio.rs"]
mod wasm_portfolio;
#[path = "wasm_scenarios.rs"]
mod wasm_scenarios;
#[path = "wasm_statements.rs"]
mod wasm_statements;
#[path = "wasm_statements_analytics.rs"]
mod wasm_statements_analytics;
#[path = "wasm_valuations.rs"]
mod wasm_valuations;
