//! Schema generator CLI plumbing: filesystem walking, index generation and
//! deterministic byte rendering.
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use super::registry::*;
use crate::{Error, Result};

/// Generation operation selected by a schema generator CLI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaGenerationMode {
    /// Write expected artifacts and remove stale schemas in the owned root.
    Write,
    /// Compare expected artifacts without mutating the filesystem.
    Check,
    /// Print the deterministic artifact path inventory.
    List,
}

/// Parsed command for a registry-driven schema generator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaGenerationCommand {
    /// Selected generation operation.
    pub mode: SchemaGenerationMode,
    /// Optional root used instead of the crate manifest directory.
    pub output_root: Option<PathBuf>,
}

impl SchemaGenerationCommand {
    /// Parse a schema generator command from process arguments.
    ///
    /// Exactly one of `--write`, `--check`, or `--list` is required.
    /// `--output-root PATH` redirects all crate-relative artifacts, enabling
    /// clean-room reproducibility checks.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for missing, conflicting, or unknown
    /// arguments.
    pub fn from_env() -> Result<Self> {
        Self::parse(std::env::args().skip(1))
    }

    /// Parse a schema generator command from an argument iterator.
    ///
    /// # Arguments
    ///
    /// * `arguments` - CLI arguments excluding the executable name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for missing, conflicting, or unknown
    /// arguments.
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut mode = None;
        let mut output_root = None;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let selected = match argument.as_str() {
                "--write" => Some(SchemaGenerationMode::Write),
                "--check" => Some(SchemaGenerationMode::Check),
                "--list" => Some(SchemaGenerationMode::List),
                "--output-root" => {
                    let path = arguments.next().ok_or_else(|| {
                        Error::Validation("--output-root requires a path".to_string())
                    })?;
                    if output_root.replace(PathBuf::from(path)).is_some() {
                        return Err(Error::Validation(
                            "--output-root may be supplied only once".to_string(),
                        ));
                    }
                    None
                }
                _ => {
                    return Err(Error::Validation(format!(
                        "unknown schema generator argument {argument:?}"
                    )))
                }
            };
            if let Some(selected) = selected {
                if mode.replace(selected).is_some() {
                    return Err(Error::Validation(
                        "choose exactly one of --write, --check, or --list".to_string(),
                    ));
                }
            }
        }
        let mode = mode.ok_or_else(|| {
            Error::Validation("choose one of --write, --check, or --list".to_string())
        })?;
        Ok(Self { mode, output_root })
    }
}

