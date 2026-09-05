//! JSON-Schema helpers for Finstack Quant types.
//!
//! Schemas are generated from the crate's serde-friendly types and checked in
//! under `schemas/`. These helpers expose them as `serde_json::Value` for use
//! in validation, UI forms, and contract generation.
//!
//! # Error Handling
//!
//! All schema accessors return `Result<&'static Value>` instead of panicking,
//! allowing callers to handle schema loading failures gracefully.

use serde_json::Value;
use std::sync::OnceLock;

#[cfg(test)]
const JSON_SCHEMA_DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
#[cfg(feature = "jsonschema-validate")]
const COMMON_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/common/1/";

#[cfg(feature = "json-schema")]
const PRICING_OVERRIDE_SCHEMA_DEFINITIONS:
    &[finstack_quant_core::schema::ExternalSchemaDefinition] = &[
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<
        crate::instruments::InstrumentPricingOverrides,
    >(
        "InstrumentPricingOverrides",
        "https://finstack_quant.dev/schemas/common/1/instrument_pricing_overrides.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<
        crate::instruments::MetricPricingOverrides,
    >(
        "MetricPricingOverrides",
        "https://finstack_quant.dev/schemas/common/1/metric_pricing_overrides.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<
        crate::instruments::ScenarioPricingOverrides,
    >(
        "ScenarioPricingOverrides",
        "https://finstack_quant.dev/schemas/common/1/scenario_pricing_overrides.schema.json",
    ),
];

/// Package a derived valuation schema using canonical shared definitions.
///
/// This pass changes only reference placement: common and cashflow definitions
/// are replaced by their equivalent published `$id`, then newly unreachable
/// local definitions are removed.
///
/// # Arguments
///
/// * `schema` - Complete schema generated from a valuation serde type.
#[cfg(feature = "json-schema")]
#[doc(hidden)]
pub fn package_valuations_schema(schema: &mut Value) -> finstack_quant_core::Result<()> {
    let mut definitions = finstack_quant_core::schema::COMMON_SCHEMA_DEFINITIONS.to_vec();
    definitions.extend_from_slice(PRICING_OVERRIDE_SCHEMA_DEFINITIONS);
    definitions.extend_from_slice(finstack_quant_cashflows::schema::CASHFLOW_SCHEMA_DEFINITIONS);
    finstack_quant_core::schema::externalize_schema_definitions(schema, &definitions)
}

/// Parse embedded JSON schema at compile time, returning a Result.
/// The JSON is embedded via `include_str!` so the content is always present,
/// but parsing can still fail if the JSON is malformed.
macro_rules! try_include_schema {
    ($path:expr) => {
        serde_json::from_str::<Value>(include_str!($path))
            .map_err(|e| format!("invalid schema JSON at {}: {}", $path, e))
    };
}

/// Get the JSON Schema for the instrument envelope.
///
/// # Errors
///
/// Returns `Error::Validation` if the embedded schema JSON is malformed.
pub fn instrument_envelope_schema() -> finstack_quant_core::Result<&'static Value> {
    static SCHEMA: OnceLock<Result<Value, String>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| try_include_schema!("../schemas/instruments/1/instrument.schema.json"))
        .as_ref()
        .map_err(|e| finstack_quant_core::Error::Validation(e.clone()))
}

fn instrument_schema_cache(
) -> &'static std::collections::BTreeMap<&'static str, Result<Value, String>> {
    static CACHE: OnceLock<std::collections::BTreeMap<&'static str, Result<Value, String>>> =
        OnceLock::new();
    CACHE.get_or_init(|| {
        crate::instruments::json_loader::instrument_registry()
            .into_iter()
            .map(|entry| (entry.tag, entry.load_embedded_schema()))
            .collect()
    })
}

#[cfg(feature = "jsonschema-validate")]
fn common_schema_resource(
    filename: &'static str,
    raw: &'static str,
) -> finstack_quant_core::Result<(String, jsonschema::Resource)> {
    let schema = serde_json::from_str::<Value>(raw).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "invalid common schema JSON at {filename}: {e}"
        ))
    })?;
    let resource = jsonschema::Resource::from_contents(schema).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "invalid common schema resource at {filename}: {e}"
        ))
    })?;
    Ok((format!("{COMMON_SCHEMA_BASE}{filename}"), resource))
}

#[cfg(feature = "jsonschema-validate")]
fn common_schema_resources() -> finstack_quant_core::Result<Vec<(String, jsonschema::Resource)>> {
    [
        (
            "attributes.schema.json",
            include_str!("../schemas/common/1/attributes.schema.json"),
        ),
        (
            "business_day_convention.schema.json",
            include_str!("../schemas/common/1/business_day_convention.schema.json"),
        ),
        (
            "currency.schema.json",
            include_str!("../schemas/common/1/currency.schema.json"),
        ),
        (
            "date.schema.json",
            include_str!("../schemas/common/1/date.schema.json"),
        ),
        (
            "day_count.schema.json",
            include_str!("../schemas/common/1/day_count.schema.json"),
        ),
        (
            "decimal.schema.json",
            include_str!("../schemas/common/1/decimal.schema.json"),
        ),
        (
            "id.schema.json",
            include_str!("../schemas/common/1/id.schema.json"),
        ),
        (
            "money.schema.json",
            include_str!("../schemas/common/1/money.schema.json"),
        ),
        (
            "instrument_pricing_overrides.schema.json",
            include_str!("../schemas/common/1/instrument_pricing_overrides.schema.json"),
        ),
        (
            "metric_pricing_overrides.schema.json",
            include_str!("../schemas/common/1/metric_pricing_overrides.schema.json"),
        ),
        (
            "scenario_pricing_overrides.schema.json",
            include_str!("../schemas/common/1/scenario_pricing_overrides.schema.json"),
        ),
        (
            "tenor.schema.json",
            include_str!("../schemas/common/1/tenor.schema.json"),
        ),
    ]
    .into_iter()
    .map(|(filename, raw)| common_schema_resource(filename, raw))
    .collect()
}

