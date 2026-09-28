//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "attribution.rs"]
mod attribution;
#[path = "credit_carry_split.rs"]
mod credit_carry_split;
#[path = "cross_factor_attribution_tests.rs"]
mod cross_factor_attribution_tests;
#[path = "economic_factors.rs"]
mod economic_factors;
#[path = "market_restore.rs"]
mod market_restore;