/// Generate a crate's schema artifacts and index from one rendered document set.
///
/// All paths and documents are validated before writing. Extra schema files are
/// removed only from `owned_roots`; unrelated directories remain untouched.
/// List mode prints sorted artifact paths without rendering or listing the index.
///
/// # Arguments
///
/// * `manifest_dir` - Owning crate's absolute Cargo manifest directory.
/// * `owned_roots` - Non-overlapping crate-relative schema directories owned by
///   the registry, such as `schemas/dates` and `schemas/market_data`.
/// * `artifacts` - Complete registry whose documents and metadata form the index.
/// * `index_relative_path` - Optional crate-relative index destination. `None`
///   emits artifacts only, as required by the WASM host-contract generator.
/// * `command` - Generation mode and optional replacement output root.
///
/// # Errors
///
/// Returns an error for invalid or duplicate paths/IDs, overlapping roots,
/// generation or filesystem failures, or check-mode drift.
pub fn run_schema_generator(
    manifest_dir: &Path,
    owned_roots: &[&str],
    artifacts: &[SchemaArtifact],
    index_relative_path: Option<&Path>,
    command: &SchemaGenerationCommand,
) -> Result<()> {
    let roots: Vec<&Path> = owned_roots.iter().map(Path::new).collect();
    for (index, root) in roots.iter().enumerate() {
        validate_owned_root(root)?;
        if roots[..index]
            .iter()
            .any(|other| root.starts_with(other) || other.starts_with(root))
        {
            return Err(Error::Validation(
                "schema roots must not overlap".to_string(),
            ));
        }
    }
    let base = command.output_root.as_deref().unwrap_or(manifest_dir);
    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for artifact in artifacts {
        let path = Path::new(artifact.relative_path);
        let root = roots
            .iter()
            .find(|root| path.starts_with(root))
            .ok_or_else(|| {
                Error::Validation(format!(
                    "schema artifact is outside the owned roots: {}",
                    path.display()
                ))
            })?;
        validate_generated_path(path, root, ".schema.json", "schema")?;
        if !ids.insert(artifact.id) {
            return Err(Error::Validation(format!(
                "duplicate schema id in registry: {}",
                artifact.id
            )));
        }
        if !paths.insert(path.to_path_buf()) {
            return Err(Error::Validation(format!(
                "duplicate schema path in registry: {}",
                path.display()
            )));
        }
    }
    if let Some(index_path) = index_relative_path {
        validate_relative_path(index_path, "schema index path")?;
        if paths.contains(index_path) {
            return Err(Error::Validation(
                "schema index overlaps an artifact path".to_string(),
            ));
        }
    }
    if command.mode == SchemaGenerationMode::List {
        for path in paths {
            println!("{}", path.display());
        }
        return Ok(());
    }

    let mut expected = BTreeMap::new();
    let mut rows = BTreeMap::new();
    for artifact in artifacts {
        let document = artifact.generate()?;
        let bytes = deterministic_json_bytes(&document)?;
        if index_relative_path.is_some() {
            rows.insert(
                artifact.relative_path,
                Value::Object(index_row(artifact, bytes.len())),
            );
        }
        expected.insert(PathBuf::from(artifact.relative_path), bytes);
    }
    if let Some(index_path) = index_relative_path {
        expected.insert(
            index_path.to_path_buf(),
            deterministic_json_bytes(&index_document(rows))?,
        );
    }
    let mut extra = BTreeSet::new();
    for root in &roots {
        extra.extend(
            generated_files(&base.join(root), base, ".schema.json")?
                .into_iter()
                .filter(|path| !expected.contains_key(path)),
        );
    }
    sync_outputs(
        base,
        &expected,
        &extra.into_iter().collect::<Vec<_>>(),
        "schema",
        command.mode,
    )?;
    if command.mode == SchemaGenerationMode::Write {
        for root in roots {
            let directory = base.join(root);
            remove_empty_directories(&directory, &directory)?;
        }
    }
    Ok(())
}

/// Synchronize one owned directory of generated files with their expected bytes.
///
/// The shared check/write/list loop behind every schema generator and the
/// valuations instrument-fixture generator. List mode prints the expected
/// paths. Check mode is non-mutating and reports missing, changed, and extra
/// files. Write mode rewrites only changed files, removes extras below
/// `owned_root`, and prunes the directories that become empty.
///
/// # Arguments
///
/// * `base` - Directory every path is relative to: the crate manifest
///   directory or the command's output root.
/// * `owned_root` - `base`-relative directory exclusively owned by this
///   generator; files in it that are not expected are extras.
/// * `expected` - Complete `base`-relative file set with its rendered bytes;
///   every path must lie below `owned_root` and end with `suffix`.
/// * `suffix` - File-name suffix of the generated files (`".schema.json"`,
///   `".json"`); only files with it count as owned.
/// * `label` - Noun used in messages, such as `"schema"` or `"fixture"`.
/// * `mode` - Operation to perform.
///
/// # Errors
///
/// Returns an error for an invalid path, a filesystem failure, or check-mode
/// drift.
pub fn sync_generated_files(
    base: &Path,
    owned_root: &Path,
    expected: &BTreeMap<PathBuf, Vec<u8>>,
    suffix: &str,
    label: &str,
    mode: SchemaGenerationMode,
) -> Result<()> {
    validate_relative_path(owned_root, &format!("owned {label} root"))?;
    for relative_path in expected.keys() {
        validate_generated_path(relative_path, owned_root, suffix, label)?;
    }
    if mode == SchemaGenerationMode::List {
        for path in expected.keys() {
            println!("{}", path.display());
        }
        return Ok(());
    }

    let owned_directory = base.join(owned_root);
    let actual = generated_files(&owned_directory, base, suffix)?;
    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();
    let extra = actual
        .difference(&expected_paths)
        .cloned()
        .collect::<Vec<_>>();

    sync_outputs(base, expected, &extra, label, mode)?;
    if mode == SchemaGenerationMode::Write {
        remove_empty_directories(&owned_directory, &owned_directory)?;
    }
    Ok(())
}

