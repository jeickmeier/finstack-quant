//! The workspace's complete JSON Schema registry.
//!
//! Each domain crate owns its own `schema::ARTIFACTS`. Only the umbrella sees
//! all of them at once, which is what a cross-document reference resolver, a
//! whole-corpus index, and a validator that can follow `$ref` need.
//!
//! # Validation errors are written for a caller who got it wrong
//!
//! A raw JSON Schema `oneOf` failure names the union, not the field: every
//! tagged payload in this workspace is a `oneOf`, so a single mistyped enum
//! value reports "is not valid under any of the schemas listed in the 'oneOf'
//! keyword" against the whole subtree. [`validate`] instead picks the branch
//! the payload most nearly matches and reports *that* branch's failures, at the
//! pointer of the offending field. Unit-enum unions collapse further, into one
//! message enumerating the accepted spellings.
//!
//! # Examples
//! ```
//! use serde_json::json;
//!
//! let artifact = finstack_quant::schema::find("bond.schema.json").expect("bond");
//! let failures = finstack_quant::schema::validate(artifact, &json!({})).expect("validate");
//! assert!(!failures.is_empty());
//! ```

use finstack_quant_core::schema::SchemaArtifact;
use finstack_quant_core::{Error, Result};
use jsonschema::error::ValidationErrorKind;
use jsonschema::paths::{Location, LocationSegment};
use serde_json::Value;
use std::collections::BTreeMap;

/// How many nested union failures to drill through before reporting as-is.
///
/// Instrument payloads nest a tagged envelope inside a tagged spec inside a
/// tagged cashflow schedule, so three levels is the observed working depth; the
/// fourth is headroom. Beyond it the reported message is the raw union failure,
/// which is correct but coarse.
const MAX_UNION_EXPANSION_DEPTH: usize = 4;

/// Every domain registry, labelled with its Python/WASM namespace name.
///
/// The label is what makes a merged index navigable: two crates may both
/// publish a `schema.json` basename, and the domain is how a caller tells them
/// apart without parsing paths.
fn domain_registries() -> Vec<(&'static str, &'static [SchemaArtifact])> {
    vec![
        ("core", finstack_quant_core::schema::ARTIFACTS),
        ("attribution", finstack_quant_attribution::schema::ARTIFACTS),
        (
            "calibration",
            finstack_quant_calibration::json_schema::artifacts(),
        ),
        ("cashflows", finstack_quant_cashflows::schema::ARTIFACTS),
        (
            "factor_model",
            finstack_quant_models::factor::schema::ARTIFACTS,
        ),
        ("margin", finstack_quant_margin::schema::ARTIFACTS),
        ("portfolio", finstack_quant_portfolio::schema::ARTIFACTS),
        ("scenarios", finstack_quant_scenarios::schema::ARTIFACTS),
        ("statements", finstack_quant_statements::schema::ARTIFACTS),
        (
            "valuations",
            finstack_quant_valuations::schema::artifacts_slice(),
        ),
    ]
}

/// Every published artifact across every domain crate, in registry order.
///
/// # Examples
/// ```
/// let artifacts = finstack_quant::schema::artifacts();
/// assert!(artifacts.iter().any(|a| a.relative_path.ends_with("bond.schema.json")));
/// ```
#[must_use]
pub fn artifacts() -> Vec<&'static SchemaArtifact> {
    artifacts_with_domain()
        .into_iter()
        .map(|(_, artifact)| artifact)
        .collect()
}

/// Every published artifact paired with the domain that owns it.
///
/// # Examples
/// ```
/// let owned = finstack_quant::schema::artifacts_with_domain();
/// let bond = owned
///     .iter()
///     .find(|(_, a)| a.relative_path.ends_with("bond.schema.json"))
///     .expect("bond");
/// assert_eq!(bond.0, "valuations");
/// ```
#[must_use]
pub fn artifacts_with_domain() -> Vec<(&'static str, &'static SchemaArtifact)> {
    domain_registries()
        .into_iter()
        .flat_map(|(domain, registry)| registry.iter().map(move |artifact| (domain, artifact)))
        .collect()
}

/// Render every artifact once, keyed by `$id`.
///
/// Published schemas reference each other by absolute `$id` on a host that
/// does not resolve; this is the map that makes those references usable
/// in process, for both validation and
/// [`project_llm`](finstack_quant_core::schema::project_llm).
///
/// # Errors
///
/// Returns [`Error::Internal`] if an artifact cannot be rendered.
pub fn documents_by_id() -> Result<BTreeMap<String, Value>> {
    let mut documents = BTreeMap::new();
    for artifact in artifacts() {
        documents.insert(artifact.id.to_string(), artifact.generate()?);
    }
    Ok(documents)
}

