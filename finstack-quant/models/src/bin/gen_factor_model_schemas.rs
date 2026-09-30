//! Generate checked-in JSON Schemas owned by the models crate.
//!
//! Two families: `schemas/factor_model` (factor-model configuration and
//! calibration contracts) and `schemas/models` (every other model contract
//! the bindings exchange). One index covers both.

use std::path::Path;

use finstack_quant_core::schema::{
    run_schema_generator, run_schema_index_generator, SchemaArtifact, SchemaGenerationCommand,
};

fn main() {
    let command = SchemaGenerationCommand::from_env()
        .unwrap_or_else(|error| panic!("parse schema generator arguments: {error}"));
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let factor = finstack_quant_models::factor::schema::ARTIFACTS;
    let models = finstack_quant_models::schema::ARTIFACTS;
    run_schema_generator(
        manifest_dir,
        Path::new("schemas/factor_model"),
        factor,
        &command,
    )
    .unwrap_or_else(|error| panic!("generate factor-model schemas: {error}"));
    run_schema_generator(manifest_dir, Path::new("schemas/models"), models, &command)
        .unwrap_or_else(|error| panic!("generate model schemas: {error}"));
    let all: Vec<SchemaArtifact> = factor.iter().chain(models).copied().collect();
    run_schema_index_generator(
        manifest_dir,
        Path::new("schemas/index.json"),
        &all,
        &command,
    )
    .unwrap_or_else(|error| panic!("generate models schema index: {error}"));
}