/// Compare or write an already validated file set and remove only known extras.
fn sync_outputs(
    base: &Path,
    expected: &BTreeMap<PathBuf, Vec<u8>>,
    extra: &[PathBuf],
    label: &str,
    mode: SchemaGenerationMode,
) -> Result<()> {
    match mode {
        SchemaGenerationMode::Check => {
            let mut drift = Vec::new();
            for (relative_path, rendered) in expected {
                let path = base.join(relative_path);
                match std::fs::read(&path) {
                    Ok(actual) if actual == *rendered => {}
                    Ok(_) => drift.push(format!("changed {}", relative_path.display())),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        drift.push(format!("missing {}", relative_path.display()));
                    }
                    Err(error) => {
                        return Err(Error::Internal(format!(
                            "read {label} {}: {error}",
                            path.display()
                        )))
                    }
                }
            }
            drift.extend(extra.iter().map(|path| format!("extra {}", path.display())));
            if drift.is_empty() {
                Ok(())
            } else {
                Err(Error::Validation(format!(
                    "{label} artifacts are not current:\n{}",
                    drift.join("\n")
                )))
            }
        }
        SchemaGenerationMode::Write => {
            for (relative_path, rendered) in expected {
                let path = base.join(relative_path);
                if std::fs::read(&path).ok().as_deref() == Some(rendered.as_slice()) {
                    continue;
                }
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        Error::Internal(format!(
                            "create {label} directory {}: {error}",
                            parent.display()
                        ))
                    })?;
                }
                std::fs::write(&path, rendered).map_err(|error| {
                    Error::Internal(format!("write {label} {}: {error}", path.display()))
                })?;
                println!("updated {}", path.display());
            }
            for relative_path in extra {
                let path = base.join(relative_path);
                std::fs::remove_file(&path).map_err(|error| {
                    Error::Internal(format!("remove stale {label} {}: {error}", path.display()))
                })?;
                println!("removed stale {}", path.display());
            }
            Ok(())
        }
        SchemaGenerationMode::List => Ok(()),
    }
}

/// Version stamped into every generated schema index.
pub const SCHEMA_INDEX_VERSION: u64 = 1;

/// Build the schema index document for one crate's registry.
///
/// Rows are sorted by artifact path, so the document is stable regardless of
/// registration order.
///
/// # Arguments
///
/// * `artifacts` - Complete registry for the owning crate.
///
/// # Errors
///
/// Returns [`Error::Internal`] if an artifact cannot be generated or rendered.
pub fn build_schema_index(artifacts: &[SchemaArtifact]) -> Result<Value> {
    let mut rows = BTreeMap::new();
    for artifact in artifacts {
        let row = schema_index_row(artifact, &artifact.generate()?)?;
        if rows
            .insert(artifact.relative_path, Value::Object(row))
            .is_some()
        {
            return Err(Error::Validation(format!(
                "duplicate schema path in index: {}",
                artifact.relative_path
            )));
        }
    }

    Ok(index_document(rows))
}

fn index_document(rows: BTreeMap<&str, Value>) -> Value {
    let mut document = Map::new();
    document.insert(
        "artifacts".to_string(),
        Value::Array(rows.into_values().collect()),
    );
    document.insert(
        "schema_index_version".to_string(),
        Value::Number(serde_json::Number::from(SCHEMA_INDEX_VERSION)),
    );
    Value::Object(document)
}

/// Render canonical metadata for a schema index row.
///
/// Besides the display `title`, the row carries `type_name`: the Rust type
/// name of the root contract type (its `schemars` schema name).
///
/// # Arguments
///
/// * `artifact` - Registry entry supplying the row's identity and description.
/// * `rendered` - Canonical generated schema; byte size includes pretty JSON and
///   the trailing newline emitted by the checked-in schema generator.
///
/// # Errors
///
/// Returns an error if the schema cannot be encoded as JSON.
pub fn schema_index_row(artifact: &SchemaArtifact, rendered: &Value) -> Result<Map<String, Value>> {
    let bytes = deterministic_json_bytes(rendered)?;
    Ok(index_row(artifact, bytes.len()))
}

