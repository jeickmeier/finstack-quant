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

/// Parse embedded JSON schema at compile time, returning a Result.
/// The JSON is embedded via `include_str!` so the content is always present,
/// but parsing can still fail if the JSON is malformed.
macro_rules! try_include_schema {
    ($path:literal) => {
        serde_json::from_str::<Value>(include_str!($path))
            .map_err(|e| format!("invalid schema JSON at {}: {}", $path, e))
    };
}

macro_rules! build_schema_cache {
    (
        []
        $(plain: $variant:ident($ty:ty) => $tag:literal @ $schema_path:literal $(, $alias:literal)*;)*
        $(boxed: $boxed_variant:ident($boxed_ty:ty) => $boxed_tag:literal @ $boxed_schema_path:literal $(, $boxed_alias:literal)*;)*
    ) => {{
        let mut cache = std::collections::BTreeMap::new();
        $(
            let schema = try_include_schema!($schema_path);
            cache.insert($tag, schema.clone());
            $(
                cache.insert($alias, schema.clone());
            )*
        )*
        $(
            let schema = try_include_schema!($boxed_schema_path);
            cache.insert($boxed_tag, schema.clone());
            $(
                cache.insert($boxed_alias, schema.clone());
            )*
        )*
        cache
    }};
}

/// Get JSON-Schema for Bond configuration.
///
/// Sourced from the generated instrument schemas under `schemas/instruments/1/`.
///
/// # Errors
///
/// Returns `Error::Validation` if the embedded schema JSON is malformed.
#[allow(dead_code)] // Public API, used in tests
pub fn bond_schema() -> finstack_quant_core::Result<&'static Value> {
    static SCHEMA: OnceLock<Result<Value, String>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| {
            try_include_schema!("../schemas/instruments/1/fixed_income/bond.schema.json")
        })
        .as_ref()
        .map_err(|e| finstack_quant_core::Error::Validation(e.clone()))
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
        crate::instruments::json_loader::with_instrument_json_registry!(build_schema_cache)
    })
}

fn fallback_instrument_schema(instrument_type: &str) -> Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": format!("{instrument_type} (generic)"),
        "description": format!(
            "Fallback schema for instrument type '{instrument_type}'. Dedicated schema is not yet available; 'spec' remains untyped."
        ),
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "schema": {
                "const": "finstack_quant.instrument/1",
                "type": "string"
            },
            "instrument": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "type": {
                        "const": instrument_type,
                        "type": "string"
                    },
                    "spec": {
                        "type": "object",
                        "description": "Dedicated schema unavailable; accepts any object payload."
                    }
                },
                "required": ["type", "spec"]
            }
        },
        "required": ["schema", "instrument"]
    })
}

/// Return the canonical instrument discriminators advertised by the envelope schema.
///
/// # Errors
///
/// Returns `Error::Validation` if the embedded schema JSON is malformed.
pub fn instrument_types() -> finstack_quant_core::Result<Vec<String>> {
    let schema = instrument_envelope_schema()?;
    let Some(values) = schema
        .pointer("/properties/instrument/properties/type/enum")
        .and_then(serde_json::Value::as_array)
    else {
        return Err(finstack_quant_core::Error::Validation(
            "instrument schema enum is missing".to_string(),
        ));
    };

    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "instrument schema enum contains a non-string value".to_string(),
                )
            })
        })
        .collect()
}

