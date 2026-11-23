#[cfg(feature = "json-schema")]
use super::validate::instrument_artifact;
#[cfg(feature = "jsonschema-validate")]
use super::validate::validator_cache;
use super::*;
#[cfg(feature = "json-schema")]
use finstack_quant_core::schema::JSON_SCHEMA_DIALECT;
#[cfg(feature = "json-schema")]
use serde_json::Value;
#[cfg(feature = "jsonschema-validate")]
use std::sync::OnceLock;

#[cfg(feature = "json-schema")]
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
#[cfg(feature = "jsonschema-validate")]
fn validate_uncached(instrument_type: &str, instance: &Value) -> finstack_quant_core::Result<()> {
    let schema = instrument_schema(instrument_type)?;
    let cache = validator_cache();
    let validator = cache.build_validator(&schema)?;
    let failures = cache.collect_failures(&validator, &schema, instance);
    finstack_quant_core::schema::format_validation_result(&failures, instrument_type)
}

#[cfg(feature = "jsonschema-validate")]
fn cached_instrument_validator(
    instrument_type: &str,
) -> finstack_quant_core::Result<&'static jsonschema::Validator> {
    let artifact = instrument_artifact(instrument_type)?;
    validator_cache().validator(artifact.id)?.ok_or_else(|| {
        finstack_quant_core::Error::Internal(format!(
            "schema artifact missing from valuations cache: {}",
            artifact.id
        ))
    })
}

#[cfg(feature = "json-schema")]
fn bond_example() -> Value {
    first_schema_example(&instrument_schema("bond").expect("bond schema"))
}

#[cfg(feature = "jsonschema-validate")]
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

