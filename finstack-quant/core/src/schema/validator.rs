//! Lazy trimmed-`$ref` JSON Schema validator builder and process-wide cache.
//!
//! Each registered artifact owns independent `OnceLock` slots for its rendered
//! JSON document, its compiled `jsonschema::Resource`, and its compiled
//! `jsonschema::Validator`. Callers only pay rendering and compilation cost
//! for schemas they query and the external `$ref` targets transitively
//! reachable from them.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde_json::Value;

use super::SchemaArtifact;
use super::diagnostics::{Failure, collect_failures};
use crate::{Error, Result};

/// Collect every external document URI referenced by `$ref` inside `value`.
///
/// Local fragment references (`#/...`) resolve inside the current document and
/// are skipped. `examples` arrays hold instance payloads rather than schema
/// assertions, so they are skipped as well.
///
/// # Arguments
///
/// * `value` - JSON Schema node walked recursively for external `$ref` strings.
/// * `out` - Destination buffer receiving each discovered external document URI
///   with any `#...` JSON Pointer fragment stripped.
pub fn collect_external_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(target) = map.get("$ref").and_then(Value::as_str) {
                let doc_id = target.split('#').next().unwrap_or(target);
                if !doc_id.is_empty() {
                    out.push(doc_id.to_string());
                }
            }
            for (key, nested) in map {
                if key != "examples" {
                    collect_external_refs(nested, out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_external_refs(item, out);
            }
        }
        _ => {}
    }
}

/// Compute the transitive set of external document `$id`s reachable from `schema`.
///
/// Walks external `$ref` targets iteratively, consulting `resolve_document` at
/// most once per unique `$id` so cyclic references terminate cleanly.
///
/// # Arguments
///
/// * `schema` - Root JSON Schema document whose external `$ref` dependencies
///   are collected.
/// * `resolve_document` - Lookup returning the rendered document for a given
///   external `$id`, `Ok(None)` when the `$id` is outside the caller's corpus,
///   or an error if rendering that referenced document fails.
///
/// # Errors
///
/// Propagates any error returned by `resolve_document`.
pub fn reachable_resource_ids<'a, F>(
    schema: &Value,
    mut resolve_document: F,
) -> Result<BTreeSet<String>>
where
    F: FnMut(&str) -> Result<Option<&'a Value>>,
{
    let mut visited = BTreeSet::new();
    let mut queue = Vec::new();
    collect_external_refs(schema, &mut queue);
    while let Some(id) = queue.pop() {
        if visited.insert(id.clone())
            && let Some(doc) = resolve_document(&id)?
        {
            collect_external_refs(doc, &mut queue);
        }
    }
    Ok(visited)
}

/// Compile a JSON Schema validator registering only the external resources
/// transitively reachable via `$ref` from `schema`.
///
/// Format assertion is always switched on (`.should_validate_formats(true)`).
/// JSON Schema leaves `format` as an annotation by default, which would let
/// malformed values such as `"01/15/2034"` pass a `format: "date"` field and
/// reach the typed Rust loader as a parse error instead of a validation error.
///
/// # Arguments
///
/// * `schema` - Root JSON Schema document to compile.
/// * `resolve_document` - Lookup returning the rendered document for an
///   external `$id`, used to walk transitive `$ref` edges.
/// * `resolve_resource` - Lookup returning a compiled `jsonschema::Resource`
///   for each reachable external `$id`.
///
/// # Errors
///
/// Returns [`Error::Internal`] if compiling `schema` fails, or propagates any
/// error returned by `resolve_document` or `resolve_resource`.
pub fn build_trimmed_validator<'a, FDoc, FRes>(
    schema: &Value,
    resolve_document: FDoc,
    mut resolve_resource: FRes,
) -> Result<jsonschema::Validator>
where
    FDoc: FnMut(&str) -> Result<Option<&'a Value>>,
    FRes: FnMut(&str) -> Result<Option<jsonschema::Resource>>,
{
    let reachable = reachable_resource_ids(schema, resolve_document)?;
    let mut resources = Vec::with_capacity(reachable.len());
    for id in &reachable {
        if let Some(resource) = resolve_resource(id)? {
            resources.push((id.as_str(), resource));
        }
    }
    jsonschema::options()
        .should_validate_formats(true)
        .with_resources(resources.into_iter())
        .build(schema)
        .map_err(|error| Error::Internal(format!("compile schema: {error}")))
}

