//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "audit_regressions.rs"]
mod audit_regressions;
#[path = "correctness_regressions.rs"]
mod correctness_regressions;
#[path = "correlation_validator_agreement.rs"]
mod correlation_validator_agreement;
#[path = "performance_smoke.rs"]
mod performance_smoke;