#[cfg(feature = "jsonschema-validate")]
fn embedded_instrument_schema_resources(
) -> finstack_quant_core::Result<Vec<(String, jsonschema::Resource)>> {
    let mut resources = std::collections::BTreeMap::new();
    for (tag, schema_result) in instrument_schema_cache() {
        let schema = schema_result.as_ref().map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "invalid instrument schema JSON for {tag}: {e}"
            ))
        })?;
        let id = schema.get("$id").and_then(Value::as_str).ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "instrument schema for {tag} is missing $id"
            ))
        })?;
        let resource = jsonschema::Resource::from_contents(schema.clone()).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "invalid instrument schema resource for {tag}: {e}"
            ))
        })?;
        resources.insert(id.to_string(), resource);
    }

    Ok(resources.into_iter().collect())
}

#[cfg(feature = "jsonschema-validate")]
fn external_schema_resources() -> finstack_quant_core::Result<Vec<(String, jsonschema::Resource)>> {
    let mut resources = common_schema_resources()?;
    resources.extend(finstack_quant_cashflows::schema::resources()?);
    resources.extend(embedded_instrument_schema_resources()?);
    Ok(resources)
}

/// Return canonical instrument discriminators from the tagged-JSON registry.
///
/// The registry is the source of truth for decoding and schema generation, so
/// this accessor does not infer type names by parsing checked-in `$ref` paths.
pub fn instrument_types() -> finstack_quant_core::Result<Vec<String>> {
    Ok(crate::instruments::json_loader::registry_tags()
        .iter()
        .map(|tag| (*tag).to_string())
        .collect())
}

/// Get the JSON Schema for a single instrument type.
///
/// Returns the dedicated generated schema for a canonical registry tag.
///
/// # Errors
///
/// Returns `Error::Validation` if the embedded schema JSON is malformed or the
/// requested instrument type is not supported.
///
/// # Arguments
///
/// * `instrument_type` - Canonical registered tagged-instrument type string.
pub fn instrument_schema(instrument_type: &str) -> finstack_quant_core::Result<Value> {
    if let Some(schema) = instrument_schema_cache().get(instrument_type) {
        return schema
            .as_ref()
            .cloned()
            .map_err(|e| finstack_quant_core::Error::Validation(e.clone()));
    }

    Err(finstack_quant_core::Error::Validation(format!(
        "unknown instrument type '{instrument_type}'"
    )))
}

/// Get JSON-Schema for ValuationResult.
///
/// Returns schema for valuation result envelope (PV + metrics).
///
/// # Errors
///
/// Returns `Error::Validation` if the embedded schema JSON is malformed.
#[allow(dead_code)] // Public API, used in tests
pub fn valuation_result_schema() -> finstack_quant_core::Result<&'static Value> {
    static SCHEMA: OnceLock<Result<Value, String>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| try_include_schema!("../schemas/results/1/valuation_result.schema.json"))
        .as_ref()
        .map_err(|e| finstack_quant_core::Error::Validation(e.clone()))
}

#[cfg(feature = "jsonschema-validate")]
/// Validate an instrument JSON value against the envelope schema.
///
/// Returns `Ok(())` if the JSON conforms to the instrument envelope schema,
/// or a detailed `Error::Validation` listing all schema violations.
///
/// # Example
///
/// ```
/// use finstack_quant_valuations::schema::validate_instrument_envelope_json;
///
/// let json: serde_json::Value = serde_json::json!({
///     "schema": "finstack_quant.instrument/1",
///     "instrument": {
///         "type": "bond",
///         "spec": {
///             "id": "UST-10Y",
///             "notional": { "amount": "1000000", "currency": "USD" },
///             "issue_date": "2024-01-15",
///             "maturity": "2034-01-15",
///             "cashflow_spec": {
///                 "fixed": {
///                     "coupon_type": "cash",
///                     "frequency": { "count": 6, "unit": "months" },
///                     "day_count": "act_act_isma",
///                     "calendar_id": "sifma",
///                     "rate": "0.0425"
///                 }
///             },
///             "discount_curve_id": "USD-TREASURY",
///             "attributes": {}
///         }
///     }
/// });
/// if let Err(e) = validate_instrument_envelope_json(&json) {
///     eprintln!("Validation errors: {e}");
/// }
/// ```
///
/// # Errors
///
/// Returns `Error::Validation` if the JSON does not conform to the schema.
///
/// # Arguments
///
/// * `instance` - Parsed JSON instrument envelope to validate against the
///   canonical v1 envelope schema and its selected type schema.
pub fn validate_instrument_envelope_json(instance: &Value) -> finstack_quant_core::Result<()> {
    // Consulted before the `instrument.type` lookup below so that a malformed
    // embedded envelope schema is still reported first, as it was before the
    // validator cache was introduced.
    let _envelope_schema = instrument_envelope_schema()?;
    let envelope_result = instrument_envelope_validator()
        .and_then(|validator| validate_with(validator, instance, "instrument envelope"));

    let instrument_type = instance
        .pointer("/instrument/type")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "instrument envelope validation passed but instrument.type is missing".to_string(),
            )
        })?;

    if let Err(envelope_error) = envelope_result {
        if instrument_types()?.iter().any(|ty| ty == instrument_type) {
            validate_instrument_type_json(instrument_type, instance)?;
        }
        return Err(envelope_error);
    }

    validate_instrument_type_json(instrument_type, instance)
}