/// Get the JSON Schema for a single instrument type.
///
/// Returns a dedicated schema when available. For supported instrument tags
/// without a dedicated checked-in schema file, returns a fallback tagged schema
/// with an untyped `spec` payload.
///
/// # Errors
///
/// Returns `Error::Validation` if the embedded schema JSON is malformed or the
/// requested instrument type is not supported.
pub fn instrument_schema(instrument_type: &str) -> finstack_quant_core::Result<Value> {
    if let Some(schema) = instrument_schema_cache().get(instrument_type) {
        return schema
            .as_ref()
            .cloned()
            .map_err(|e| finstack_quant_core::Error::Validation(e.clone()));
    }

    if instrument_types()?.iter().any(|ty| ty == instrument_type) {
        return Ok(fallback_instrument_schema(instrument_type));
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

/// Validate an instrument JSON value against the envelope schema.
///
/// Returns `Ok(())` if the JSON conforms to the instrument envelope schema,
/// or a detailed `Error::Validation` listing all schema violations.
///
/// # Example
///
/// ```ignore
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
///                 "Fixed": {
///                     "coupon_type": "Cash",
///                     "freq": { "count": 6, "unit": "months" },
///                     "dc": "ActActIsma",
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
pub fn validate_instrument_envelope_json(instance: &Value) -> finstack_quant_core::Result<()> {
    let schema = instrument_envelope_schema()?;
    validate_against_schema(instance, schema, "instrument envelope")?;

    let instrument_type = instance
        .pointer("/instrument/type")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "instrument envelope validation passed but instrument.type is missing".to_string(),
            )
        })?;

    validate_instrument_type_json(instrument_type, instance)
}

/// Validate a JSON value against a specific instrument type's schema.
///
/// # Errors
///
/// Returns `Error::Validation` if the JSON does not conform to the schema.
pub fn validate_instrument_type_json(
    instrument_type: &str,
    instance: &Value,
) -> finstack_quant_core::Result<()> {
    let schema = instrument_schema(instrument_type)?;
    validate_against_schema(instance, &schema, instrument_type)
}

/// Validate a JSON value against an arbitrary schema.
fn validate_against_schema(
    instance: &Value,
    schema: &Value,
    context: &str,
) -> finstack_quant_core::Result<()> {
    let validator = jsonschema::validator_for(schema).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("Invalid {context} schema: {e}"))
    })?;

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

    #[test]
    fn test_schema_stubs() {
        // Verify stub schemas are valid JSON and have expected structure
        let bond = bond_schema().expect("bond schema should parse");
        assert_eq!(bond["$schema"], "http://json-schema.org/draft-07/schema#");
        assert_eq!(bond["title"], "Bond");

        let envelope =
            instrument_envelope_schema().expect("instrument envelope schema should parse");
        assert_eq!(envelope["title"], "Finstack Quant Instrument");

        let result = valuation_result_schema().expect("valuation result schema should parse");
        assert_eq!(result["title"], "ValuationResult");
    }

    #[test]
    fn test_all_schemas_parse_successfully() {
        // Ensure all embedded schemas parse without error.
        // This test catches invalid JSON at CI time rather than runtime.
        assert!(bond_schema().is_ok(), "bond_schema() should return Ok");
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
        assert!(types.iter().any(|ty| ty == "bond"));
        assert!(types.iter().any(|ty| ty == "cms_swap"));
    }

    #[test]
    fn test_instrument_schema_returns_dedicated_schema_when_available() {
        let schema = instrument_schema("bond").expect("bond schema should load");
        assert_eq!(schema["title"], "Bond");
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
                !desc.contains("Dedicated schema is not yet available"),
                "'{ty}' is using a fallback schema — add a dedicated schema file"
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
    fn test_instrument_schema_cache_covers_registered_aliases() {
        let bond = instrument_schema("bond").expect("bond");
        assert_eq!(bond["title"], "Bond");
        let swap = instrument_schema("interest_rate_swap").expect("irs");
        assert_eq!(swap["title"], "Interest Rate Swap");
        let cap_floor_alias = instrument_schema("interest_rate_option").expect("cap/floor alias");
        assert_eq!(cap_floor_alias["title"], "Cap Floor");
    }

    #[test]
    fn test_validate_instrument_json_accepts_valid_envelope() {
        let valid = first_schema_example(bond_schema().expect("bond schema"));
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
    fn test_validate_instrument_json_rejects_unknown_type() {
        let invalid = serde_json::json!({
            "schema": "finstack_quant.instrument/1",
            "instrument": { "type": "not_real", "spec": {} }
        });
        let err = validate_instrument_envelope_json(&invalid);
        assert!(err.is_err(), "unknown instrument type should fail");
    }
}