/// Format structured validation failures into the canonical `Error::Validation`
/// message returned by envelope and per-type validators.
///
/// # Arguments
///
/// * `failures` - Structured violations returned by [`collect_failures`] or
///   [`ValidatorCache::validate_artifact`].
/// * `context` - Human-readable schema label prefixed onto the summary line
///   (for example `"instrument envelope"` or `"bond"`).
///
/// # Errors
///
/// Returns [`Error::Validation`] listing each violation (prefixed by its JSON
/// Pointer when non-empty) whenever `failures` is non-empty.
pub fn format_validation_result(failures: &[Failure], context: &str) -> Result<()> {
    if failures.is_empty() {
        return Ok(());
    }
    let errors: Vec<String> = failures
        .iter()
        .map(|failure| {
            if failure.pointer.is_empty() {
                failure.message.clone()
            } else {
                format!("{}: {}", failure.pointer, failure.message)
            }
        })
        .collect();
    Err(Error::Validation(format!(
        "{context} validation failed with {} error(s):\n  {}",
        errors.len(),
        errors.join("\n  ")
    )))
}

struct CachedArtifactEntry {
    artifact: &'static SchemaArtifact,
    document: OnceLock<Result<Value>>,
    resource: OnceLock<Result<jsonschema::Resource>>,
    validator: OnceLock<Result<jsonschema::Validator>>,
}

/// Lazy per-`$id` cache of rendered schemas, compiled `$ref` resources, and
/// trimmed JSON Schema validators.
///
/// Each registered [`SchemaArtifact`] owns independent `OnceLock` slots so
/// callers only render and compile schemas they actually inspect or reach via
/// `$ref`. Both `Value` and `jsonschema::Validator` are `Send + Sync`, so a
/// `&'static ValidatorCache` can be shared safely across threads without locks.
/// Failed renders and compilations are cached as `Err` and replayed verbatim on
/// subsequent calls.
pub struct ValidatorCache {
    entries: BTreeMap<&'static str, CachedArtifactEntry>,
}

impl ValidatorCache {
    /// Construct an empty-slot cache covering every artifact yielded by
    /// `artifacts`, keyed by canonical `$id`.
    ///
    /// Construction performs no schema rendering or validator compilation.
    ///
    /// # Arguments
    ///
    /// * `artifacts` - Registered schema artifacts that may be queried directly
    ///   or resolved as external `$ref` targets.
    #[must_use]
    pub fn new(artifacts: impl IntoIterator<Item = &'static SchemaArtifact>) -> Self {
        let mut entries = BTreeMap::new();
        for artifact in artifacts {
            entries
                .entry(artifact.id)
                .or_insert_with(|| CachedArtifactEntry {
                    artifact,
                    document: OnceLock::new(),
                    resource: OnceLock::new(),
                    validator: OnceLock::new(),
                });
        }
        Self { entries }
    }

    /// Return the cached rendered schema document for `id`, rendering it on
    /// first request.
    ///
    /// Returns `Ok(None)` when `id` is not registered in this cache.
    ///
    /// # Arguments
    ///
    /// * `id` - Canonical `$id` URI of the schema artifact to look up.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Internal`] if rendering the registered artifact fails.
    pub fn document(&self, id: &str) -> Result<Option<&Value>> {
        let Some(entry) = self.entries.get(id) else {
            return Ok(None);
        };
        entry
            .document
            .get_or_init(|| entry.artifact.generate())
            .as_ref()
            .map(Some)
            .map_err(Clone::clone)
    }

