//! Typed externalization and packaging of shared `$defs`
//! ([`ExternalSchemaDefinition`], [`externalize_schema_definitions`]).
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::registry::*;
use crate::{Error, Result};

/// Typed external definition that a packaging pass may reference.
///
/// The generated schema for `T` is compared with the local `$defs` subgraph
/// before any reference is rewritten, so a reused definition name cannot
/// silently substitute a different validation contract.
#[derive(Clone, Copy)]
pub struct ExternalSchemaDefinition {
    name: &'static str,
    uri: &'static str,
    generator: fn() -> Result<Value>,
}

impl ExternalSchemaDefinition {
    /// Declare one typed external schema definition.
    ///
    /// # Arguments
    ///
    /// * `name` - Exact local Schemars `$defs` name to externalize.
    /// * `uri` - Canonical absolute `$id` of the equivalent artifact.
    #[must_use]
    pub const fn new<T: SerdeSchema>(name: &'static str, uri: &'static str) -> Self {
        Self {
            name,
            uri,
            generator: raw_schema::<T>,
        }
    }
}

pub(super) fn raw_schema<T: SerdeSchema>() -> Result<Value> {
    serde_json::to_value(schemars::schema_for!(T))
        .map_err(|error| Error::Internal(format!("serialize derived schema: {error}")))
}

/// Externalize only definitions proven equivalent to typed serde contracts.
///
/// The comparison recursively resolves local `$defs` references and removes
/// annotation-only keywords before comparing validation assertions. A name
/// collision or shape mismatch fails generation before the schema is changed.
/// Definitions that are not present in this particular derived document are
/// ignored, allowing one canonical definition list to serve a whole registry.
///
/// # Arguments
///
/// * `schema` - Mutable schema derived from a registered serde root.
/// * `definitions` - Typed external definitions eligible for replacement.
///
/// # Errors
///
/// Returns [`Error::Validation`] for duplicate names, unresolved references,
/// or assertion mismatches. Recursive definitions are compared as typed schema
/// graphs, including their cycle edges. Returns [`Error::Internal`] if a typed
/// comparison schema cannot be serialized.
pub fn externalize_schema_definitions(
    schema: &mut Value,
    definitions: &[ExternalSchemaDefinition],
) -> Result<()> {
    let local_defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut external_refs = BTreeMap::new();
    for definition in definitions {
        if external_refs.contains_key(definition.name) {
            return Err(Error::Validation(format!(
                "duplicate external schema definition: {}",
                definition.name
            )));
        }
        let Some(local) = local_defs.get(definition.name) else {
            continue;
        };
        let mut local_stack = vec![definition.name.to_string()];
        let local_validation = validation_view(local, &local_defs, &mut local_stack)?;

        // A recursive root refers to itself as `#`; file it under its local
        // name so both sides compare the same cycle edges.
        let mut external = (definition.generator)()?;
        let root_ref = format!("#/$defs/{}", definition.name);
        rename_root_refs(&mut external, &root_ref);
        let mut external_defs = external
            .get("$defs")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        external_defs.insert(definition.name.to_string(), external.clone());
        let mut external_stack = vec![definition.name.to_string()];
        let external_validation = validation_view(&external, &external_defs, &mut external_stack)?;
        if local_validation != external_validation {
            return Err(Error::Validation(format!(
                "local $defs/{} is not assertion-equivalent to {}",
                definition.name, definition.uri
            )));
        }
        external_refs.insert(definition.name.to_string(), definition.uri.to_string());
    }

    externalize_refs(schema, &external_refs);
    prune_unreachable_defs(schema);
    Ok(())
}

/// Rewrite every whole-document `$ref: "#"` to `root_ref`.
fn rename_root_refs(value: &mut Value, root_ref: &str) {
    match value {
        Value::Object(map) => {
            if map.get("$ref").and_then(Value::as_str) == Some("#") {
                map.insert("$ref".to_string(), Value::String(root_ref.to_string()));
            }
            for child in map.values_mut() {
                rename_root_refs(child, root_ref);
            }
        }
        Value::Array(items) => {
            for item in items {
                rename_root_refs(item, root_ref);
            }
        }
        _ => {}
    }
}

