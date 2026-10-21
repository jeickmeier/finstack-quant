//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "audit_regressions.rs"]
mod audit_regressions;
/// Cashflow builder, schedule generation, amortization, and credit model tests.
#[path = "cashflows/builder/mod.rs"]
mod builder;
#[path = "coupon_spec_strictness.rs"]
mod coupon_spec_strictness;
/// Shared tolerances for rates, factors, and amounts.
#[path = "cashflows/helpers.rs"]
mod helpers;
#[path = "principal_credit_regressions.rs"]
mod principal_credit_regressions;
#[path = "production_fixing_roll.rs"]
mod production_fixing_roll;
/// JSON schema serialization roundtrip tests.
#[path = "cashflows/schema_roundtrip.rs"]
mod schema_roundtrip;
