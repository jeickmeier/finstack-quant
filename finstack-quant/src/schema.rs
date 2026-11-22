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

use finstack_quant_core::schema::diagnostics::Failure;
use finstack_quant_core::schema::{
    SchemaArtifact, ValidatorCache, format_validation_result, reachable_resource_ids,
};
use finstack_quant_core::{Error, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const INSTRUMENT_ENVELOPE_PATH: &str = "schemas/instruments/1/instrument.schema.json";
const VALUATION_RESULT_PATH: &str = "schemas/results/1/valuation_result.schema.json";
const INSTRUMENT_SCHEMA_PREFIX: &str = "schemas/instruments/1/";

/// Every domain registry, labelled with its Python/WASM namespace name.
///
/// The label is what makes a merged index navigable: two crates may both
/// publish a `schema.json` basename, and the domain is how a caller tells them
/// apart without parsing paths.
fn domain_registries() -> [(&'static str, &'static [SchemaArtifact]); 11] {
    [
        ("analytics", finstack_quant_analytics::schema::ARTIFACTS),
        ("core", finstack_quant_core::schema::ARTIFACTS),
        ("attribution", finstack_quant_attribution::schema::ARTIFACTS),
        (
            "calibration",
            finstack_quant_calibration::schema::artifacts(),
        ),
        ("cashflows", finstack_quant_cashflows::schema::ARTIFACTS),
        ("factor_model", finstack_quant_factor::schema::ARTIFACTS),
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

/// Look up one domain's schema artifact slice by its namespace name.
///
/// # Arguments
///
/// * `domain` - Published domain name from [`domains`] (for example `"core"`,
///   `"valuations"`, or `"factor_model"`).
///
/// # Errors
///
/// Returns [`Error::Input`] ([`InputError::NotFound`](finstack_quant_core::InputError::NotFound))
/// when `domain` is not a registered schema domain.
///
/// # Examples
/// ```
/// let core = finstack_quant::schema::domain_registry("core").expect("core");
/// assert!(!core.is_empty());
/// ```
pub fn domain_registry(domain: &str) -> Result<&'static [SchemaArtifact]> {
    domain_registries()
        .into_iter()
        .find_map(|(name, registry)| (name == domain).then_some(registry))
        .ok_or_else(|| {
            Error::Input(finstack_quant_core::InputError::NotFound {
                id: format!("schema domain {domain:?} (call domains() for valid domains)"),
            })
        })
}

/// The domain names that label the rows of [`index`], sorted and unique.
///
/// # Examples
/// ```
/// let domains = finstack_quant::schema::domains();
/// assert!(domains.contains(&"valuations"));
/// assert!(domains.windows(2).all(|pair| pair[0] < pair[1]));
/// ```
#[must_use]
pub fn domains() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = domain_registries()
        .into_iter()
        .map(|(domain, _)| domain)
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Process-wide lazy cache of rendered schemas, compiled `$ref` resources, and
/// trimmed per-`$id` validators across all eleven domains.
fn validator_cache() -> &'static ValidatorCache {
    static CACHE: OnceLock<ValidatorCache> = OnceLock::new();
    CACHE.get_or_init(|| ValidatorCache::new(artifacts()))
}

/// Return the cached rendered schema document for a registered workspace artifact.
fn cached_document(artifact: &'static SchemaArtifact) -> Result<&'static Value> {
    validator_cache()
        .document(artifact.id)?
        .ok_or_else(|| Error::Internal(format!("{} missing from validator cache", artifact.id)))
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
        documents.insert(artifact.id.to_string(), cached_document(artifact)?.clone());
    }
    Ok(documents)
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
/// Returns [`Error::Input`] ([`InputError::NotFound`](finstack_quant_core::InputError::NotFound))
/// when nothing matches `selector`.
///
/// # Examples
/// ```
/// let artifact = finstack_quant::schema::find("bond.schema.json").expect("bond");
/// assert_eq!(artifact.title, "bond");
/// ```
pub fn find(selector: &str) -> Result<&'static SchemaArtifact> {
    finstack_quant_core::schema::find_schema_artifact(artifacts(), selector)
}

/// List every schema the workspace publishes, across all eleven domains.
///
/// One call replaces per-crate `index()` calls, which is what a service
/// wrapping this library needs in order to advertise its contracts without
/// hard-coding the domain list.
///
/// Each row carries `domain`, `path`, `$id`, `title`, `type_name` (the Rust
/// root type), `summary`, `bytes` and `kind` (`input` for documents you
/// author, `output` for documents the library emits, `component` for shared
/// definitions).
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
    static CACHE: OnceLock<Result<Value>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let mut rows = Vec::new();
            for (domain, artifact) in artifacts_with_domain() {
                let rendered = cached_document(artifact)?;
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
        })
        .clone()
}