#[cfg(feature = "jsonschema-validate")]
/// Validate a JSON value against a specific instrument type's schema.
///
/// # Errors
///
/// Returns `Error::Validation` if the JSON does not conform to the schema.
///
/// # Arguments
///
/// * `instrument_type` - Registered tagged-instrument type whose schema is
///   selected for validation.
/// * `instance` - Parsed JSON value to validate against that type schema.
pub fn validate_instrument_type_json(
    instrument_type: &str,
    instance: &Value,
) -> finstack_quant_core::Result<()> {
    let validator = instrument_type_validator(instrument_type)?;
    validate_with(validator, instance, instrument_type)
}

/// Outcome of compiling one schema, as stored in the validator caches.
///
/// `finstack_quant_core::Error` is `Clone`, so a cached failure is replayed
/// verbatim - same variant, same message - on every later call.
#[cfg(feature = "jsonschema-validate")]
type CachedValidator = Result<jsonschema::Validator, finstack_quant_core::Error>;

#[cfg(feature = "jsonschema-validate")]
/// Resources an individual instrument schema can actually reach.
///
/// Every instrument schema `$ref`s only into `common/1` and `cashflow/1` — none
/// references another instrument schema (verified: 654 `common/1` and 21
/// `cashflow/1` absolute refs across all seventy, zero cross-instrument).
/// Registering all seventy instrument documents for a single-type validator
/// therefore costs memory without ever resolving a reference. The envelope
/// validator still needs the full set, because its `oneOf` branches point at
/// every instrument.
///
/// # Memory
///
/// A compiled `jsonschema::Validator` retains its whole resolver registry, and
/// `jsonschema` 0.28 offers no way to share one registry across validators. The
/// cache is therefore unbounded but per-type-lazy: measured from Python, warming
/// all seventy type validators costs ~477 MB over baseline with this trimmed set
/// versus ~1375 MB with the full one. A process that only touches the handful of
/// types it actually prices pays proportionally less. This is a deliberate trade
/// against the ~2800x latency win — rebuilding per call cost ~105 ms.
///
/// `registry_coverage::every_registered_instrument_satisfies_persistence_contract`
/// validates all seventy fixtures through this path, so a schema that grew a
/// cross-instrument `$ref` would fail there rather than silently mis-resolve.
fn instrument_ref_resources() -> finstack_quant_core::Result<Vec<(String, jsonschema::Resource)>> {
    let mut resources = common_schema_resources()?;
    resources.extend(finstack_quant_cashflows::schema::resources()?);
    Ok(resources)
}

#[cfg(feature = "jsonschema-validate")]
/// Compile `schema` against the supplied resolver resources.
///
/// This is the expensive step the caches below exist to amortize: each call
/// parses the `common/1` schemas, clones the cashflow resources and (for the
/// envelope) all embedded instrument schemas, then builds a resolver registry
/// over them.
fn build_validator_with(
    schema: &Value,
    context: &str,
    resources: Vec<(String, jsonschema::Resource)>,
) -> CachedValidator {
    jsonschema::options()
        .with_resources(resources.into_iter())
        .build(schema)
        .map_err(|e| {
            finstack_quant_core::Error::Validation(format!("Invalid {context} schema: {e}"))
        })
}

#[cfg(feature = "jsonschema-validate")]
/// Compile `schema` with every embedded resource registered.
///
/// Used for the envelope, whose `oneOf` reaches every instrument schema.
fn build_validator(schema: &Value, context: &str) -> CachedValidator {
    build_validator_with(schema, context, external_schema_resources()?)
}

#[cfg(feature = "jsonschema-validate")]
/// Compile a single instrument type's schema with only the resources it can reach.
fn build_instrument_validator(schema: &Value, context: &str) -> CachedValidator {
    build_validator_with(schema, context, instrument_ref_resources()?)
}

