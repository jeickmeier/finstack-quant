//! Cached JSON-Schema lookup and runtime validation for valuations payloads.

#[cfg(feature = "json-schema")]
use super::registry::artifacts_slice;
#[cfg(feature = "json-schema")]
use finstack_quant_core::schema::SchemaArtifact;

#[cfg(feature = "json-schema")]
use serde_json::Value;
#[cfg(feature = "json-schema")]
use std::sync::OnceLock;

#[cfg(feature = "json-schema")]
const INSTRUMENT_ENVELOPE_PATH: &str = "schemas/instruments/1/instrument.schema.json";

#[cfg(feature = "json-schema")]
const VALUATION_RESULT_PATH: &str = "schemas/results/1/valuation_result.schema.json";

#[cfg(feature = "json-schema")]
const INSTRUMENT_SCHEMA_PREFIX: &str = "schemas/instruments/1/";

#[cfg(feature = "json-schema")]
pub(super) fn artifact_by_path(
    relative_path: &str,
) -> finstack_quant_core::Result<&'static SchemaArtifact> {
    artifacts_slice()
        .iter()
        .find(|artifact| artifact.relative_path == relative_path)
        .ok_or_else(|| {
            finstack_quant_core::Error::Internal(format!(
                "schema artifact missing from valuations registry: {relative_path}"
            ))
        })
}

#[cfg(feature = "json-schema")]
pub(super) fn instrument_artifacts()
-> &'static std::collections::BTreeMap<&'static str, &'static SchemaArtifact> {
    static MAP: OnceLock<std::collections::BTreeMap<&'static str, &'static SchemaArtifact>> =
        OnceLock::new();
    MAP.get_or_init(|| {
        artifacts_slice()
            .iter()
            .filter(|artifact| {
                artifact.relative_path.starts_with(INSTRUMENT_SCHEMA_PREFIX)
                    && artifact.relative_path != INSTRUMENT_ENVELOPE_PATH
            })
            .map(|artifact| (artifact.title, artifact))
            .collect()
    })
}

#[cfg(feature = "json-schema")]
pub(super) fn instrument_artifact(
    instrument_type: &str,
) -> finstack_quant_core::Result<&'static SchemaArtifact> {
    instrument_artifacts()
        .get(instrument_type)
        .copied()
        .ok_or_else(|| unknown_instrument_type(instrument_type))
}

#[cfg(feature = "jsonschema-validate")]
pub(super) fn validator_cache() -> &'static finstack_quant_core::schema::ValidatorCache {
    static CACHE: OnceLock<finstack_quant_core::schema::ValidatorCache> = OnceLock::new();
    CACHE.get_or_init(|| {
        finstack_quant_core::schema::ValidatorCache::new(
            artifacts_slice()
                .iter()
                .chain(finstack_quant_cashflows::schema::ARTIFACTS.iter())
                .chain(finstack_quant_core::schema::ARTIFACTS.iter()),
        )
    })
}

#[cfg(feature = "jsonschema-validate")]
pub(super) fn cached_document(
    artifact: &'static SchemaArtifact,
) -> finstack_quant_core::Result<&'static Value> {
    validator_cache().document(artifact.id)?.ok_or_else(|| {
        finstack_quant_core::Error::Internal(format!(
            "schema artifact missing from valuations cache: {}",
            artifact.id
        ))
    })
}

#[cfg(all(feature = "json-schema", not(feature = "jsonschema-validate")))]
fn cached_document(
    artifact: &'static SchemaArtifact,
) -> finstack_quant_core::Result<&'static Value> {
    static CACHE: OnceLock<
        std::collections::BTreeMap<&'static str, OnceLock<finstack_quant_core::Result<Value>>>,
    > = OnceLock::new();
    let slots = CACHE.get_or_init(|| {
        artifacts_slice()
            .iter()
            .map(|entry| (entry.id, OnceLock::new()))
            .collect()
    });
    let slot = slots.get(artifact.id).ok_or_else(|| {
        finstack_quant_core::Error::Internal(format!(
            "schema artifact missing from valuations cache: {}",
            artifact.id
        ))
    })?;
    slot.get_or_init(|| artifact.generate())
        .as_ref()
        .map_err(Clone::clone)
}

/// Get the JSON Schema for the instrument envelope.
///
/// Rendered on demand from the crate's schema registry and cached for the
/// process lifetime.
///
/// # Errors
///
/// Returns an error if the envelope schema cannot be generated.
#[cfg(feature = "json-schema")]
pub fn instrument_envelope_schema() -> finstack_quant_core::Result<&'static Value> {
    cached_document(artifact_by_path(INSTRUMENT_ENVELOPE_PATH)?)
}

/// Return every canonical instrument discriminator, in registry order.
///
/// The tagged-JSON registry is the source of truth for decoding and schema
/// generation, so this is the authoritative set of values accepted in an
/// envelope's `instrument.type` field; it does not infer type names by
/// parsing checked-in `$ref` paths.
#[must_use]
pub fn instrument_types() -> &'static [&'static str] {
    crate::instruments::json_loader::INSTRUMENT_TYPES
}

/// Get the JSON Schema for a single instrument type.
///
/// Returns the dedicated generated schema for a canonical registry tag,
/// rendered lazily on first request and cached for the process lifetime.
///
/// # Errors
///
/// Returns an error if the schema cannot be generated, and
/// `InputError::NotFound` if the requested instrument type is not registered.
///
/// # Arguments
///
/// * `instrument_type` - Canonical registered tagged-instrument type string.
#[cfg(feature = "json-schema")]
pub fn instrument_schema(instrument_type: &str) -> finstack_quant_core::Result<Value> {
    cached_document(instrument_artifact(instrument_type)?).cloned()
}

/// The lookup miss reported for a discriminator that is not in the registry.
#[cfg(feature = "json-schema")]
fn unknown_instrument_type(instrument_type: &str) -> finstack_quant_core::Error {
    finstack_quant_core::InputError::NotFound {
        id: format!(
            "unknown instrument type '{instrument_type}'; call instrument_types() for the valid set"
        ),
    }
    .into()
}

/// Get JSON-Schema for ValuationResult.
///
/// Returns schema for valuation result envelope (PV + metrics), rendered on
/// demand and cached for the process lifetime.
///
/// # Errors
///
/// Returns an error if the valuation-result schema cannot be generated.
#[cfg(feature = "json-schema")]
pub fn valuation_result_schema() -> finstack_quant_core::Result<&'static Value> {
    cached_document(artifact_by_path(VALUATION_RESULT_PATH)?)
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
///             }, "quote_compounding": {"periodic": 2},
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
    let envelope_artifact = artifact_by_path(INSTRUMENT_ENVELOPE_PATH)?;
    let cache = validator_cache();
    let envelope_validator = cache.validator(envelope_artifact.id)?.ok_or_else(|| {
        finstack_quant_core::Error::Internal(format!(
            "schema artifact missing from valuations cache: {}",
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
        return finstack_quant_core::schema::format_validation_result(
            &failures,
            "instrument envelope",
        );
    }

    let instrument_type = instrument_type.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "instrument envelope validation passed but instrument.type is missing".to_string(),
        )
    })?;

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
    let artifact = instrument_artifact(instrument_type)?;
    let failures = validator_cache().validate_artifact(artifact, instance)?;
    finstack_quant_core::schema::format_validation_result(&failures, instrument_type)
}