    /// Return the cached `jsonschema::Resource` for `id`, compiling it on first
    /// request.
    ///
    /// Returns `Ok(None)` when `id` is not registered in this cache.
    ///
    /// # Arguments
    ///
    /// * `id` - Canonical `$id` URI of the schema resource to look up.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Internal`] if the document cannot be rendered or is not
    /// a valid JSON Schema resource.
    pub fn resource(&self, id: &str) -> Result<Option<jsonschema::Resource>> {
        let Some(entry) = self.entries.get(id) else {
            return Ok(None);
        };
        entry
            .resource
            .get_or_init(|| {
                let doc = self
                    .document(id)?
                    .ok_or_else(|| Error::Internal(format!("{id} missing from validator cache")))?;
                jsonschema::Resource::from_contents(doc.clone())
                    .map_err(|error| Error::Internal(format!("compile resource {id}: {error}")))
            })
            .clone()
            .map(Some)
    }

    /// Compile an arbitrary schema document against only the cached external
    /// resources transitively reachable from its `$ref` targets.
    ///
    /// # Arguments
    ///
    /// * `schema` - JSON Schema document (for example a root schema or an
    ///   extracted union branch) to compile with format validation enabled.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Internal`] if compiling `schema` or any reachable
    /// external resource fails.
    pub fn build_validator(&self, schema: &Value) -> Result<jsonschema::Validator> {
        build_trimmed_validator(schema, |id| self.document(id), |id| self.resource(id))
    }

    /// Return the cached trimmed validator for `id`, compiling it on first
    /// request.
    ///
    /// Returns `Ok(None)` when `id` is not registered in this cache.
    ///
    /// # Arguments
    ///
    /// * `id` - Canonical `$id` URI of the schema artifact to look up.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Internal`] if rendering or compiling the schema or any
    /// transitively reachable `$ref` target fails.
    pub fn validator(&self, id: &str) -> Result<Option<&jsonschema::Validator>> {
        let Some(entry) = self.entries.get(id) else {
            return Ok(None);
        };
        entry
            .validator
            .get_or_init(|| {
                let schema = self
                    .document(id)?
                    .ok_or_else(|| Error::Internal(format!("{id} missing from validator cache")))?;
                self.build_validator(schema)
            })
            .as_ref()
            .map(Some)
            .map_err(Clone::clone)
    }

    /// Expand validation failures from an already-compiled validator using this
    /// cache for external `$ref` lookups and union-branch compilation.
    ///
    /// # Arguments
    ///
    /// * `validator` - Compiled validator for `schema`.
    /// * `schema` - Root JSON Schema document matching `validator`.
    /// * `payload` - JSON value being validated.
    #[must_use]
    pub fn collect_failures(
        &self,
        validator: &jsonschema::Validator,
        schema: &Value,
        payload: &Value,
    ) -> Vec<Failure> {
        if validator.is_valid(payload) {
            return Vec::new();
        }
        collect_failures(
            validator,
            schema,
            payload,
            &|id| self.document(id).ok().flatten().cloned(),
            &|branch| self.build_validator(branch),
        )
    }

