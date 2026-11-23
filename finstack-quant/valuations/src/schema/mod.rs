//! JSON-Schema helpers for Finstack Quant types.
//!
//! Schemas are generated on demand from the crate's serde-friendly types and
//! cached in-process for validation, UI forms, and contract generation.
//!
//! # Error Handling
//!
//! All schema accessors return `Result<&'static Value>` instead of panicking,
//! allowing callers to handle schema generation failures gracefully.

#[cfg(feature = "json-schema")]
mod common_examples;
#[cfg(feature = "json-schema")]
mod examples;
#[cfg(feature = "json-schema")]
mod registry;
#[cfg(test)]
mod tests;
mod validate;

#[cfg(feature = "json-schema")]
pub use registry::{artifacts, artifacts_slice};
pub use validate::instrument_types;
#[cfg(feature = "json-schema")]
pub use validate::{instrument_envelope_schema, instrument_schema, valuation_result_schema};
#[cfg(feature = "jsonschema-validate")]
pub use validate::{validate_instrument_envelope_json, validate_instrument_type_json};