#[cfg(feature = "jsonschema-validate")]
/// Compiled validator for the instrument envelope schema.
///
/// # Why this is cached
///
/// Building a `jsonschema::Validator` compiles every embedded resource - the
/// twelve `common/1` schemas, the cashflow schemas and all instrument schemas -
/// into a resolver registry. Measured in release on this workspace that build
/// costs ~16 ms, while checking an instance against an already-built validator
/// costs ~1 us. Rebuilding the validator per call therefore dominated bulk
/// validation: a 5,000-instrument book spent minutes recompiling the same
/// schemas. Caching makes the compile a one-off.
///
/// `jsonschema::Validator` is `Send + Sync`, so lending `&'static` references
/// is sound from rayon workers and from Python callers that released the GIL.
/// `OnceLock` stores the compilation *result*, so a schema that fails to
/// compile reports the identical error on every call instead of being silently
/// retried or skipped.
fn instrument_envelope_validator() -> finstack_quant_core::Result<&'static jsonschema::Validator> {
    static VALIDATOR: OnceLock<CachedValidator> = OnceLock::new();
    VALIDATOR
        .get_or_init(|| build_validator(instrument_envelope_schema()?, "instrument envelope"))
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(feature = "jsonschema-validate")]
/// Per-instrument-type compiled validators, keyed by canonical registry tag.
///
/// Each tag owns its own `OnceLock`, so two instrument types can never share a
/// validator and each is compiled at most once. Slots are filled lazily rather
/// than all at once because a validator retains its own copy of the resource
/// registry (~20 MB measured): compiling all of them up front would cost about
/// a second and 1.4 GB for callers that touch only a handful of types.
fn instrument_validator_cache(
) -> &'static std::collections::BTreeMap<&'static str, OnceLock<CachedValidator>> {
    static CACHE: OnceLock<std::collections::BTreeMap<&'static str, OnceLock<CachedValidator>>> =
        OnceLock::new();
    CACHE.get_or_init(|| {
        instrument_schema_cache()
            .keys()
            .map(|tag| (*tag, OnceLock::new()))
            .collect()
    })
}

#[cfg(feature = "jsonschema-validate")]
/// Compiled validator for one canonical instrument tag.
///
/// # Errors
///
/// Returns `Error::Validation` if the type is not registered, if its embedded
/// schema is malformed, or if that schema fails to compile.
///
/// # Arguments
///
/// * `instrument_type` - Registered tagged-instrument type to look up.
fn instrument_type_validator(
    instrument_type: &str,
) -> finstack_quant_core::Result<&'static jsonschema::Validator> {
    let (tag, slot) = instrument_validator_cache()
        .get_key_value(instrument_type)
        .map(|(tag, slot)| (*tag, slot))
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "unknown instrument type '{instrument_type}'"
            ))
        })?;
    slot.get_or_init(|| build_instrument_validator(&instrument_schema(tag)?, tag))
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(feature = "jsonschema-validate")]
/// Collect every violation `validator` reports for `instance`.
fn validate_with(
    validator: &jsonschema::Validator,
    instance: &Value,
    context: &str,
) -> finstack_quant_core::Result<()> {
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| {
            let path = e.instance_path.to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{path}: {e}")
            }
        })
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "{context} validation failed with {} error(s):\n  {}",
            errors.len(),
            errors.join("\n  ")
        )))
    }
}

#[cfg(feature = "json-schema")]
use crate::instruments::json_loader::instrument_registry;
#[cfg(feature = "json-schema")]
use crate::instruments::{
    InstrumentEnvelope, InstrumentPricingOverrides, MetricPricingOverrides,
    ScenarioPricingOverrides,
};
#[cfg(feature = "json-schema")]
use finstack_quant_core::schema::{SchemaArtifact, SchemaKind};

#[cfg(feature = "json-schema")]
/// Canonical `Attributes` payloads.
fn attributes_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({"tags": ["rates", "hedge"], "meta": {"desk": "ny_rates"}}),
        serde_json::json!({}),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical `BusinessDayConvention` values.
fn business_day_convention_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("modified_following".to_string()),
        Value::String("following".to_string()),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical ISO 4217 codes.
fn currency_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("USD".to_string()),
        Value::String("EUR".to_string()),
        Value::String("JPY".to_string()),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical `Date` payloads.
fn date_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("2024-01-15".to_string()),
        Value::String("2034-01-15".to_string()),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical day-count labels.
fn day_count_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("act_360".to_string()),
        Value::String("30_360".to_string()),
        Value::String("act_act_isma".to_string()),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical identifier payloads.
///
/// Ids are opaque strings resolved against whatever market or registry the call
/// supplies; the examples show the spellings used throughout the fixtures.
fn id_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("USD-OIS".to_string()),
        Value::String("US912828XG33".to_string()),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical `Tenor` payloads.
fn tenor_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({"count": 3, "unit": "months"}),
        serde_json::json!({"count": 10, "unit": "years"}),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical `Diagnostic` payloads.
fn diagnostic_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![serde_json::json!({
        "code": "contract/version_mismatch",
        "message": "unsupported schema marker",
        "phase": "version",
        "severity": "error",
        "pointer": "/schema"
    })])
}

#[cfg(feature = "json-schema")]
/// Canonical `ValidationReport` payloads.
fn validation_report_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({"diagnostics": [], "truncated": false}),
        serde_json::json!({
            "diagnostics": [{
                "code": "contract/unknown_field",
                "message": "unknown field `maturity_date`",
                "phase": "structure",
                "severity": "error",
                "pointer": "/instrument/spec"
            }],
            "truncated": false
        }),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical pricing-override payloads.
