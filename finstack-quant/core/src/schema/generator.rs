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

/// Execute a complete registry-driven schema generation operation.
///
/// The expected artifact set is built in memory before any writes occur.
/// Check mode is non-mutating and reports missing, changed, and extra schema
/// files. Write mode removes extras only below `owned_root` after both the root
/// and every artifact path have passed strict relative-path validation.
///
/// # Arguments
///
/// * `manifest_dir` - Owning crate's absolute Cargo manifest directory.
/// * `owned_root` - Crate-relative schema directory exclusively owned by this
///   registry, such as `schemas/cashflow`.
/// * `artifacts` - Complete set of generated schemas for the owned root.
/// * `command` - Explicit generation mode and optional output root.
///
/// # Errors
///
/// Returns an error for invalid paths, duplicate registry entries, generation
/// failures, filesystem failures, or check-mode drift.
pub fn run_schema_generator(
    manifest_dir: &Path,
    owned_root: &Path,
    artifacts: &[SchemaArtifact],
    command: &SchemaGenerationCommand,
) -> Result<()> {
    validate_owned_root(owned_root)?;
    let base = command.output_root.as_deref().unwrap_or(manifest_dir);
    let mut expected = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for artifact in artifacts {
        let relative_path = Path::new(artifact.relative_path);
        validate_artifact_path(relative_path, owned_root)?;
        if !ids.insert(artifact.id) {
            return Err(Error::Validation(format!(
                "duplicate schema id in registry: {}",
                artifact.id
            )));
        }
        let rendered = deterministic_json_bytes(&artifact.generate()?)?;
        if expected
            .insert(relative_path.to_path_buf(), rendered)
            .is_some()
        {
            return Err(Error::Validation(format!(
                "duplicate schema path in registry: {}",
                relative_path.display()
            )));
        }
    }

    if command.mode == SchemaGenerationMode::List {
        for path in expected.keys() {
            println!("{}", path.display());
        }
        return Ok(());
    }

    let owned_directory = base.join(owned_root);
    let actual = schema_files(&owned_directory, base)?;
    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();
    let extra = actual
        .difference(&expected_paths)
        .cloned()
        .collect::<Vec<_>>();

    match command.mode {
        SchemaGenerationMode::Check => {
            let mut drift = Vec::new();
            for (relative_path, rendered) in &expected {
                let path = base.join(relative_path);
                match std::fs::read(&path) {
                    Ok(actual) if actual == *rendered => {}
                    Ok(_) => drift.push(format!("changed {}", relative_path.display())),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        drift.push(format!("missing {}", relative_path.display()));
                    }
                    Err(error) => {
                        return Err(Error::Internal(format!(
                            "read schema {}: {error}",
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
                    "schema artifacts are not current:\n{}",
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
                            "create schema directory {}: {error}",
                            parent.display()
                        ))
                    })?;
                }
                std::fs::write(&path, rendered).map_err(|error| {
                    Error::Internal(format!("write schema {}: {error}", path.display()))
                })?;
                println!("updated {}", path.display());
            }
            for relative_path in extra {
                let path = base.join(&relative_path);
                std::fs::remove_file(&path).map_err(|error| {
                    Error::Internal(format!("remove stale schema {}: {error}", path.display()))
                })?;
                println!("removed stale {}", path.display());
            }
            remove_empty_directories(&owned_directory, &owned_directory)?;
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

    let mut document = Map::new();
    document.insert(
        "artifacts".to_string(),
        Value::Array(rows.into_values().collect()),
    );
    document.insert(
        "schema_index_version".to_string(),
        Value::Number(serde_json::Number::from(SCHEMA_INDEX_VERSION)),
    );
    Ok(Value::Object(document))
}

/// Render canonical metadata for a schema index row.
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
    let mut row = Map::new();
    row.insert("$id".to_string(), Value::String(artifact.id.to_string()));
    row.insert(
        "bytes".to_string(),
        Value::Number(serde_json::Number::from(bytes.len())),
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
    Ok(row)
}

/// Write or verify the schema index for one crate's registry.
///
/// The index is what a non-Rust consumer reads first: it names every contract
/// the crate publishes, whether the caller authors or reads it, and how large
/// the document is. Call this once per generator with the crate's complete
/// artifact list, even when the artifacts span several owned roots.
///
/// `--write` writes the index and `--check` byte-compares it. `--list`
/// deliberately does **not** name it: that listing is the *schema artifact*
/// inventory and is required to be sorted, and one index per crate cannot sit
/// in sorted position relative to every crate's root directory name. The index
/// is instead reconstructed from each generator's crate by
/// `scripts/check_schema_generation.py`, which is what compares the generated
/// tree against the registry.
///
/// # Arguments
///
/// * `manifest_dir` - Owning crate's manifest directory.
/// * `index_relative_path` - Index destination relative to the crate.
/// * `artifacts` - Complete registry for the owning crate.
/// * `command` - Parsed generator command selecting the mode and output root.
///
/// # Errors
///
/// Returns [`Error::Validation`] when the path escapes the crate or the
/// checked-in index has drifted, and [`Error::Internal`] on IO failure.
pub fn run_schema_index_generator(
    manifest_dir: &Path,
    index_relative_path: &Path,
    artifacts: &[SchemaArtifact],
    command: &SchemaGenerationCommand,
) -> Result<()> {
    validate_relative_path(index_relative_path, "schema index path")?;
    if command.mode == SchemaGenerationMode::List {
        return Ok(());
    }

    let rendered = deterministic_json_bytes(&build_schema_index(artifacts)?)?;
    let base = command.output_root.as_deref().unwrap_or(manifest_dir);
    let path = base.join(index_relative_path);

    match command.mode {
        SchemaGenerationMode::Check => match std::fs::read(&path) {
            Ok(actual) if actual == rendered => Ok(()),
            Ok(_) => Err(Error::Validation(format!(
                "schema index is not current: changed {}",
                index_relative_path.display()
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(Error::Validation(format!(
                    "schema index is not current: missing {}",
                    index_relative_path.display()
                )))
            }
            Err(error) => Err(Error::Internal(format!(
                "read schema index {}: {error}",
                path.display()
            ))),
        },
        SchemaGenerationMode::Write => {
            if std::fs::read(&path).ok().as_deref() == Some(rendered.as_slice()) {
                return Ok(());
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    Error::Internal(format!(
                        "create schema index directory {}: {error}",
                        parent.display()
                    ))
                })?;
            }
            std::fs::write(&path, rendered).map_err(|error| {
                Error::Internal(format!("write schema index {}: {error}", path.display()))
            })?;
            println!("updated {}", path.display());
            Ok(())
        }
        SchemaGenerationMode::List => Ok(()),
    }
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

pub(super) fn validate_artifact_path(path: &Path, owned_root: &Path) -> Result<()> {
    validate_relative_path(path, "schema artifact path")?;
    if !path.starts_with(owned_root)
        || path.extension().and_then(|extension| extension.to_str()) != Some("json")
        || !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".schema.json"))
    {
        return Err(Error::Validation(format!(
            "schema artifact must be a .schema.json file below {}: {}",
            owned_root.display(),
            path.display()
        )));
    }
    Ok(())
}

pub(super) fn schema_files(directory: &Path, base: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    if !directory.exists() {
        return Ok(files);
    }
    collect_schema_files(directory, base, &mut files)?;
    Ok(files)
}

pub(super) fn collect_schema_files(
    directory: &Path,
    base: &Path,
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
            collect_schema_files(&path, base, files)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".schema.json"))
        {
            let relative = path.strip_prefix(base).map_err(|error| {
                Error::Internal(format!("resolve schema path {}: {error}", path.display()))
            })?;
            files.insert(relative.to_path_buf());
        }
    }
    Ok(())
}

pub(super) fn remove_empty_directories(directory: &Path, owned_root: &Path) -> Result<bool> {
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