/// The whole corpus, rendered once and cached for the process lifetime.
///
/// # Errors
///
/// Returns [`Error::Internal`] if any single artifact fails to render; a
/// partial corpus would silently degrade every cross-document `$ref`
/// resolution and union-failure expansion that depends on it.
fn corpus() -> Result<&'static BTreeMap<String, Value>> {
    static CACHE: std::sync::OnceLock<Result<BTreeMap<String, Value>>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(documents_by_id)
        .as_ref()
        .map_err(Clone::clone)
}

/// Every corpus document as a compiled, in-process resolvable resource.
///
/// # Errors
///
/// Returns [`Error::Internal`] if [`corpus`] fails to render, or if a
/// rendered document is not a compilable JSON Schema resource.
fn resources() -> Result<&'static [(String, jsonschema::Resource)]> {
    static CACHE: std::sync::OnceLock<Result<Vec<(String, jsonschema::Resource)>>> =
        std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            corpus()?
                .iter()
                .map(|(id, document)| {
                    jsonschema::Resource::from_contents(document.clone())
                        .map(|resource| (id.clone(), resource))
                        .map_err(|error| Error::Internal(format!("compile resource {id}: {error}")))
                })
                .collect()
        })
        .as_deref()
        .map_err(Clone::clone)
}

/// Build a validator for one document, with the whole corpus resolvable.
///
/// Format assertion is switched on. JSON Schema leaves `format` as an
/// annotation by default, which would let `"01/15/2034"` pass a `format: date`
/// field and reach the typed loader as a parse error instead of a validation
/// error — a false pass for anyone validating against the published contract
/// rather than calling the Rust loader.
///
/// # Errors
///
/// Returns [`Error::Internal`] if the document is not a compilable schema.
fn build_validator(schema: &Value) -> Result<jsonschema::Validator> {
    jsonschema::options()
        .should_validate_formats(true)
        .with_resources(
            resources()?
                .iter()
                .map(|(id, resource)| (id.as_str(), resource.clone())),
        )
        .build(schema)
        .map_err(|error| Error::Internal(format!("compile schema: {error}")))
}

/// Look up one artifact by `$id`, full path, or trailing filename.
///
/// # Arguments
///
/// * `selector` - An `$id` or `path` from [`index`], or just the trailing
///   filename such as `"bond.schema.json"`. Filename matches are anchored to a
///   path separator, so `"bond.schema.json"` never matches
///   `convertible_bond.schema.json`.
///
/// # Errors
///
/// Returns [`Error::Internal`] naming the domains searched when nothing
/// matches.
///
/// # Examples
/// ```
/// let artifact = finstack_quant::schema::find("bond.schema.json").expect("bond");
/// assert_eq!(artifact.title, "bond");
/// ```
pub fn find(selector: &str) -> Result<&'static SchemaArtifact> {
    finstack_quant_core::schema::find_schema_artifact(artifacts(), selector)
}

/// List every schema the workspace publishes, across all ten domains.
///
/// One call replaces nine per-crate `index()` calls, which is what a service
/// wrapping this library needs in order to advertise its contracts without
/// hard-coding the domain list.
///
/// Each row carries `domain`, `path`, `$id`, `title`, `summary`, `bytes` and
/// `kind` (`input` for documents you author, `output` for documents the library
/// emits, `component` for shared definitions).
///
/// # Errors
///
/// Returns [`Error::Internal`] if an artifact cannot be rendered.
///
/// # Examples
/// ```
/// let index = finstack_quant::schema::index().expect("index");
/// assert_eq!(index["schema_index_version"], serde_json::json!(1));
/// assert!(index["artifacts"].as_array().expect("rows").len() > 100);
/// ```
pub fn index() -> Result<Value> {
    let corpus = corpus()?;
    let mut rows = Vec::new();
    for (domain, artifact) in artifacts_with_domain() {
        let rendered = corpus.get(artifact.id).ok_or_else(|| {
            Error::Internal(format!("{} missing from rendered corpus", artifact.id))
        })?;
        let mut row = finstack_quant_core::schema::schema_index_row(artifact, rendered)?;
        row.insert("domain".to_string(), Value::String(domain.to_string()));
        rows.push(Value::Object(row));
    }
    rows.sort_by(|left, right| {
        (left["domain"].as_str(), left["path"].as_str())
            .cmp(&(right["domain"].as_str(), right["path"].as_str()))
    });
    Ok(serde_json::json!({
        "artifacts": rows,
        "schema_index_version": finstack_quant_core::schema::SCHEMA_INDEX_VERSION,
    }))
}