///
/// All three override maps default to empty and are omitted when empty, so the
/// first example is the one almost every payload uses.
fn empty_override_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![serde_json::json!({})])
}

/// Canonical `ScenarioPricingOverrides` payloads.
#[cfg(feature = "json-schema")]
fn scenario_override_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({}),
        serde_json::json!({"scenario_spread_shock_bp": 150.0}),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical `ValuationResult`: a priced bond with its policy stamps.
fn valuation_result_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let as_of =
        finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 15)
            .map_err(|error| {
                finstack_quant_core::Error::Internal(format!("build example as-of date: {error}"))
            })?;
    let value = finstack_quant_core::money::Money::new(
        1_012_345.67,
        finstack_quant_core::currency::Currency::USD,
    )?;
    // `stamped` records a wall-clock timestamp, which would make the checked-in
    // artifact differ on every regeneration; the example carries a fixed meta.
    let meta = finstack_quant_core::config::ResultsMeta {
        timestamp: None,
        ..finstack_quant_core::config::results_meta_now(
            &finstack_quant_core::config::FinstackConfig::default(),
        )
    };
    let result =
        crate::results::ValuationResult::stamped_with_meta("US912828XG33", as_of, value, meta);
    let value = serde_json::to_value(&result).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize valuation result example: {error}"))
    })?;
    Ok(vec![value])
}

#[cfg(feature = "json-schema")]
/// Canonical examples for the instrument umbrella union.
fn instrument_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let mut examples = Vec::new();
    for entry in instrument_registry() {
        let entry_examples = entry.examples().map_err(|error| {
            finstack_quant_core::Error::Internal(format!(
                "build canonical {} instrument example: {error}",
                entry.tag
            ))
        })?;
        examples.extend(entry_examples);
    }
    Ok(examples)
}

#[cfg(feature = "json-schema")]
/// Canonical `Decimal` payloads.
///
/// Exact decimals are JSON *strings*; emitting a JSON number here is the most
/// common way to fail an otherwise well-formed payload, so the contract is shown
/// rather than only described.
fn decimal_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("1000000".to_string()),
        Value::String("0.0425".to_string()),
        Value::String("-1234.56789".to_string()),
    ])
}

#[cfg(feature = "json-schema")]
/// Canonical `Money` payloads, pairing a decimal string with an ISO 4217 code.
fn money_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({"amount": "1000000", "currency": "USD"}),
        serde_json::json!({"amount": "-2500.75", "currency": "EUR"}),
    ])
}

#[cfg(feature = "json-schema")]
/// The crate's schema registry as a shared, lazily built slice.
///
/// [`artifacts`] builds a fresh `Vec` because the instrument entries are
/// assembled at runtime; callers that need a borrow with a `'static` lifetime -
/// the language bindings, for one - use this cached view instead.
#[must_use]
pub fn artifacts_slice() -> &'static [SchemaArtifact] {
    static CACHE: OnceLock<Vec<SchemaArtifact>> = OnceLock::new();
    CACHE.get_or_init(artifacts)
}