/// Per-domain cached schema index documents, keyed by domain name.
fn domain_index_cache() -> &'static BTreeMap<&'static str, OnceLock<Result<Value>>> {
    static CACHE: OnceLock<BTreeMap<&'static str, OnceLock<Result<Value>>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        domain_registries()
            .into_iter()
            .map(|(domain, _)| (domain, OnceLock::new()))
            .collect()
    })
}

/// Build one domain's schema index document from the cached rendered corpus.
fn build_domain_index_from_corpus(registry: &'static [SchemaArtifact]) -> Result<Value> {
    let mut rows = BTreeMap::new();
    for artifact in registry {
        let rendered = cached_document(artifact)?;
        let row = finstack_quant_core::schema::schema_index_row(artifact, rendered)?;
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
    Ok(serde_json::json!({
        "artifacts": rows.into_values().collect::<Vec<_>>(),
        "schema_index_version": finstack_quant_core::schema::SCHEMA_INDEX_VERSION,
    }))
}

/// List every schema published by a single domain crate, cached per domain.
///
/// Matches the structure produced by
/// [`build_schema_index`](finstack_quant_core::schema::build_schema_index)
/// (`artifacts` sorted by `path`, plus `schema_index_version`) while reusing
/// the cached rendered corpus so repeated calls do not regenerate schemas.
///
/// # Arguments
///
/// * `domain` - Published domain name from [`domains`] (for example `"core"`,
///   `"valuations"`, or `"factor_model"`).
///
/// # Errors
///
/// Returns [`Error::Input`] ([`InputError::NotFound`](finstack_quant_core::InputError::NotFound))
/// when `domain` is not a registered schema domain, or [`Error::Internal`] if a
/// schema in the domain cannot be rendered or encoded.
///
/// # Examples
/// ```
/// let index = finstack_quant::schema::index_for_domain("core").expect("core index");
/// assert_eq!(index["schema_index_version"], serde_json::json!(1));
/// assert!(!index["artifacts"].as_array().expect("rows").is_empty());
/// ```
pub fn index_for_domain(domain: &str) -> Result<Value> {
    let registry = domain_registry(domain)?;
    let slot = domain_index_cache()
        .get(domain)
        .ok_or_else(|| Error::Internal(format!("schema domain cache missing {domain:?}")))?;
    slot.get_or_init(|| build_domain_index_from_corpus(registry))
        .clone()
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
    let cache = validator_cache();
    let canonical = match cache.document(artifact.id)? {
        Some(doc) => doc.clone(),
        None => artifact.generate()?,
    };
    match profile {
        "canonical" => Ok(canonical),
        "llm" => {
            let _ = reachable_resource_ids(&canonical, |id| cache.document(id))?;
            finstack_quant_core::schema::project_llm(
                &canonical,
                &|id: &str| cache.document(id).ok().flatten().cloned(),
                &finstack_quant_core::schema::LlmProfile::default(),
            )
        }
        other => Err(Error::Validation(format!(
            "unknown schema profile {other:?}; expected canonical or llm"
        ))),
    }
}

/// Validate a payload against one published artifact.
///
/// Validators are compiled lazily on first use per artifact `$id` with only the
/// corpus resources transitively reachable via `$ref`, and cached for the
/// process lifetime. Union failures are drilled into rather than reported at
/// the union: see the module documentation for why that matters.
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
    validator_cache().validate_artifact(artifact, payload)
}

