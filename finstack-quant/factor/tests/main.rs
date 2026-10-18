//! Integration tests for this crate.
//!
//! Every test file is a module of this one target so the crate links once
//! instead of once per file.

#[path = "canonical_contract.rs"]
mod canonical_contract;
#[path = "credit_calibration.rs"]
mod credit_calibration;
#[path = "credit_decomposition.rs"]
mod credit_decomposition;
#[path = "credit_peel_parity.rs"]
mod credit_peel_parity;
#[path = "host_bridges.rs"]
mod host_bridges;
#[path = "multi_asset_config.rs"]
mod multi_asset_config;
#[cfg(feature = "json-schema")]
#[path = "schema_contract.rs"]
mod schema_contract;
#[path = "strictness.rs"]
mod strictness;
