//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "audit_regressions.rs"]
mod audit_regressions;
#[path = "cashflows.rs"]
mod cashflows;
#[path = "coupon_spec_strictness.rs"]
mod coupon_spec_strictness;
#[path = "principal_credit_regressions.rs"]
mod principal_credit_regressions;
#[path = "production_fixing_roll.rs"]
mod production_fixing_roll;
