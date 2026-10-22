//! Generate checked-in JSON Schemas owned by `finstack-quant-core`.

use std::path::Path;

use finstack_quant_core::schema::{run_schema_generator, SchemaGenerationCommand, ARTIFACTS};

/// Schema families owned by core; each is regenerated (and pruned) on its own.
const ROOTS: [&str; 4] = [
    "schemas/dates",
    "schemas/market_data",
    "schemas/rating_scales",
    "schemas/table",
];

fn main() {
    let command = SchemaGenerationCommand::from_env()
        .unwrap_or_else(|error| panic!("parse schema generator arguments: {error}"));
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    run_schema_generator(
        manifest_dir,
        &ROOTS,
        ARTIFACTS,
        Some(std::path::Path::new("schemas/index.json")),
        &command,
    )
    .unwrap_or_else(|error| panic!("generate core schemas: {error}"));
}
