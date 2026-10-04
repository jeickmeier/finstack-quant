//! Specific JSON Schema union diagnostics shared by domain and workspace validators.
use crate::Result;
use jsonschema::error::ValidationErrorKind;
use jsonschema::paths::{Location, LocationSegment};
use serde_json::Value;
/// One validation failure, located by JSON Pointer into the payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Failure {
    /// JSON Pointer into the payload, naming the value that failed.
    pub pointer: String,
    /// What was wrong, phrased so the caller can act on it.
    pub message: String,
}

/// Collect violations from an already-compiled validator, expanding up to four nested union failures.
///
/// # Arguments
///
/// * `validator` - Cached validator for the root schema; successful inputs require no branch compilation.
/// * `schema` - Root document used to resolve error keyword locations and local references.
/// * `payload` - JSON value being checked; returned pointers identify its invalid fields.
/// * `resolve` - Lookup of external schema documents by exact URI; unresolved references retain the original diagnostic.
/// * `build` - Domain compiler for a selected union branch, with the same registered resources as the root validator; failed compilation retains the original diagnostic.
pub fn collect_failures(
    validator: &jsonschema::Validator,
    schema: &Value,
    payload: &Value,
    resolve: &dyn Fn(&str) -> Option<Value>,
    build: &dyn Fn(&Value) -> Result<jsonschema::Validator>,
) -> Vec<Failure> {
    validator
        .iter_errors(payload)
        .flat_map(|error| expand_failure(&error, schema, 4, resolve, build))
        .collect()
}

/// Whether an error kind is a union failure worth drilling into.
fn is_union_failure(kind: &ValidationErrorKind) -> bool {
    matches!(
        kind,
        ValidationErrorKind::AnyOf | ValidationErrorKind::OneOfNotValid
    )
}

/// Follow one `$ref` target, returning the document it lands in and the node.
///
/// The document is returned alongside the node because it becomes the base for
/// any further local `#/$defs/...` reference inside that node.
fn follow_reference(
    target: &str,
    base: &Value,
    resolve: &dyn Fn(&str) -> Option<Value>,
) -> Option<(Value, Value)> {
    let (document_id, fragment) = target.split_once('#').unwrap_or((target, ""));
    let document = if document_id.is_empty() {
        base.clone()
    } else {
        resolve(document_id)?
    };
    let node = if fragment.is_empty() {
        document.clone()
    } else {
        document.pointer(fragment)?.clone()
    };
    Some((document, node))
}

/// Resolve a keyword location into the schema node it names.
///
/// Keyword locations interleave `$ref` segments with property and index steps,
/// for example `/properties/instrument/$ref/properties/spec/$ref/oneOf`. A
/// `$ref` segment means "follow the reference on the current node".
///
/// Returns the resolved node together with the document it lives in.
fn schema_node_at(
    root: &Value,
    location: &Location,
    resolve: &dyn Fn(&str) -> Option<Value>,
) -> Option<(Value, Value)> {
    let mut base = root.clone();
    let mut current = root.clone();
    for segment in location {
        match segment {
            LocationSegment::Property("$ref") => {
                let target = current.get("$ref")?.as_str()?;
                let (next_base, node) = follow_reference(target, &base, resolve)?;
                base = next_base;
                current = node;
            }
            LocationSegment::Property(name) => current = current.get(name)?.clone(),
            LocationSegment::Index(position) => current = current.get(position)?.clone(),
        }
    }
    Some((base, current))
}

/// Read every branch of a union as a scalar `const`, if they all are.
///
/// A unit enum is published as a union of single-`const` branches, so a wrong
/// spelling would otherwise report once per branch. Recognising the shape lets
/// one message enumerate the accepted values, matching what the typed loader
/// says.
fn const_union_values(branches: &[Value]) -> Option<Vec<String>> {
    branches
        .iter()
        .map(|branch| match branch.get("const") {
            Some(Value::String(value)) => Some(value.clone()),
            Some(other) if !other.is_object() && !other.is_array() => Some(other.to_string()),
            _ => None,
        })
        .collect()
}

/// Join a parent instance pointer with a child's relative pointer.
fn join_pointer(parent: &str, child: &str) -> String {
    format!("{parent}{child}")
}

