//! Generate every valuations JSON Schema and canonical instrument fixture.

use finstack_quant_core::schema::{
    deterministic_json_bytes, run_schema_generator, sync_generated_files, SchemaGenerationCommand,
};
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations::instruments::json_loader::instrument_registry;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = "tests/instruments/json_examples";

fn fixture_artifacts() -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut expected = BTreeMap::new();
    for entry in instrument_registry() {
        let examples = entry.examples().map_err(|error| {
            Error::Internal(format!(
                "build canonical {} instrument fixture: {error}",
                entry.tag
            ))
        })?;
        let [example] = examples.as_slice() else {
            return Err(Error::Internal(format!(
                "{} must provide exactly one canonical fixture example",
                entry.tag
            )));
        };
        let path = PathBuf::from(entry.fixture_path);
        if expected
            .insert(path.clone(), deterministic_json_bytes(example)?)
            .is_some()
        {
            return Err(Error::Validation(format!(
                "duplicate instrument fixture path {}",
                path.display()
            )));
        }
    }
    Ok(expected)
}

fn main() -> Result<()> {
    let command = SchemaGenerationCommand::from_env()?;
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = fixture_artifacts()?;
    let schemas = finstack_quant_valuations::schema::artifacts();
    run_schema_generator(
        manifest_dir,
        &["schemas/common", "schemas/instruments", "schemas/results"],
        &schemas,
        Some(std::path::Path::new("schemas/index.json")),
        &command,
    )?;
    sync_generated_files(
        command.output_root.as_deref().unwrap_or(manifest_dir),
        Path::new(FIXTURE_ROOT),
        &fixtures,
        ".json",
        "fixture",
        command.mode,
    )
}