    /// Validate `payload` against `artifact`, reusing the cached document and
    /// trimmed validator when `artifact.id` is registered.
    ///
    /// # Arguments
    ///
    /// * `artifact` - Schema artifact describing the contract to enforce.
    /// * `payload` - Parsed JSON value to validate against `artifact`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Internal`] if rendering or compiling `artifact` or any
    /// reachable `$ref` target fails.
    pub fn validate_artifact(
        &self,
        artifact: &SchemaArtifact,
        payload: &Value,
    ) -> Result<Vec<Failure>> {
        if let Some(validator) = self.validator(artifact.id)? {
            if validator.is_valid(payload) {
                return Ok(Vec::new());
            }
            let schema = self.document(artifact.id)?.ok_or_else(|| {
                Error::Internal(format!("{} missing from validator cache", artifact.id))
            })?;
            return Ok(self.collect_failures(validator, schema, payload));
        }

        let schema = artifact.generate()?;
        let validator = self.build_validator(&schema)?;
        if validator.is_valid(payload) {
            return Ok(Vec::new());
        }
        Ok(self.collect_failures(&validator, &schema, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;

    #[test]
    fn collect_external_refs_strips_fragments_and_skips_local_and_examples() {
        let schema = json!({
            "type": "object",
            "properties": {
                "a": { "$ref": "#/$defs/Local" },
                "b": { "$ref": "https://example.test/one.schema.json#/$defs/Part" },
                "c": { "$ref": "https://example.test/two.schema.json" }
            },
            "examples": [
                { "$ref": "https://example.test/ignored_in_example.schema.json" }
            ]
        });
        let mut refs = Vec::new();
        collect_external_refs(&schema, &mut refs);
        assert_eq!(
            refs,
            vec![
                "https://example.test/one.schema.json",
                "https://example.test/two.schema.json",
            ]
        );
    }

    #[test]
    fn reachable_resource_ids_follows_transitive_edges_and_terminates_on_cycles() {
        let docs: BTreeMap<String, Value> = BTreeMap::from([
            (
                "https://example.test/a.schema.json".to_string(),
                json!({
                    "$id": "https://example.test/a.schema.json",
                    "properties": { "b": { "$ref": "https://example.test/b.schema.json" } }
                }),
            ),
            (
                "https://example.test/b.schema.json".to_string(),
                json!({
                    "$id": "https://example.test/b.schema.json",
                    "properties": { "a": { "$ref": "https://example.test/a.schema.json" } }
                }),
            ),
            (
                "https://example.test/unrelated.schema.json".to_string(),
                json!({ "$id": "https://example.test/unrelated.schema.json", "type": "string" }),
            ),
        ]);
        let root = json!({
            "type": "object",
            "properties": { "a": { "$ref": "https://example.test/a.schema.json" } }
        });

        let reachable = reachable_resource_ids(&root, |id| Ok(docs.get(id))).expect("reachable");
        assert_eq!(
            reachable,
            BTreeSet::from([
                "https://example.test/a.schema.json".to_string(),
                "https://example.test/b.schema.json".to_string(),
            ])
        );
    }

    #[test]
    fn build_trimmed_validator_trims_resources_and_enforces_format() {
        let date_doc = json!({
            "$id": "https://example.test/date.schema.json",
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "string",
            "format": "date"
        });
        let unused_doc = json!({
            "$id": "https://example.test/unused.schema.json",
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "string"
        });
        let docs: BTreeMap<String, Value> = BTreeMap::from([
            (
                "https://example.test/date.schema.json".to_string(),
                date_doc,
            ),
            (
                "https://example.test/unused.schema.json".to_string(),
                unused_doc,
            ),
        ]);
        let root = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["as_of"],
            "properties": {
                "as_of": { "$ref": "https://example.test/date.schema.json" }
            }
        });

        let requested = Cell::new(0usize);
        let validator = build_trimmed_validator(
            &root,
            |id| Ok(docs.get(id)),
            |id| {
                assert_ne!(
                    id, "https://example.test/unused.schema.json",
                    "unreachable resource must not be loaded"
                );
                requested.set(requested.get() + 1);
                let Some(doc) = docs.get(id) else {
                    return Ok(None);
                };
                jsonschema::Resource::from_contents(doc.clone())
                    .map(Some)
                    .map_err(|err| Error::Internal(err.to_string()))
            },
        )
        .expect("validator");

        assert_eq!(requested.get(), 1);
        assert!(validator.is_valid(&json!({ "as_of": "2025-01-15" })));
        assert!(
            !validator.is_valid(&json!({ "as_of": "01/15/2025" })),
            "format: date must be asserted"
        );
    }

    #[test]
    fn validator_cache_reuses_validators_across_calls_and_replays_compile_errors() {
        let cache = ValidatorCache::new(crate::schema::ARTIFACTS.iter());
        let artifact = &crate::schema::ARTIFACTS[0];

        let first = cache
            .validator(artifact.id)
            .expect("compile")
            .expect("registered");
        let second = cache
            .validator(artifact.id)
            .expect("compile")
            .expect("registered");
        assert!(std::ptr::eq(first, second));
        assert!(
            cache
                .validator("https://example.test/missing.schema.json")
                .expect("lookup")
                .is_none()
        );

        let broken = json!({ "type": "not_a_json_schema_type" });
        let first_err = cache
            .build_validator(&broken)
            .expect_err("broken schema must fail")
            .to_string();
        let second_err = cache
            .build_validator(&broken)
            .expect_err("broken schema must fail")
            .to_string();
        assert_eq!(first_err, second_err);
    }
}
