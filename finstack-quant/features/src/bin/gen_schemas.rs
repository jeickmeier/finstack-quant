//! Generate checked-in JSON Schemas owned by the features crate.

use std::path::Path;

use finstack_quant_core::schema::{
    run_schema_generator, run_schema_index_generator, SchemaGenerationCommand,
};
use finstack_quant_features::schema::ARTIFACTS;

fn main() {
    let command = SchemaGenerationCommand::from_env()
        .unwrap_or_else(|error| panic!("parse schema generator arguments: {error}"));
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    run_schema_generator(
        manifest_dir,
        Path::new("schemas/features"),
        ARTIFACTS,
        &command,
    )
    .unwrap_or_else(|error| panic!("generate features schemas: {error}"));
    run_schema_index_generator(
        manifest_dir,
        Path::new("schemas/index.json"),
        ARTIFACTS,
        &command,
    )
    .unwrap_or_else(|error| panic!("generate features schema index: {error}"));
}
