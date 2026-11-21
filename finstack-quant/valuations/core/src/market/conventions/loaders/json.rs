//! Shared JSON registry loader helpers.

use finstack_quant_core::Error;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::{FxHashMap, FxHashSet};

const SUPPORTED_REGISTRY_SCHEMA_MAJOR: u32 = 2;

/// A registry JSON file containing entries with alias IDs.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegistryFile<R> {
    /// Schema identifier.
    pub(crate) schema: String,
    /// Namespace identifier.
    pub(crate) namespace: String,
    /// Version number.
    pub(crate) version: u32,
    /// Registry entries.
    pub(crate) entries: Vec<RegistryEntry<R>>,
}

impl<R> RegistryFile<R> {
    /// Validate registry metadata before consuming embedded convention data.
    pub(crate) fn validate_metadata(&self, registry_name: &str) -> Result<(), Error> {
        if self.schema.trim().is_empty() {
            return Err(Error::Validation(format!(
                "Embedded {registry_name} conventions registry has an empty `schema`"
            )));
        }

        let schema_major = parse_schema_major(&self.schema).ok_or_else(|| {
            Error::Validation(format!(
                "Embedded {registry_name} conventions registry has unsupported schema identifier `{}`",
                self.schema
            ))
        })?;
        if schema_major > SUPPORTED_REGISTRY_SCHEMA_MAJOR {
            return Err(Error::Validation(format!(
                "Embedded {registry_name} conventions registry schema major version {} exceeds supported version {}",
                schema_major, SUPPORTED_REGISTRY_SCHEMA_MAJOR
            )));
        }

        if self.namespace.trim().is_empty() {
            return Err(Error::Validation(format!(
                "Embedded {registry_name} conventions registry has an empty `namespace`"
            )));
        }

        if self.version == 0 {
            return Err(Error::Validation(format!(
                "Embedded {registry_name} conventions registry `version` must be positive"
            )));
        }

        Ok(())
    }
}

/// Parse a tenor string from an embedded convention registry with field context.
pub(crate) fn parse_tenor_field(
    raw: &str,
    field: &str,
    registry_name: &str,
) -> Result<Tenor, Error> {
    Tenor::parse(raw).map_err(|e| {
        Error::Validation(format!(
            "Invalid `{field}` in {registry_name} conventions registry: '{raw}': {e}"
        ))
    })
}

fn parse_schema_major(schema: &str) -> Option<u32> {
    schema.rsplit_once(".registry.v")?.1.parse().ok()
}

/// One registry record plus its set of alias IDs.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegistryEntry<R> {
    /// List of alias IDs.
    pub(crate) ids: Vec<String>,
    /// The record content.
    pub(crate) record: R,
}

/// Normalize a registry ID by trimming whitespace.
pub(crate) fn normalize_registry_id(id: &str) -> String {
    id.trim().to_string()
}

/// Parse a JSON convention registry, convert each record, and re-key using a domain ID wrapper.
///
/// This is the canonical helper for all simple convention loaders. It handles:
/// 1. Deserializing `RegistryFile<R>` from JSON
/// 2. Consuming each owned `R` record via `map_record` (which may return `Result<V>`)
/// 3. Re-keying from `String` to a typed domain ID via `make_id`
///
/// # Errors
///
/// Returns [`Error::Validation`] if JSON parsing fails, if any record conversion fails,
/// or if duplicate IDs are found after normalization.
pub(crate) fn parse_and_rekey<R, Id, V>(
    json: &str,
    registry_name: &str,
    make_id: impl Fn(String) -> Id,
    map_record: impl Fn(R) -> Result<V, Error>,
) -> Result<FxHashMap<Id, V>, Error>
where
    R: for<'de> serde::Deserialize<'de>,
    Id: std::hash::Hash + Eq,
    V: Clone,
{
    let file: RegistryFile<R> = serde_json::from_str(json).map_err(|e| {
        Error::Validation(format!(
            "Failed to parse embedded {registry_name} conventions registry JSON: {e}"
        ))
    })?;
    file.validate_metadata(registry_name)?;

    let mut final_map = FxHashMap::default();
    let mut seen_ids: FxHashSet<String> = FxHashSet::default();
    for entry in file.entries {
        let value = map_record(entry.record)?;
        for id in entry.ids {
            let key = normalize_registry_id(&id);
            if !seen_ids.insert(key.clone()) {
                return Err(Error::Validation(format!(
                    "Duplicate registry id after normalization: '{}' (from '{}')",
                    key, id
                )));
            }
            final_map.insert(make_id(key), value.clone());
        }
    }

    Ok(final_map)
}