fn valuations_artifact_by_path(relative_path: &str) -> Result<&'static SchemaArtifact> {
    finstack_quant_valuations::schema::artifacts_slice()
        .iter()
        .find(|artifact| artifact.relative_path == relative_path)
        .ok_or_else(|| {
            Error::Internal(format!(
                "schema artifact missing from valuations registry: {relative_path}"
            ))
        })
}

fn instrument_artifacts() -> &'static BTreeMap<&'static str, &'static SchemaArtifact> {
    static MAP: OnceLock<BTreeMap<&'static str, &'static SchemaArtifact>> = OnceLock::new();
    MAP.get_or_init(|| {
        finstack_quant_valuations::schema::artifacts_slice()
            .iter()
            .filter(|artifact| {
                artifact.relative_path.starts_with(INSTRUMENT_SCHEMA_PREFIX)
                    && artifact.relative_path != INSTRUMENT_ENVELOPE_PATH
            })
            .map(|artifact| (artifact.title, artifact))
            .collect()
    })
}

fn instrument_artifact(instrument_type: &str) -> Result<&'static SchemaArtifact> {
    instrument_artifacts()
        .get(instrument_type)
        .copied()
        .ok_or_else(|| {
            finstack_quant_core::InputError::NotFound {
                id: format!(
                    "unknown instrument type '{instrument_type}'; call instrument_types() for the valid set"
                ),
            }
            .into()
        })
}

/// Return every canonical tagged-instrument discriminator, in registry order.
#[must_use]
pub fn instrument_types() -> &'static [&'static str] {
    finstack_quant_valuations::schema::instrument_types()
}

/// Return the cached JSON Schema for the canonical instrument envelope.
///
/// Backed by the workspace-wide [`ValidatorCache`] so callers sharing this
/// process never render or pin a second copy of the valuations schemas.
///
/// # Errors
///
/// Returns [`Error::Internal`] if the envelope schema cannot be rendered.
pub fn instrument_envelope_schema() -> Result<&'static Value> {
    cached_document(valuations_artifact_by_path(INSTRUMENT_ENVELOPE_PATH)?)
}

/// Return the cached JSON Schema for a single registered instrument type.
///
/// Backed by the workspace-wide [`ValidatorCache`].
///
/// # Arguments
///
/// * `instrument_type` - Canonical registered tagged-instrument discriminator
///   from [`instrument_types`] (for example `"bond"` or `"interest_rate_swap"`).
///
/// # Errors
///
/// Returns [`Error::Input`] ([`InputError::NotFound`](finstack_quant_core::InputError::NotFound))
/// when `instrument_type` is not registered, or [`Error::Internal`] if the
/// schema cannot be rendered.
pub fn instrument_schema(instrument_type: &str) -> Result<&'static Value> {
    cached_document(instrument_artifact(instrument_type)?)
}

/// Return the cached JSON Schema for `ValuationResult`.
///
/// Backed by the workspace-wide [`ValidatorCache`].
///
/// # Errors
///
/// Returns [`Error::Internal`] if the valuation-result schema cannot be rendered.
pub fn valuation_result_schema() -> Result<&'static Value> {
    cached_document(valuations_artifact_by_path(VALUATION_RESULT_PATH)?)
}

