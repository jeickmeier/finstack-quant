//! Deterministic JSON Schema assembly helpers.

mod artifacts;
/// Shared diagnostics for compiled JSON Schema validators.
#[cfg(feature = "jsonschema-validate")]
pub mod diagnostics;
mod externalize;
mod generator;
mod llm;
mod registry;
#[cfg(test)]
mod tests;
#[cfg(feature = "jsonschema-validate")]
mod validator;

pub use artifacts::ARTIFACTS;
pub use externalize::{ExternalSchemaDefinition, externalize_schema_definitions};
pub use generator::{
    SCHEMA_INDEX_VERSION, SchemaGenerationCommand, SchemaGenerationMode, build_schema_index,
    deterministic_json_bytes, run_schema_generator, schema_index_row, sync_generated_files,
};
pub use llm::{DEFAULT_MAX_INLINE_BYTES, LlmProfile, RESOLVES_FROM_KEYWORD, project_llm};
pub use registry::{
    COMMON_SCHEMA_BASE, COMMON_SCHEMA_DEFINITIONS, JSON_SCHEMA_DIALECT, SchemaArtifact, SchemaKind,
    SerdeSchema, example, example_from_json, find_schema_artifact,
};
#[cfg(feature = "jsonschema-validate")]
pub use validator::{
    ValidatorCache, build_trimmed_validator, collect_external_refs, format_validation_result,
    reachable_resource_ids,
};
