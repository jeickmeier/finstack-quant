//! Integration tests for P&L attribution.
//!
//! ## Test Modules
//!
//! - `audit_steps`: Exported endpoints and step rows rebuild the reported buckets
//! - `bond_attribution`: Basic bond P&L attribution tests
//! - `fx_attribution`: FX translation and waterfall attribution tests
//! - `invariants`: Mathematical invariants (sign conventions, scaling, edge cases)
//! - `metrics_based_convexity`: Second-order metrics support
//! - `analytical_self_consistency`: Validates attribution against the library's
//!   own analytical DV01 / Convexity formulas (self-consistency, NOT a true
//!   external QuantLib parity — that is a separate effort).
//! - `scalars_attribution`: Market scalars extraction/restoration
//! - `serialization_roundtrip`: JSON serialization tests
//! - `spec_tests`: Attribution spec validation tests
//! - `rounding_policy`: Rounding policy stamping tests

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
