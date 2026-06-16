//! Schema Validation Tests
//!
//! Tests ensuring schema files stay synchronized with Rust types:
//!
//! - [`parity`]: JSON schema ↔ Rust type synchronization
//! - [`credit_factor_model`]: Credit factor model schema validation tests (PR-9)
//! - [`ts_export`]: TypeScript type generation (requires `ts_export` feature)

pub mod credit_factor_model;
pub mod parity;
#[cfg(feature = "ts_export")]
pub mod ts_export;
