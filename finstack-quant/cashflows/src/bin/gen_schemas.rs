//! Generate the JSON Schemas owned by `finstack-quant-cashflows`.

use finstack_quant_cashflows::schema::ARTIFACTS;
use finstack_quant_core::schema::{run_schema_generator, SchemaGenerationCommand};
use std::path::Path;

fn main() {
    let command = SchemaGenerationCommand::from_env()
        .unwrap_or_else(|error| panic!("parse schema generator arguments: {error}"));
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    run_schema_generator(
        manifest_dir,
        &["schemas/cashflow"],
        ARTIFACTS,
        Some(std::path::Path::new("schemas/index.json")),
        &command,
    )
    .unwrap_or_else(|error| panic!("generate cashflow schemas: {error}"));
}