/// Turn one validation error into the most specific failures available.
fn expand_failure(
    error: &jsonschema::ValidationError<'_>,
    root: &Value,
    depth: usize,
    resolve: &dyn Fn(&str) -> Option<Value>,
    build: &dyn Fn(&Value) -> Result<jsonschema::Validator>,
) -> Vec<Failure> {
    let here = Failure {
        pointer: error.instance_path.to_string(),
        message: error.to_string(),
    };
    if depth == 0 || !is_union_failure(&error.kind) {
        return vec![here];
    }
    let Some((base, node)) = schema_node_at(root, &error.schema_path, resolve) else {
        return vec![here];
    };
    let Some(branches) = node.as_array() else {
        return vec![here];
    };
    if branches.is_empty() {
        return vec![here];
    }

    if let Some(allowed) = const_union_values(branches) {
        return vec![Failure {
            pointer: here.pointer,
            message: format!("{} is not one of [{}]", error.instance, allowed.join(", ")),
        }];
    }

    let Some(best) = best_branch_failures(branches, &base, &error.instance, depth, resolve, build)
    else {
        return vec![here];
    };
    if best.is_empty() {
        return vec![here];
    }
    best.into_iter()
        .map(|failure| Failure {
            pointer: join_pointer(&here.pointer, &failure.pointer),
            message: failure.message,
        })
        .collect()
}

/// Validate the instance against every branch and return the closest match's failures.
///
/// "Closest" is fewest failures. For the tagged unions in this workspace the
/// intended branch is unambiguous — every other branch rejects the
/// discriminator outright and reports far more — so counting is enough without
/// reading discriminators.
fn best_branch_failures(
    branches: &[Value],
    base: &Value,
    instance: &Value,
    depth: usize,
    resolve: &dyn Fn(&str) -> Option<Value>,
    build: &dyn Fn(&Value) -> Result<jsonschema::Validator>,
) -> Option<Vec<Failure>> {
    let mut best: Option<Vec<Failure>> = None;
    for branch in branches {
        let Some(schema) = branch_schema(branch, base) else {
            continue;
        };
        let Ok(validator) = build(&schema) else {
            continue;
        };
        let mut failures = Vec::new();
        for error in validator.iter_errors(instance) {
            failures.extend(expand_failure(&error, &schema, depth - 1, resolve, build));
        }
        if failures.is_empty() {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|current| failures.len() < current.len())
        {
            best = Some(failures);
        }
    }
    best
}

/// Build a standalone, compilable schema for one union branch.
///
/// The branch is lifted out of its document, so any local `#/$defs/...`
/// reference it carries would dangle. Copying the owning document's `$defs`
/// alongside it keeps those references resolvable; absolute references are
/// already served from the corpus.
fn branch_schema(branch: &Value, base: &Value) -> Option<Value> {
    let mut schema = branch.clone();
    let object = schema.as_object_mut()?;
    if !object.contains_key("$defs") {
        if let Some(defs) = base.get("$defs") {
            object.insert("$defs".to_string(), defs.clone());
        }
    }
    Some(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;

    #[test]
    fn shared_diagnostics_preserve_nested_enum_pointer() {
        let schema = json!({"type":"object", "properties":{"leg":{"type":"object", "properties":{"side":{"oneOf":[{"const":"pay"},{"const":"receive"}]}}}}});
        let validator = jsonschema::validator_for(&schema).expect("schema");
        let build = |schema: &Value| {
            jsonschema::validator_for(schema)
                .map_err(|error| crate::Error::Validation(error.to_string()))
        };
        let failures = collect_failures(
            &validator,
            &schema,
            &json!({"leg":{"side":"invalid"}}),
            &|_| None,
            &build,
        );
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].pointer, "/leg/side");
        assert!(failures[0].message.contains("pay"));
        assert!(failures[0].message.contains("receive"));
    }

    #[test]
    fn valid_payload_never_compiles_union_branches() {
        let schema = json!({"oneOf":[{"type":"string"},{"type":"number"}]});
        let validator = jsonschema::validator_for(&schema).expect("schema");
        let calls = Cell::new(0);
        let build = |schema: &Value| {
            calls.set(calls.get() + 1);
            jsonschema::validator_for(schema)
                .map_err(|error| crate::Error::Validation(error.to_string()))
        };
        assert!(collect_failures(&validator, &schema, &json!(2.0), &|_| None, &build).is_empty());
        assert_eq!(calls.get(), 0);
    }
}