fn index_row(artifact: &SchemaArtifact, byte_count: usize) -> Map<String, Value> {
    let mut row = Map::new();
    row.insert("$id".to_string(), Value::String(artifact.id.to_string()));
    row.insert(
        "bytes".to_string(),
        Value::Number(serde_json::Number::from(byte_count)),
    );
    row.insert(
        "kind".to_string(),
        Value::String(artifact.kind.as_str().to_string()),
    );
    row.insert(
        "path".to_string(),
        Value::String(artifact.relative_path.to_string()),
    );
    row.insert(
        "summary".to_string(),
        Value::String(artifact.index_summary().to_string()),
    );
    row.insert(
        "title".to_string(),
        Value::String(artifact.title.to_string()),
    );
    row.insert(
        "type_name".to_string(),
        Value::String(artifact.type_name().into_owned()),
    );
    row
}

/// Render a JSON value with recursively sorted object keys, UTF-8 encoding,
/// two-space indentation, LF line endings, and one final newline.
///
/// # Arguments
///
/// * `value` - JSON value to render deterministically.
///
/// # Errors
///
/// Returns [`Error::Internal`] if serialization fails.
pub fn deterministic_json_bytes(value: &Value) -> Result<Vec<u8>> {
    // `serde_json::Map` is a `BTreeMap` (`preserve_order` is off), so object
    // keys are already byte-ordered; see `canonical::tests::map_keys_iterate_sorted`.
    let mut json = serde_json::to_vec_pretty(value)
        .map_err(|error| Error::Internal(format!("serialize schema: {error}")))?;
    json.push(b'\n');
    Ok(json)
}
pub(super) fn validate_relative_path(path: &Path, label: &str) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::Validation(format!(
            "{label} must be a non-empty normalized relative path: {}",
            path.display()
        )));
    }
    Ok(())
}

pub(super) fn validate_owned_root(owned_root: &Path) -> Result<()> {
    validate_relative_path(owned_root, "owned schema root")?;
    if owned_root.components().next() != Some(Component::Normal("schemas".as_ref()))
        || owned_root.components().count() < 2
    {
        return Err(Error::Validation(format!(
            "owned schema root must be a specific directory below schemas/: {}",
            owned_root.display()
        )));
    }
    Ok(())
}

fn validate_generated_path(
    path: &Path,
    owned_root: &Path,
    suffix: &str,
    label: &str,
) -> Result<()> {
    validate_relative_path(path, &format!("{label} artifact path"))?;
    if !path.starts_with(owned_root)
        || !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
    {
        return Err(Error::Validation(format!(
            "{label} artifact must be a {suffix} file below {}: {}",
            owned_root.display(),
            path.display()
        )));
    }
    Ok(())
}

fn generated_files(directory: &Path, base: &Path, suffix: &str) -> Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    if !directory.exists() {
        return Ok(files);
    }
    collect_generated_files(directory, base, suffix, &mut files)?;
    Ok(files)
}

fn collect_generated_files(
    directory: &Path,
    base: &Path,
    suffix: &str,
    files: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let entries = std::fs::read_dir(directory).map_err(|error| {
        Error::Internal(format!(
            "read schema directory {}: {error}",
            directory.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            Error::Internal(format!(
                "read schema entry in {}: {error}",
                directory.display()
            ))
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_generated_files(&path, base, suffix, files)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
        {
            let relative = path.strip_prefix(base).map_err(|error| {
                Error::Internal(format!("resolve schema path {}: {error}", path.display()))
            })?;
            files.insert(relative.to_path_buf());
        }
    }
    Ok(())
}

fn remove_empty_directories(directory: &Path, owned_root: &Path) -> Result<bool> {
    if !directory.exists() {
        return Ok(true);
    }
    for entry in std::fs::read_dir(directory).map_err(|error| {
        Error::Internal(format!(
            "read schema directory {}: {error}",
            directory.display()
        ))
    })? {
        let path = entry
            .map_err(|error| {
                Error::Internal(format!(
                    "read schema entry in {}: {error}",
                    directory.display()
                ))
            })?
            .path();
        if path.is_dir() {
            remove_empty_directories(&path, owned_root)?;
        }
    }
    let empty = std::fs::read_dir(directory)
        .map_err(|error| {
            Error::Internal(format!(
                "read schema directory {}: {error}",
                directory.display()
            ))
        })?
        .next()
        .is_none();
    if empty && directory != owned_root {
        std::fs::remove_dir(directory).map_err(|error| {
            Error::Internal(format!(
                "remove empty schema directory {}: {error}",
                directory.display()
            ))
        })?;
    }
    Ok(empty)
}