/// Validates `instance` through the cached envelope path twice and checks
/// the verdict and its stability across calls.
#[cfg(feature = "jsonschema-validate")]
fn assert_envelope_verdict_is_stable(label: &str, instance: &Value, expect_ok: bool) {
    let first = validate_instrument_envelope_json(instance);
    assert_eq!(first.is_ok(), expect_ok, "{label}: unexpected verdict");
    assert_eq!(
        validate_instrument_envelope_json(instance),
        first,
        "{label}: cached envelope validation must be stable across calls"
    );
}

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_cached_envelope_validation_keeps_its_verdict_for_a_valid_example() {
    assert_envelope_verdict_is_stable("valid", &bond_example(), true);
}

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_cached_envelope_validation_keeps_its_verdict_for_an_unknown_field() {
    let mut unknown_field = bond_example();
    unknown_field["instrument"]["spec"]["not_a_real_field"] = serde_json::json!(1);
    assert_envelope_verdict_is_stable("unknown field", &unknown_field, false);
}

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_cached_envelope_validation_keeps_its_verdict_for_a_malformed_scalar() {
    let mut malformed_scalar = bond_example();
    malformed_scalar["instrument"]["spec"]["notional"]["amount"] =
        serde_json::json!("not-a-decimal");
    assert_envelope_verdict_is_stable("malformed scalar", &malformed_scalar, false);
}

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_cached_validators_are_not_shared_across_instrument_types() {
    let bond = cached_instrument_validator("bond").expect("bond validator");
    let swap = cached_instrument_validator("interest_rate_swap").expect("swap validator");
    assert!(
        !std::ptr::eq(bond, swap),
        "each instrument type must get its own validator"
    );
    assert!(
        std::ptr::eq(
            bond,
            cached_instrument_validator("bond").expect("bond validator")
        ),
        "repeat lookups must reuse the same cached validator"
    );
}

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_cached_validator_is_shared_across_threads() {
    let instance = bond_example();
    let address = |validator: &jsonschema::Validator| std::ptr::from_ref(validator) as usize;
    let expected = address(cached_instrument_validator("bond").expect("bond validator"));
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..20 {
                    validate_instrument_type_json("bond", &instance)
                        .expect("valid bond example should pass on any thread");
                }
                assert_eq!(
                    address(cached_instrument_validator("bond").expect("bond validator")),
                    expected,
                    "all threads must observe the same cached validator"
                );
            });
        }
    });
}

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_cached_compilation_failure_is_reported_on_every_call() {
    // A schema that cannot compile must keep failing with the same error
    // rather than being retried or silently forgotten by the cache.
    static SLOT: OnceLock<finstack_quant_core::Result<jsonschema::Validator>> = OnceLock::new();
    let broken = serde_json::json!({ "type": "not_a_json_schema_type" });
    let first = SLOT
        .get_or_init(|| validator_cache().build_validator(&broken))
        .as_ref()
        .map_err(Clone::clone);
    let second = SLOT
        .get_or_init(|| validator_cache().build_validator(&broken))
        .as_ref()
        .map_err(Clone::clone);
    let message = first
        .expect_err("broken schema must not compile")
        .to_string();
    assert!(
        message.contains("compile schema"),
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
#[cfg(feature = "jsonschema-validate")]
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

#[cfg(feature = "json-schema")]
#[test]
fn test_schema_stubs() {
    // Verify stub schemas are valid JSON and have expected structure
    let bond = instrument_schema("bond").expect("bond schema should parse");
    assert_eq!(bond["$schema"], JSON_SCHEMA_DIALECT);
    assert_eq!(bond["title"], "bond");

    let envelope = instrument_envelope_schema().expect("instrument envelope schema should parse");
    assert_eq!(envelope["title"], "Finstack Quant Instrument");

    let result = valuation_result_schema().expect("valuation result schema should parse");
    assert_eq!(result["title"], "Valuation Result");
}

#[cfg(feature = "json-schema")]
#[test]
fn test_all_schemas_parse_successfully() {
    // Ensure all generated schemas render without error.
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
    let types = instrument_types();
    assert_eq!(
        types.len(),
        crate::instruments::json_loader::instrument_registry().len()
    );
    assert!(types.contains(&"bond"));
    assert!(types.contains(&"cms_swap"));
}

#[cfg(feature = "json-schema")]
#[test]
fn test_instrument_schema_returns_dedicated_schema_when_available() {
    let schema = instrument_schema("bond").expect("bond schema should load");
    assert_eq!(schema["title"], "bond");
    assert_eq!(
        schema["$id"],
        "https://finstack_quant.dev/schemas/instrument/1/fixed_income/bond.schema.json"
    );
}

#[cfg(feature = "json-schema")]
#[test]
fn test_all_envelope_types_have_dedicated_schemas() {
    for ty in instrument_types() {
        let schema =
            instrument_schema(ty).unwrap_or_else(|e| panic!("schema for '{ty}' should load: {e}"));
        let desc = schema["description"]
            .as_str()
            .unwrap_or_else(|| panic!("schema for '{ty}' should have a description"));
        assert!(
            !desc.trim().is_empty(),
            "schema for '{ty}' should be documented"
        );
    }
}

#[cfg(feature = "json-schema")]
#[test]
fn test_instrument_schema_rejects_unknown_discriminator() {
    let err = instrument_schema("not_a_supported_instrument_type").expect_err("unknown type");
    assert_eq!(err.kind(), finstack_quant_core::ErrorKind::NotFound);
    let msg = err.to_string();
    assert!(
        msg.contains("unknown instrument type"),
        "unexpected message: {msg}"
    );
}

#[cfg(feature = "json-schema")]
#[test]
fn test_instrument_schema_cache_covers_canonical_tags_only() {
    let bond = instrument_schema("bond").expect("bond");
    assert_eq!(bond["title"], "bond");
    let swap = instrument_schema("interest_rate_swap").expect("irs");
    assert_eq!(swap["title"], "interest_rate_swap");
    assert!(instrument_schema("interest_rate_option").is_err());
}

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_validate_instrument_json_accepts_valid_envelope() {
    let valid = first_schema_example(&instrument_schema("bond").expect("bond schema"));
    assert!(
        validate_instrument_envelope_json(&valid).is_ok(),
        "valid bond example should pass validation"
    );
}

#[cfg(feature = "jsonschema-validate")]
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

#[cfg(feature = "jsonschema-validate")]
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

#[cfg(feature = "jsonschema-validate")]
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

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_validate_instrument_json_rejects_unknown_type() {
    let invalid = serde_json::json!({
        "schema": "finstack_quant.instrument/1",
        "instrument": { "type": "not_real", "spec": {} }
    });
    let err = validate_instrument_envelope_json(&invalid);
    assert!(err.is_err(), "unknown instrument type should fail");
}

#[cfg(feature = "jsonschema-validate")]
#[test]
fn test_validate_instrument_envelope_rejects_malformed_date_format() {
    let mut payload = bond_example();
    payload["instrument"]["spec"]["maturity"] = serde_json::json!("01/15/2034");

    let envelope_err = validate_instrument_envelope_json(&payload)
        .expect_err("malformed date must fail envelope validation")
        .to_string();
    assert!(
        envelope_err.contains("/instrument/spec/maturity"),
        "envelope error must point at maturity: {envelope_err}"
    );

    let typed_err = validate_instrument_type_json("bond", &payload)
        .expect_err("malformed date must fail typed validation")
        .to_string();
    assert!(
        typed_err.contains("/instrument/spec/maturity"),
        "typed error must point at maturity: {typed_err}"
    );
}
