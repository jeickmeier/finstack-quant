//! Generate every valuations JSON Schema and canonical instrument fixture.

use finstack_quant_core::schema::{
    deterministic_json_bytes, run_schema_generator, run_schema_index_generator,
    sync_generated_files, SchemaArtifact, SchemaGenerationCommand,
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

fn run_schema_registries(
    manifest_dir: &Path,
    command: &SchemaGenerationCommand,
    artifacts: Vec<SchemaArtifact>,
) -> Result<()> {
    let roots = ["schemas/common", "schemas/instruments", "schemas/results"];
    let mut registries = roots
        .into_iter()
        .map(|root| (root, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for artifact in artifacts {
        let root = roots
            .into_iter()
            .find(|root| Path::new(artifact.relative_path).starts_with(root))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "schema artifact is outside a registered valuations root: {}",
                    artifact.relative_path
                ))
            })?;
        registries
            .get_mut(root)
            .ok_or_else(|| Error::Internal(format!("missing schema registry for {root}")))?
            .push(artifact);
    }
    for (root, artifacts) in registries {
        run_schema_generator(manifest_dir, Path::new(root), &artifacts, command)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let command = SchemaGenerationCommand::from_env()?;
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = fixture_artifacts()?;
    let schemas = finstack_quant_valuations::schema::artifacts();
    run_schema_index_generator(
        manifest_dir,
        Path::new("schemas/index.json"),
        &schemas,
        &command,
    )?;
    run_schema_registries(manifest_dir, &command, schemas)?;
    sync_generated_files(
        command.output_root.as_deref().unwrap_or(manifest_dir),
        Path::new(FIXTURE_ROOT),
        &fixtures,
        ".json",
        "fixture",
        command.mode,
    )
}