/// Validate a parsed instrument envelope against the canonical envelope and
/// selected per-type instrument schemas using the workspace-wide validator cache.
///
/// # Arguments
///
/// * `instance` - Parsed JSON instrument envelope (`finstack_quant.instrument/1`)
///   to validate against the envelope schema and its `instrument.type` schema.
///
/// # Errors
///
/// Returns [`Error::Validation`] listing every violation when `instance` fails
/// validation, or [`Error::Internal`] if a schema cannot be rendered or compiled.
pub fn validate_instrument_envelope_json(instance: &Value) -> Result<()> {
    let envelope_artifact = valuations_artifact_by_path(INSTRUMENT_ENVELOPE_PATH)?;
    let cache = validator_cache();
    let envelope_validator = cache.validator(envelope_artifact.id)?.ok_or_else(|| {
        Error::Internal(format!(
            "{} missing from validator cache",
            envelope_artifact.id
        ))
    })?;
    let envelope_valid = envelope_validator.is_valid(instance);

    let instrument_type = instance.pointer("/instrument/type").and_then(Value::as_str);

    if !envelope_valid {
        if let Some(ty) = instrument_type
            && instrument_types().contains(&ty)
        {
            validate_instrument_type_json(ty, instance)?;
        }
        let envelope_schema = cached_document(envelope_artifact)?;
        let failures = cache.collect_failures(envelope_validator, envelope_schema, instance);
        return format_validation_result(&failures, "instrument envelope");
    }

    let instrument_type = instrument_type.ok_or_else(|| {
        Error::Validation(
            "instrument envelope validation passed but instrument.type is missing".to_string(),
        )
    })?;

    validate_instrument_type_json(instrument_type, instance)
}