/// Render a published schema in canonical or LLM projection form.
///
/// # Arguments
///
/// * `artifact` - Published schema selected from a domain or workspace registry.
/// * `profile` - `"canonical"` for the validation contract or `"llm"` for a
///   self-contained projection intended for structured-output generation.
///
/// # Errors
///
/// Returns an error for unknown profiles, failed schema generation, or failed
/// corpus rendering or projection. A partial corpus is never substituted.
pub fn render_profile(artifact: &SchemaArtifact, profile: &str) -> Result<Value> {
    match profile {
        "canonical" => artifact.generate(),
        "llm" => {
            let corpus = corpus()?;
            let canonical = artifact.generate()?;
            finstack_quant_core::schema::project_llm(
                &canonical,
                &|id: &str| corpus.get(id).cloned(),
                &finstack_quant_core::schema::LlmProfile::default(),
            )
        }
        other => Err(Error::Validation(format!(
            "unknown schema profile {other:?}; expected canonical or llm"
        ))),
    }
}

/// One validation failure, located by JSON Pointer into the payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Failure {
    /// JSON Pointer into the payload, naming the value that failed.
    pub pointer: String,
    /// What was wrong, phrased so the caller can act on it.
    pub message: String,
}

impl Failure {
    /// Render as the `{pointer, message}` object the bindings return.
    fn to_value(&self) -> Value {
        serde_json::json!({"pointer": self.pointer, "message": self.message})
    }
}

/// Render a slice of failures as a JSON array.
///
/// # Arguments
///
/// * `failures` - Validation failures to render; each becomes a
///   `{pointer, message}` object in the returned array, preserving order.
///
/// # Examples
/// ```
/// use serde_json::json;
///
/// let artifact = finstack_quant::schema::find("bond.schema.json").expect("bond");
/// let failures = finstack_quant::schema::validate(artifact, &json!({})).expect("validate");
/// let report = finstack_quant::schema::failures_to_value(&failures);
/// assert!(report.as_array().expect("array").len() == failures.len());
/// ```
#[must_use]
pub fn failures_to_value(failures: &[Failure]) -> Value {
    Value::Array(failures.iter().map(Failure::to_value).collect())
}

/// Validate a payload against one published artifact.
///
/// Union failures are drilled into rather than reported at the union: see the
/// module documentation for why that matters.
///
/// # Arguments
///
/// * `artifact` - The contract to check against, from [`find`] or [`artifacts`].
/// * `payload` - The parsed JSON value to check.
///
/// # Errors
///
/// Returns [`Error::Internal`] if the artifact cannot be rendered or compiled.
///
/// # Examples
/// ```
/// use serde_json::json;
///
/// let artifact = finstack_quant::schema::find("bond.schema.json").expect("bond");
/// let mut payload = artifact.generate().expect("schema")["examples"][0].clone();
/// assert!(finstack_quant::schema::validate(artifact, &payload).expect("ok").is_empty());
///
/// payload["instrument"]["spec"]["cashflow_spec"]["fixed"]["day_count"] = json!("ACT/365");
/// let failures = finstack_quant::schema::validate(artifact, &payload).expect("ok");
/// assert_eq!(failures[0].pointer, "/instrument/spec/cashflow_spec/fixed/day_count");
/// ```
pub fn validate(artifact: &SchemaArtifact, payload: &Value) -> Result<Vec<Failure>> {
    let schema = artifact.generate()?;
    validate_document(&schema, payload)
}

