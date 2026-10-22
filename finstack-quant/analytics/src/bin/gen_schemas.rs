//! Generate checked-in JSON Schemas owned by the analytics crate.

use std::path::Path;

use finstack_quant_analytics::schema::ARTIFACTS;
use finstack_quant_core::schema::{run_schema_generator, SchemaGenerationCommand};

fn main() {
    let command = SchemaGenerationCommand::from_env()
        .unwrap_or_else(|error| panic!("parse schema generator arguments: {error}"));
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    run_schema_generator(
        manifest_dir,
        &["schemas/analytics"],
        ARTIFACTS,
        Some(std::path::Path::new("schemas/index.json")),
        &command,
    )
    .unwrap_or_else(|error| panic!("generate analytics schemas: {error}"));
}
