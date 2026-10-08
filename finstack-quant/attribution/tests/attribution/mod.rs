//! P&L attribution integration tests, one module per feature area.

#[allow(dead_code)]
#[path = "../support/attribution_test_utils.rs"]
pub(crate) mod attribution_support;

mod analytical_self_consistency;
mod audit_steps;
mod bond_attribution;
mod carry_credit_factor;
mod carry_decomposition_window;
mod convertible_attribution;
mod coverage_gaps;
mod credit_factor_linear;
mod credit_factor_waterfall_parallel;
mod credit_schema_contract;
mod factors_snapshot;
mod fx_attribution;
mod invariants;
mod metrics_based_convexity;
mod no_model;
mod quantlib_parity;
mod return_contribution;
mod rounding_policy;
mod scalars_attribution;
mod schema_contract;
mod serialization_roundtrip;
mod spec_tests;
mod taylor_wing_buckets;
mod types_pnl;
mod vol_factor_attribution;
