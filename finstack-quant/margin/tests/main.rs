//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "audit_regressions.rs"]
mod audit_regressions;
#[path = "frtb_sba_charges.rs"]
mod frtb_sba_charges;
#[path = "production_simm_csa_audit.rs"]
mod production_simm_csa_audit;
#[path = "regulatory_determinism.rs"]
mod regulatory_determinism;
#[path = "schema_parity.rs"]
mod schema_parity;
#[path = "simm_schedule_parity.rs"]
mod simm_schedule_parity;
