//! Generate the checked-in JSON Schemas under `finstack-quant/models/schemas`.
//!
//! Two families: `schemas/factor_model` (factor-model configuration and
//! calibration contracts) and `schemas/models` (every other model contract
//! the bindings exchange). One index covers both.
//!
//! The directory belongs to `finstack-quant-models`, but some of its artifacts
//! are owned by `finstack-quant-factor`, which `finstack-quant-models` must not
//! depend on. This umbrella crate is the one place that sees both registries,
//! so the generator lives here and writes into the models crate's directory.

use std::path::Path;

use finstack_quant_core::schema::{run_schema_generator, SchemaArtifact, SchemaGenerationCommand};

fn main() {
    let command = SchemaGenerationCommand::from_env()
        .unwrap_or_else(|error| panic!("parse schema generator arguments: {error}"));
    let models_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("models");
    let factor = finstack_quant_factor::schema::ARTIFACTS;
    let models: Vec<SchemaArtifact> = finstack_quant_models::schema::ARTIFACTS
        .iter()
        .chain(finstack_quant_factor::schema::MODELS_ARTIFACTS)
        .copied()
        .collect();
    let all: Vec<SchemaArtifact> = factor.iter().chain(&models).copied().collect();
    run_schema_generator(
        &models_dir,
        &["schemas/factor_model", "schemas/models"],
        &all,
        Some(std::path::Path::new("schemas/index.json")),
        &command,
    )
    .unwrap_or_else(|error| panic!("generate models schemas: {error}"));
}
