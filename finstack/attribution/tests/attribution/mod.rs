//! Integration tests for P&L attribution.
//!
//! ## Test Modules
//!
//! - `bond_attribution`: Basic bond P&L attribution tests
//! - `fx_attribution`: FX translation and waterfall attribution tests
//! - `invariants`: Mathematical invariants (sign conventions, scaling, edge cases)
//! - `metrics_based_convexity`: Second-order metrics support
//! - `model_params_attribution`: Prepayment, default, recovery attribution
//! - `analytical_self_consistency`: Validates attribution against the library's
//!   own analytical DV01 / Convexity formulas (self-consistency, NOT a true
//!   external QuantLib parity — that is a separate effort).
//! - `scalars_attribution`: Market scalars extraction/restoration
//! - `serialization_roundtrip`: JSON serialization tests
//! - `spec_tests`: Attribution spec validation tests
//! - `rounding_policy`: Rounding policy stamping tests

mod analytical_self_consistency;
mod bond_attribution;
mod carry_credit_factor;
mod convertible_attribution;
mod credit_factor_linear;
mod credit_factor_waterfall_parallel;
mod dump_baseline;
mod factors_snapshot;
mod fx_attribution;
mod invariants;
mod metrics_based_convexity;
mod model_params_attribution;
mod no_model_compatibility;
mod quantlib_parity;
mod rounding_policy;
mod scalars_attribution;
mod serialization_roundtrip;
mod spec_tests;
mod types_pnl;
mod vol_factor_attribution;
