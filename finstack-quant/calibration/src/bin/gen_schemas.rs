//! Generate checked-in calibration and market-quote schemas.

use finstack_quant_core::schema::{run_schema_generator, SchemaGenerationCommand};
use std::path::Path;

fn main() -> finstack_quant_core::Result<()> {
    let command = SchemaGenerationCommand::from_env()?;
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let artifacts = finstack_quant_calibration::json_schema::artifacts();
    run_schema_generator(
        manifest_dir,
        &["schemas/calibration", "schemas/market"],
        artifacts,
        Some(std::path::Path::new("schemas/index.json")),
        &command,
    )
}