pub(super) fn validation_view(
    value: &Value,
    definitions: &Map<String, Value>,
    stack: &mut Vec<String>,
) -> Result<Value> {
    match value {
        Value::Object(map) => {
            let mut assertions = Map::new();
            for (key, child) in map {
                if matches!(
                    key.as_str(),
                    "$schema"
                        | "$id"
                        | "$defs"
                        | "$comment"
                        | "title"
                        | "description"
                        | "default"
                        | "deprecated"
                        | "examples"
                        | "readOnly"
                        | "writeOnly"
                ) {
                    continue;
                }
                assertions.insert(key.clone(), child.clone());
            }

            if let Some(reference) = assertions
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(local_definition_ref)
                .map(|(name, suffix)| (name, suffix.map(str::to_string)))
            {
                let (name, suffix) = reference;
                if stack.iter().any(|entry| entry == &name) {
                    assertions.remove("$ref");
                    let recursive_reference = match suffix.as_deref() {
                        Some(suffix) => format!("#/$defs/{name}/{suffix}"),
                        None => format!("#/$defs/{name}"),
                    };
                    let expanded = serde_json::json!({
                        "$recursiveRef": recursive_reference
                    });
                    if assertions.is_empty() {
                        return Ok(expanded);
                    }
                    let siblings = validation_view(&Value::Object(assertions), definitions, stack)?;
                    return Ok(serde_json::json!({ "allOf": [expanded, siblings] }));
                }
                let target = definitions.get(&name).ok_or_else(|| {
                    Error::Validation(format!("unresolved local schema definition {name}"))
                })?;
                let target = match suffix.as_deref() {
                    Some(suffix) => target.pointer(&format!("/{suffix}")).ok_or_else(|| {
                        Error::Validation(format!(
                            "unresolved schema pointer #/$defs/{name}/{suffix}"
                        ))
                    })?,
                    None => target,
                };
                stack.push(name);
                let expanded = validation_view(target, definitions, stack)?;
                stack.pop();
                assertions.remove("$ref");
                if assertions.is_empty() {
                    return Ok(expanded);
                }
                let siblings = validation_view(&Value::Object(assertions), definitions, stack)?;
                return Ok(serde_json::json!({ "allOf": [expanded, siblings] }));
            }

            assertions
                .into_iter()
                .map(|(key, child)| {
                    validation_view(&child, definitions, stack).map(|child| (key, child))
                })
                .collect::<Result<Map<_, _>>>()
                .map(Value::Object)
        }
        Value::Array(items) => items
            .iter()
            .map(|item| validation_view(item, definitions, stack))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        _ => Ok(value.clone()),
    }
}

pub(super) fn externalize_refs(value: &mut Value, external_refs: &BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            let replacement = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(local_definition_ref)
                .and_then(|(name, suffix)| {
                    external_refs.get(&name).map(|reference| match suffix {
                        Some(suffix) => format!("{reference}#/{suffix}"),
                        None => reference.clone(),
                    })
                });
            if let Some(reference) = replacement {
                map.insert("$ref".to_string(), Value::String(reference));
            }
            for child in map.values_mut() {
                externalize_refs(child, external_refs);
            }
        }
        Value::Array(items) => {
            for item in items {
                externalize_refs(item, external_refs);
            }
        }
        _ => {}
    }
}

pub(super) fn local_definition_ref(reference: &str) -> Option<(String, Option<&str>)> {
    let rest = reference.strip_prefix("#/$defs/")?;
    let (name, suffix) = match rest.split_once('/') {
        Some((name, suffix)) => (name, Some(suffix)),
        None => (rest, None),
    };
    if name.is_empty() {
        return None;
    }
    Some((name.replace("~1", "/").replace("~0", "~"), suffix))
}

pub(super) fn collect_local_def_refs(value: &Value, refs: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(name) = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(local_definition_ref)
                .map(|(name, _)| name)
            {
                refs.insert(name);
            }
            for child in map.values() {
                collect_local_def_refs(child, refs);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_local_def_refs(item, refs);
            }
        }
        _ => {}
    }
}

pub(super) fn prune_unreachable_defs(value: &mut Value) {
    let Some(defs) = value.get("$defs").and_then(Value::as_object).cloned() else {
        return;
    };
    let mut root = value.clone();
    if let Some(root) = root.as_object_mut() {
        root.remove("$defs");
    }
    let mut pending = BTreeSet::new();
    collect_local_def_refs(&root, &mut pending);
    let mut reachable = BTreeSet::new();
    while let Some(name) = pending.pop_first() {
        if reachable.insert(name.clone()) {
            if let Some(definition) = defs.get(&name) {
                collect_local_def_refs(definition, &mut pending);
            }
        }
    }
    if let Some(defs) = value.get_mut("$defs").and_then(Value::as_object_mut) {
        defs.retain(|name, _| reachable.contains(name));
        if defs.is_empty() {
            if let Some(value) = value.as_object_mut() {
                value.remove("$defs");
            }
        }
    }
}