/// Validate a parsed JSON value against one registered instrument type's schema
/// using the workspace-wide validator cache.
///
/// # Arguments
///
/// * `instrument_type` - Canonical registered tagged-instrument discriminator
///   from [`instrument_types`] (for example `"bond"`).
/// * `instance` - Parsed JSON payload to check against that type's schema.
///
/// # Errors
///
/// Returns [`Error::Input`] ([`InputError::NotFound`](finstack_quant_core::InputError::NotFound))
/// when `instrument_type` is not registered, [`Error::Validation`] listing
/// every violation when `instance` does not conform, or [`Error::Internal`] if
/// the schema cannot be rendered or compiled.
pub fn validate_instrument_type_json(instrument_type: &str, instance: &Value) -> Result<()> {
    let artifact = instrument_artifact(instrument_type)?;
    let failures = validator_cache().validate_artifact(artifact, instance)?;
    format_validation_result(&failures, instrument_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // `validate` / `index` compile the full published corpus. Selector
    // lookups stay on the default path; the rest run via `mise run rust-test --slow`.

    fn bond() -> &'static SchemaArtifact {
        find("bond.schema.json").expect("bond artifact")
    }

    fn bond_example() -> Value {
        bond().generate().expect("schema")["examples"][0].clone()
    }

    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn published_example_validates_clean() {
        let failures = validate(bond(), &bond_example()).expect("validate");
        assert_eq!(failures, Vec::new(), "published example must validate");
    }

    #[test]
    #[ignore = "slow: covered by mise run rust-test --slow"]
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
    #[ignore = "slow: covered by mise run rust-test --slow"]
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
    #[ignore = "slow: covered by mise run rust-test --slow"]
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
    #[ignore = "slow: covered by mise run rust-test --slow"]
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
    #[ignore = "slow: covered by mise run rust-test --slow"]
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
    #[ignore = "slow: covered by mise run rust-test --slow"]
    fn index_covers_all_eleven_domains() {
        let index = index().expect("index");
        let rows = index["artifacts"].as_array().expect("rows");
        let domains: std::collections::BTreeSet<&str> = rows
            .iter()
            .filter_map(|row| row["domain"].as_str())
            .collect();

        assert_eq!(
            domains,
            [
                "analytics",
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
    fn domains_are_the_sorted_registry_labels() {
        assert_eq!(
            domains(),
            [
                "analytics",
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

    #[test]
    fn domain_registry_resolves_known_domains_and_rejects_unknown() {
        assert_eq!(
            domain_registry("core").expect("core").len(),
            finstack_quant_core::schema::ARTIFACTS.len()
        );
        assert_eq!(
            domain_registry("valuations").expect("valuations").len(),
            finstack_quant_valuations::schema::artifacts_slice().len()
        );
        let Err(err) = domain_registry("no_such_domain") else {
            panic!("unknown domain must fail");
        };
        assert_eq!(err.kind(), finstack_quant_core::ErrorKind::NotFound);
    }

    #[test]
    fn cached_validator_is_reused_and_trimmed_to_reachable_refs() {
        let artifact = bond();
        let cache = validator_cache();
        let bond_doc = cache
            .document(artifact.id)
            .expect("bond document")
            .expect("bond in cache");
        let reachable =
            reachable_resource_ids(bond_doc, |id| cache.document(id)).expect("reachable");
        assert!(
            reachable.contains("https://finstack_quant.dev/schemas/common/1/money.schema.json"),
            "bond must reach money schema: {reachable:?}"
        );
        assert!(
            reachable.len() < artifacts().len() / 2,
            "single-instrument validator should trim corpus resources ({} vs {})",
            reachable.len(),
            artifacts().len()
        );

        let first = cache
            .validator(artifact.id)
            .expect("compile")
            .expect("registered");
        let second = cache
            .validator(artifact.id)
            .expect("compile")
            .expect("registered");
        assert!(
            std::ptr::eq(first, second),
            "repeat lookups must reuse the same cached validator"
        );

        let valid = bond_example();
        assert_eq!(validate(artifact, &valid).expect("validate"), Vec::new());
        assert_eq!(validate(artifact, &valid).expect("validate"), Vec::new());

        let mut bad_date = valid;
        bad_date["instrument"]["spec"]["maturity"] = json!("01/15/2034");
        let failures = validate(artifact, &bad_date).expect("validate");
        assert!(
            failures
                .iter()
                .any(|failure| failure.pointer == "/instrument/spec/maturity"),
            "cached validator must assert format: date, got {failures:?}"
        );
    }

    #[test]
    fn valuations_schema_helpers_share_workspace_cache_and_preserve_errors() {
        let bond_artifact = bond();
        let cached_doc = validator_cache()
            .document(bond_artifact.id)
            .expect("bond doc")
            .expect("registered");
        let helper_doc = instrument_schema("bond").expect("instrument_schema");
        assert!(
            std::ptr::eq(cached_doc, helper_doc),
            "instrument_schema must borrow from the workspace ValidatorCache"
        );
        assert_eq!(
            instrument_envelope_schema().expect("envelope")["title"],
            "Finstack Quant Instrument"
        );
        assert_eq!(
            valuation_result_schema().expect("result")["title"],
            "Valuation Result"
        );

        let valid = bond_example();
        validate_instrument_envelope_json(&valid).expect("valid envelope");
        validate_instrument_type_json("bond", &valid).expect("valid typed");

        let mut bad_date = valid;
        bad_date["instrument"]["spec"]["maturity"] = json!("01/15/2034");
        let envelope_err = validate_instrument_envelope_json(&bad_date)
            .expect_err("malformed date must fail envelope validation")
            .to_string();
        assert!(
            envelope_err.contains("bond validation failed")
                && envelope_err.contains("/instrument/spec/maturity"),
            "unexpected envelope error: {envelope_err}"
        );
        let typed_err = validate_instrument_type_json("bond", &bad_date)
            .expect_err("malformed date must fail typed validation")
            .to_string();
        assert_eq!(envelope_err, typed_err);

        let unknown_schema = instrument_schema("not_a_type").expect_err("unknown type");
        assert_eq!(
            unknown_schema.kind(),
            finstack_quant_core::ErrorKind::NotFound
        );
        assert!(unknown_schema.to_string().contains("instrument_types()"));

        let unknown_validate =
            validate_instrument_type_json("not_a_type", &json!({})).expect_err("unknown type");
        assert_eq!(
            unknown_validate.kind(),
            finstack_quant_core::ErrorKind::NotFound
        );
    }

    #[test]
    fn index_for_domain_matches_build_schema_index_and_caches() {
        let cached = index_for_domain("core").expect("core index");
        let expected =
            finstack_quant_core::schema::build_schema_index(finstack_quant_core::schema::ARTIFACTS)
                .expect("build core index");
        assert_eq!(cached, expected);
        assert_eq!(index_for_domain("core").expect("core index repeat"), cached);
        let err = index_for_domain("no_such_domain").expect_err("unknown domain");
        assert_eq!(err.kind(), finstack_quant_core::ErrorKind::NotFound);
    }
}
