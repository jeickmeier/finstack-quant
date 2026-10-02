//! Generate checked-in JSON Schemas owned by `finstack-quant-core`.

use std::path::Path;

use finstack_quant_core::schema::{
    run_schema_generator, run_schema_index_generator, SchemaArtifact, SchemaGenerationCommand,
    ARTIFACTS,
};

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
    for root in ROOTS {
        let owned: Vec<SchemaArtifact> = ARTIFACTS
            .iter()
            .filter(|artifact| Path::new(artifact.relative_path).starts_with(root))
            .copied()
            .collect();
        run_schema_generator(manifest_dir, Path::new(root), &owned, &command)
            .unwrap_or_else(|error| panic!("generate core schemas under {root}: {error}"));
    }
    run_schema_index_generator(
        manifest_dir,
        Path::new("schemas/index.json"),
        ARTIFACTS,
        &command,
    )
    .unwrap_or_else(|error| panic!("generate core schema index: {error}"));
}