/// Validate a payload against an already-rendered schema document.
///
/// # Arguments
///
/// * `schema` - A rendered schema document, as written to disk.
/// * `payload` - The parsed JSON value to check.
///
/// # Errors
///
/// Returns [`Error::Internal`] if the document is not a compilable schema.
pub fn validate_document(schema: &Value, payload: &Value) -> Result<Vec<Failure>> {
    let validator = build_validator(schema)?;
    let mut failures = Vec::new();
    for error in validator.iter_errors(payload) {
        failures.extend(expand_failure(&error, schema, MAX_UNION_EXPANSION_DEPTH));
    }
    Ok(failures)
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
fn follow_reference(target: &str, base: &Value) -> Option<(Value, Value)> {
    let (document_id, fragment) = target.split_once('#').unwrap_or((target, ""));
    let document = if document_id.is_empty() {
        base.clone()
    } else {
        corpus().ok()?.get(document_id)?.clone()
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
fn schema_node_at(root: &Value, location: &Location) -> Option<(Value, Value)> {
    let mut base = root.clone();
    let mut current = root.clone();
    for segment in location {
        match segment {
            LocationSegment::Property("$ref") => {
                let target = current.get("$ref")?.as_str()?;
                let (next_base, node) = follow_reference(target, &base)?;
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
) -> Vec<Failure> {
    let here = Failure {
        pointer: error.instance_path.to_string(),
        message: error.to_string(),
    };
    if depth == 0 || !is_union_failure(&error.kind) {
        return vec![here];
    }
    let Some((base, node)) = schema_node_at(root, &error.schema_path) else {
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

    let Some(best) = best_branch_failures(branches, &base, &error.instance, depth) else {
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
) -> Option<Vec<Failure>> {
    let mut best: Option<Vec<Failure>> = None;
    for branch in branches {
        let Some(schema) = branch_schema(branch, base) else {
            continue;
        };
        let Ok(validator) = build_validator(&schema) else {
            continue;
        };
        let mut failures = Vec::new();
        for error in validator.iter_errors(instance) {
            failures.extend(expand_failure(&error, &schema, depth - 1));
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

    // `validate` / `index` compile the full published corpus. Selector
    // lookups stay on the default path; the rest run via rust-test-slow.

    fn bond() -> &'static SchemaArtifact {
        find("bond.schema.json").expect("bond artifact")
    }

    fn bond_example() -> Value {
        bond().generate().expect("schema")["examples"][0].clone()
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn published_example_validates_clean() {
        let failures = validate(bond(), &bond_example()).expect("validate");
        assert_eq!(failures, Vec::new(), "published example must validate");
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn wrong_enum_spelling_names_the_field_and_lists_accepted_values() {
        let mut payload = bond_example();
        payload["instrument"]["spec"]["cashflow_spec"]["fixed"]["day_count"] = json!("ACT/365");

        let failures = validate(bond(), &payload).expect("validate");

        assert_eq!(failures.len(), 1, "one mistake reports once: {failures:?}");
        assert_eq!(
            failures[0].pointer,
            "/instrument/spec/cashflow_spec/fixed/day_count"
        );
        assert!(
            failures[0].message.contains("act_365f"),
            "message must enumerate accepted spellings: {}",
            failures[0].message
        );
        assert!(
            !failures[0].message.contains("oneOf"),
            "must not surface the raw union failure: {}",
            failures[0].message
        );
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn decimal_as_json_number_points_at_the_decimal_field() {
        let mut payload = bond_example();
        payload["instrument"]["spec"]["cashflow_spec"]["fixed"]["rate"] = json!(0.0425);

        let failures = validate(bond(), &payload).expect("validate");

        assert!(
            failures
                .iter()
                .any(|failure| failure.pointer == "/instrument/spec/cashflow_spec/fixed/rate"),
            "must point at the offending decimal, got {failures:?}"
        );
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn malformed_date_fails_validation_not_only_the_typed_loader() {
        let mut payload = bond_example();
        payload["instrument"]["spec"]["maturity"] = json!("01/15/2034");

        let failures = validate(bond(), &payload).expect("validate");

        assert!(
            failures
                .iter()
                .any(|failure| failure.pointer == "/instrument/spec/maturity"),
            "format: date must be asserted, got {failures:?}"
        );
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn unknown_property_still_names_the_property() {
        let mut payload = bond_example();
        payload["instrument"]["spec"]["notionall"] = json!(1);

        let failures = validate(bond(), &payload).expect("validate");

        assert!(
            failures
                .iter()
                .any(|failure| failure.message.contains("notionall")),
            "typo must be named, got {failures:?}"
        );
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn index_labels_every_artifact_with_its_domain() {
        let index = index().expect("index");
        let rows = index["artifacts"].as_array().expect("rows");

        assert_eq!(rows.len(), artifacts().len());
        assert!(rows.iter().all(|row| row["domain"].is_string()));

        let bond_row = rows
            .iter()
            .find(|row| {
                row["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/bond.schema.json"))
            })
            .expect("bond row");
        assert_eq!(bond_row["domain"], json!("valuations"));
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn index_covers_all_ten_domains() {
        let index = index().expect("index");
        let rows = index["artifacts"].as_array().expect("rows");
        let domains: std::collections::BTreeSet<&str> = rows
            .iter()
            .filter_map(|row| row["domain"].as_str())
            .collect();

        assert_eq!(
            domains,
            [
                "attribution",
                "calibration",
                "cashflows",
                "core",
                "factor_model",
                "margin",
                "portfolio",
                "scenarios",
                "statements",
                "valuations",
            ]
            .into_iter()
            .collect::<std::collections::BTreeSet<&str>>()
        );
    }

    #[test]
    fn filename_selector_is_anchored_to_a_path_separator() {
        assert_eq!(find("bond.schema.json").expect("bond").title, "bond");
        assert_eq!(
            find("convertible_bond.schema.json")
                .expect("convertible bond")
                .title,
            "convertible_bond"
        );
    }

    #[test]
    fn unknown_selector_reports_how_to_discover_valid_ones() {
        let Err(error) = find("no_such_schema.json") else {
            panic!("unknown selector must fail");
        };
        assert!(error.to_string().contains("index()"), "{error}");
    }
}
