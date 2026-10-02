//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "audit_regressions.rs"]
mod audit_regressions;
#[path = "engine_conventions.rs"]
mod engine_conventions;
#[path = "integration.rs"]
mod integration;
#[cfg(feature = "json-schema")]
#[path = "schema_contract.rs"]
mod schema_contract;
#[path = "serialization.rs"]
mod serialization;