#[cfg(feature = "json-schema")]
/// The crate's complete schema registry, sorted by artifact path.
///
/// This lives in the library, not the generator binary, so the generator, the
/// contract tests and the bindings all render from one definition. Render an
/// entry with [`SchemaArtifact::generate`].
#[must_use]
pub fn artifacts() -> Vec<SchemaArtifact> {
    let mut artifacts = vec![
        SchemaArtifact::new::<finstack_quant_core::types::Attributes>(
            "schemas/common/1/attributes.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "attributes.schema.json"
            ),
            "Attributes",
            "User-defined tags and key-value metadata for classification.",
        )
        .with_examples(attributes_examples),
        SchemaArtifact::new::<finstack_quant_core::contract::Diagnostic>(
            "schemas/common/1/diagnostic.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "diagnostic.schema.json"
            ),
            "Diagnostic",
            "One structured finding emitted while loading a persisted contract.",
        )
        .with_examples(diagnostic_examples),
        SchemaArtifact::new::<finstack_quant_core::contract::ValidationReport>(
            "schemas/common/1/validation_report.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "validation_report.schema.json"
            ),
            "Validation Report",
            "Bounded structured diagnostics emitted by persisted-contract validation.",
        )
        .with_examples(validation_report_examples),
        SchemaArtifact::new::<finstack_quant_core::dates::BusinessDayConvention>(
            "schemas/common/1/business_day_convention.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "business_day_convention.schema.json"
            ),
            "Business Day Convention",
            "Business-day adjustment convention.",
        )
        .with_examples(business_day_convention_examples),
        SchemaArtifact::new::<finstack_quant_core::currency::Currency>(
            "schemas/common/1/currency.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "currency.schema.json"
            ),
            "Currency",
            "ISO 4217 currency code.",
        )
        .with_examples(currency_examples),
        SchemaArtifact::new::<finstack_quant_core::wire::DateWire>(
            "schemas/common/1/date.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "date.schema.json"
            ),
            "Date",
            "ISO 8601 calendar date string.",
        )
        .with_examples(date_examples),
        SchemaArtifact::new::<finstack_quant_core::dates::DayCount>(
            "schemas/common/1/day_count.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "day_count.schema.json"
            ),
            "Day Count",
            "Day-count convention.",
        )
        .with_examples(day_count_examples),
        SchemaArtifact::new::<finstack_quant_core::wire::DecimalWire>(
            "schemas/common/1/decimal.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "decimal.schema.json"
            ),
            "Decimal",
            "Exact decimal encoded as a JSON string.",
        )
        .with_examples(decimal_examples),
        SchemaArtifact::new::<finstack_quant_core::types::InstrumentId>(
            "schemas/common/1/id.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "id.schema.json"
            ),
            "Id",
            "Opaque string identifier.",
        )
        .with_examples(id_examples),
        SchemaArtifact::new::<finstack_quant_core::money::Money>(
            "schemas/common/1/money.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "money.schema.json"
            ),
            "Money",
            "Currency-tagged monetary amount.",
        )
        .with_examples(money_examples),
        SchemaArtifact::new::<InstrumentPricingOverrides>(
            "schemas/common/1/instrument_pricing_overrides.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "instrument_pricing_overrides.schema.json"
            ),
            "Instrument Pricing Overrides",
            "Instrument-owned market quote and model configuration overrides.",
        )
        .with_examples(empty_override_examples),
        SchemaArtifact::new::<MetricPricingOverrides>(
            "schemas/common/1/metric_pricing_overrides.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "metric_pricing_overrides.schema.json"
            ),
            "Metric Pricing Overrides",
            "Metric-time pricing and finite-difference configuration.",
        )
        .with_examples(empty_override_examples),
        SchemaArtifact::new::<ScenarioPricingOverrides>(
            "schemas/common/1/scenario_pricing_overrides.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "scenario_pricing_overrides.schema.json"
            ),
            "Scenario Pricing Overrides",
            "Scenario-only price and spread shocks.",
        )
        .with_examples(scenario_override_examples),
        SchemaArtifact::new::<finstack_quant_core::dates::Tenor>(
            "schemas/common/1/tenor.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/common/1/",
                "tenor.schema.json"
            ),
            "Tenor",
            "Parsed financial tenor.",
        )
        .with_examples(tenor_examples),
        SchemaArtifact::new::<InstrumentEnvelope>(
            "schemas/instruments/1/instrument.schema.json",
            concat!(
                "https://finstack_quant.dev/schemas/instrument/1/",
                "instrument.schema.json"
            ),
            "Finstack Quant Instrument",
            "Canonical v1 envelope for every supported financial instrument.",
        )
        .with_packager(package_valuations_schema)
        .with_kind(SchemaKind::Input)
        .with_summary(
            "The seventy-branch union of every instrument type. Prefer the per-type schema \
             beside it: this document is too large to hand to a model.",
        )
        .with_examples(instrument_examples),
        SchemaArtifact::new::<crate::results::ValuationResult>(
            "schemas/results/1/valuation_result.schema.json",
            "https://finstack_quant.dev/schemas/results/1/valuation_result.schema.json",
            "Valuation Result",
            "Canonical valuation result containing PV and typed metrics.",
        )
        .with_packager(package_valuations_schema)
        .with_kind(SchemaKind::Output)
        .with_summary("PV plus requested measures, with rounding and FX policy stamps.")
        .with_examples(valuation_result_examples),
    ];

    artifacts.extend(
        instrument_registry()
            .into_iter()
            .map(|entry| entry.artifact),
    );
    artifacts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_schema_example(schema: &Value) -> Value {
        schema
            .get("examples")
            .and_then(Value::as_array)
            .and_then(|examples| examples.first())
            .cloned()
            .expect("schema should have at least one example")
    }

    /// Validate the way the pre-cache code did: compile a throwaway validator
    /// for every call. Used as the behavioural reference for the cached path.
    fn validate_uncached(
        instrument_type: &str,
        instance: &Value,
    ) -> finstack_quant_core::Result<()> {
        let schema = instrument_schema(instrument_type)?;
        let validator = build_validator(&schema, instrument_type)?;
        validate_with(&validator, instance, instrument_type)
    }

    fn bond_example() -> Value {
        first_schema_example(&instrument_schema("bond").expect("bond schema"))
    }

    #[test]
    fn test_cached_validator_matches_uncached_verdicts() {
        let valid = bond_example();

        let mut unknown_field = bond_example();
        unknown_field["instrument"]["spec"]["not_a_real_field"] = serde_json::json!(1);

        let mut malformed_scalar = bond_example();
        malformed_scalar["instrument"]["spec"]["notional"]["amount"] =
            serde_json::json!("not-a-decimal");

        // (instrument type, payload, expected verdict)
        let cases: [(&str, &Value, bool); 5] = [
            ("bond", &valid, true),
            // Cross-type: a bond envelope must not satisfy another type's schema.
            ("interest_rate_swap", &valid, false),
            ("deposit", &valid, false),
            ("bond", &unknown_field, false),
            ("bond", &malformed_scalar, false),
        ];

        for (instrument_type, instance, expect_ok) in cases {
            let cached = validate_instrument_type_json(instrument_type, instance);
            let uncached = validate_uncached(instrument_type, instance);
            assert_eq!(
                cached.is_ok(),
                expect_ok,
                "{instrument_type}: unexpected verdict: {cached:?}"
            );
            assert_eq!(
                cached, uncached,
                "{instrument_type}: cached and uncached validation must agree exactly"
            );
            // Repeat calls must be stable, not just the first one.
            assert_eq!(
                validate_instrument_type_json(instrument_type, instance),
                uncached,
                "{instrument_type}: repeated cached validation must stay identical"
            );
        }
    }

    #[test]
    fn test_cached_envelope_validation_keeps_its_verdicts() {
        let valid = bond_example();

        let mut unknown_field = bond_example();
        unknown_field["instrument"]["spec"]["not_a_real_field"] = serde_json::json!(1);

        let mut malformed_scalar = bond_example();
        malformed_scalar["instrument"]["spec"]["notional"]["amount"] =
            serde_json::json!("not-a-decimal");

        for (label, instance, expect_ok) in [
            ("valid", &valid, true),
            ("unknown field", &unknown_field, false),
            ("malformed scalar", &malformed_scalar, false),
        ] {
            let first = validate_instrument_envelope_json(instance);
            assert_eq!(first.is_ok(), expect_ok, "{label}: unexpected verdict");
            assert_eq!(
                validate_instrument_envelope_json(instance),
                first,
                "{label}: cached envelope validation must be stable across calls"
            );
        }
    }

    #[test]
    fn test_cached_validators_are_not_shared_across_instrument_types() {
        let bond = instrument_type_validator("bond").expect("bond validator");
        let swap = instrument_type_validator("interest_rate_swap").expect("swap validator");
        assert!(
            !std::ptr::eq(bond, swap),
            "each instrument type must get its own validator"
        );
        assert!(
            std::ptr::eq(
                bond,
                instrument_type_validator("bond").expect("bond validator")
            ),
            "repeat lookups must reuse the same cached validator"
        );
    }

    #[test]
    fn test_cached_validator_is_shared_across_threads() {
        let instance = bond_example();
        let address = |validator: &jsonschema::Validator| std::ptr::from_ref(validator) as usize;
        let expected = address(instrument_type_validator("bond").expect("bond validator"));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..20 {
                        validate_instrument_type_json("bond", &instance)
                            .expect("valid bond example should pass on any thread");
                    }
                    assert_eq!(
                        address(instrument_type_validator("bond").expect("bond validator")),
                        expected,
                        "all threads must observe the same cached validator"
                    );
                });
            }
        });
    }

    #[test]
    fn test_cached_compilation_failure_is_reported_on_every_call() {
        // A schema that cannot compile must keep failing with the same error
        // rather than being retried or silently forgotten by the cache.
        static SLOT: OnceLock<CachedValidator> = OnceLock::new();
        let broken = serde_json::json!({ "type": "not_a_json_schema_type" });
        let first = SLOT
            .get_or_init(|| build_validator(&broken, "broken"))
            .as_ref()
            .map_err(Clone::clone);
        let second = SLOT
            .get_or_init(|| build_validator(&broken, "broken"))
            .as_ref()
            .map_err(Clone::clone);
        let message = first
            .expect_err("broken schema must not compile")
            .to_string();
        assert!(
            message.contains("Invalid broken schema"),
            "unexpected message: {message}"
        );
        assert_eq!(
            message,
            second
                .expect_err("broken schema must not compile")
                .to_string(),
            "cached compilation failures must be replayed verbatim"
        );
    }

    /// Timing harness for the validator cache; run with
    /// `cargo nextest run -p finstack-quant-valuations --release
    /// --run-ignored all -E 'test(bench_validator_cache)' --no-capture`.
    #[test]
    #[ignore = "timing harness, not a correctness assertion"]
    fn bench_validator_cache() {
        use std::time::Instant;

        let instance = bond_example();
        let iterations = 50;

        // Pre-cache behaviour: rebuild the validator on every call.
        let start = Instant::now();
        for _ in 0..iterations {
            validate_uncached("bond", &instance).expect("valid");
        }
        let uncached = start.elapsed() / iterations;

        validate_instrument_type_json("bond", &instance).expect("warm the cache");
        let start = Instant::now();
        for _ in 0..iterations {
            validate_instrument_type_json("bond", &instance).expect("valid");
        }
        let cached = start.elapsed() / iterations;

        println!("uncached: {uncached:?}/call");
        println!("cached:   {cached:?}/call");
        println!(
            "speedup:  {:.0}x",
            uncached.as_secs_f64() / cached.as_secs_f64()
        );
    }

    #[test]
    fn test_schema_stubs() {
        // Verify stub schemas are valid JSON and have expected structure
        let bond = instrument_schema("bond").expect("bond schema should parse");
        assert_eq!(bond["$schema"], JSON_SCHEMA_DIALECT);
        assert_eq!(bond["title"], "bond");

        let envelope =
            instrument_envelope_schema().expect("instrument envelope schema should parse");
        assert_eq!(envelope["title"], "Finstack Quant Instrument");

        let result = valuation_result_schema().expect("valuation result schema should parse");
        assert_eq!(result["title"], "Valuation Result");
    }

    #[test]
    fn test_all_schemas_parse_successfully() {
        // Ensure all embedded schemas parse without error.
        // This test catches invalid JSON at CI time rather than runtime.
        assert!(
            instrument_schema("bond").is_ok(),
            "instrument_schema(\"bond\") should return Ok"
        );
        assert!(
            instrument_envelope_schema().is_ok(),
            "instrument_envelope_schema() should return Ok"
        );
        assert!(
            valuation_result_schema().is_ok(),
            "valuation_result_schema() should return Ok"
        );
    }

    #[test]
    fn test_instrument_types_lists_supported_tags() {
        let types = instrument_types().expect("instrument types should parse");
        let expected: Vec<String> = crate::instruments::json_loader::registry_tags()
            .iter()
            .map(|tag| (*tag).to_string())
            .collect();
        assert_eq!(types, expected);
        assert!(types.iter().any(|ty| ty == "bond"));
        assert!(types.iter().any(|ty| ty == "cms_swap"));
    }

    #[test]
    fn test_instrument_schema_returns_dedicated_schema_when_available() {
        let schema = instrument_schema("bond").expect("bond schema should load");
        assert_eq!(schema["title"], "bond");
        assert_eq!(
            schema["$id"],
            "https://finstack_quant.dev/schemas/instrument/1/fixed_income/bond.schema.json"
        );
    }

    #[test]
    fn test_all_envelope_types_have_dedicated_schemas() {
        let types = instrument_types().expect("instrument types should parse");
        for ty in &types {
            let schema = instrument_schema(ty)
                .unwrap_or_else(|e| panic!("schema for '{ty}' should load: {e}"));
            let desc = schema["description"]
                .as_str()
                .unwrap_or_else(|| panic!("schema for '{ty}' should have a description"));
            assert!(
                !desc.trim().is_empty(),
                "schema for '{ty}' should be documented"
            );
        }
    }

    #[test]
    fn test_instrument_schema_rejects_unknown_discriminator() {
        let err = instrument_schema("not_a_supported_instrument_type").expect_err("unknown type");
        let msg = err.to_string();
        assert!(
            msg.contains("unknown instrument type"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn test_instrument_schema_cache_covers_canonical_tags_only() {
        let bond = instrument_schema("bond").expect("bond");
        assert_eq!(bond["title"], "bond");
        let swap = instrument_schema("interest_rate_swap").expect("irs");
        assert_eq!(swap["title"], "interest_rate_swap");
        assert!(instrument_schema("interest_rate_option").is_err());
    }

    #[test]
    fn test_validate_instrument_json_accepts_valid_envelope() {
        let valid = first_schema_example(&instrument_schema("bond").expect("bond schema"));
        assert!(
            validate_instrument_envelope_json(&valid).is_ok(),
            "valid bond example should pass validation"
        );
    }

    #[test]
    fn test_validate_instrument_json_rejects_empty_typed_spec() {
        let invalid = serde_json::json!({
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "bond",
                "spec": {}
            }
        });
        let msg = validate_instrument_envelope_json(&invalid)
            .expect_err("empty bond spec should fail typed validation")
            .to_string();
        assert!(
            msg.contains("bond validation failed"),
            "error should mention typed bond validation: {msg}"
        );
    }

    #[test]
    fn test_validate_instrument_json_rejects_missing_schema() {
        let invalid = serde_json::json!({
            "instrument": { "type": "bond", "spec": {} }
        });
        let msg = validate_instrument_envelope_json(&invalid)
            .expect_err("missing 'schema' field should fail")
            .to_string();
        assert!(
            msg.contains("validation failed"),
            "error should mention validation: {msg}"
        );
    }

    #[test]
    fn near_miss_instrument_error_names_the_missing_field() {
        // Single-instrument schemas used to wrap their payload in a one-branch
        // `oneOf`, so a validator reported at `/instrument` and attached the
        // whole instance instead of naming the field. The shared emitter now
        // collapses that wrapper; this pins the diagnostic that unlocks.
        let mut payload = first_schema_example(&instrument_schema("bond").expect("bond schema"));
        payload
            .pointer_mut("/instrument/spec")
            .and_then(Value::as_object_mut)
            .expect("bond spec object")
            .remove("maturity");

        let message = validate_instrument_envelope_json(&payload)
            .expect_err("a bond without a maturity must fail")
            .to_string();

        assert!(
            message.contains("/instrument/spec"),
            "error must point at the failing object: {message}"
        );
        assert!(
            message.contains("maturity"),
            "error must name the missing field: {message}"
        );
        assert!(
            !message.contains("discount_curve_id"),
            "error must not attach the whole instance: {message}"
        );
    }

    #[test]
    fn test_validate_instrument_json_rejects_unknown_type() {
        let invalid = serde_json::json!({
            "schema": "finstack_quant.instrument/1",
            "instrument": { "type": "not_real", "spec": {} }
        });
        let err = validate_instrument_envelope_json(&invalid);
        assert!(err.is_err(), "unknown instrument type should fail");
    }
}
