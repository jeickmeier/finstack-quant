//! JSON Schema contract tests for scenario envelopes.

use finstack_quant_valuations::pricer::InstrumentType;
use serde_json::{json, Value};

/// Render through the registry: only that path applies the packager, the
/// single-branch-union collapse and examples, which is what is checked in.
fn generated_schema() -> Value {
    finstack_quant_scenarios::schema::ARTIFACTS[0]
        .generate()
        .expect("scenario envelope schema generates")
}

fn checked_in_schema() -> Value {
    serde_json::from_str(include_str!("../schemas/scenarios/1/scenario.schema.json"))
        .expect("checked-in scenario schema parses")
}

fn validate(schema: &Value, fixture: &Value) -> Result<(), Vec<String>> {
    let validator = jsonschema::validator_for(schema).expect("scenario schema compiles");
    let errors: Vec<_> = validator
        .iter_errors(fixture)
        .map(|error| error.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[test]
fn checked_in_envelope_schema_validates_canonical_fixture() {
    let fixture: Value = serde_json::from_str(include_str!("data/canonical/scenario.json"))
        .expect("canonical scenario fixture parses");

    validate(&checked_in_schema(), &fixture)
        .unwrap_or_else(|errors| panic!("canonical scenario failed validation: {errors:#?}"));
}

#[test]
fn checked_in_envelope_schema_rejects_malformed_nested_operation() {
    let mut fixture: Value = serde_json::from_str(include_str!("data/canonical/scenario.json"))
        .expect("canonical scenario fixture parses");
    fixture["scenario"]["operations"][0] = json!({
        "kind": "curve_parallel_bp",
        "curve_kind": "discount",
        "curve_id": "USD-OIS",
        "bp": "fifty"
    });

    assert!(
        validate(&checked_in_schema(), &fixture).is_err(),
        "typed scenario schema must reject an invalid nested operation"
    );
}

#[test]
fn checked_in_envelope_schema_rejects_invalid_instrument_type() {
    let mut fixture: Value = serde_json::from_str(include_str!("data/canonical/scenario.json"))
        .expect("canonical scenario fixture parses");
    fixture["scenario"]["operations"][0] = json!({
        "kind": "instrument_price_pct_by_type",
        "instrument_types": ["NotAnInstrumentType"],
        "pct": -5.0
    });

    assert!(
        validate(&checked_in_schema(), &fixture).is_err(),
        "typed scenario schema must reject an unknown instrument type"
    );
}

#[test]
fn checked_in_envelope_schema_accepts_serde_instrument_type_value() {
    let mut fixture: Value = serde_json::from_str(include_str!("data/canonical/scenario.json"))
        .expect("canonical scenario fixture parses");
    let instrument_type =
        serde_json::to_value(InstrumentType::Bond).expect("instrument type serializes");
    assert_eq!(instrument_type, json!("bond"));
    fixture["scenario"]["operations"][0] = json!({
        "kind": "instrument_price_pct_by_type",
        "instrument_types": [instrument_type],
        "pct": -5.0
    });

    validate(&checked_in_schema(), &fixture)
        .unwrap_or_else(|errors| panic!("serde instrument type failed validation: {errors:#?}"));
}

#[test]
fn checked_in_envelope_schema_rejects_wrong_contract_marker() {
    let mut fixture: Value = serde_json::from_str(include_str!("data/canonical/scenario.json"))
        .expect("canonical scenario fixture parses");
    fixture["schema"] = json!("finstack_quant.scenario/999");

    assert!(
        validate(&checked_in_schema(), &fixture).is_err(),
        "scenario schema must require the current contract marker"
    );
}

#[test]
fn checked_in_schema_matches_generated_type_and_metadata() {
    let schema = checked_in_schema();

    assert_eq!(schema, generated_schema());
    assert_eq!(
        schema["$id"],
        "https://finstack_quant.dev/schemas/scenarios/1/scenario.schema.json"
    );
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn checked_in_schema_keeps_meaningful_nested_validation() {
    let schema = checked_in_schema();

    assert!(schema.pointer("/$defs/ScenarioSpec").is_some());
    assert!(schema
        .pointer("/$defs/InstrumentType/oneOf")
        .and_then(Value::as_array)
        .is_some_and(|variants| variants.len() > 50));
    assert!(schema
        .pointer("/$defs/OperationSpec/oneOf")
        .and_then(Value::as_array)
        .is_some_and(|variants| variants.len() > 10));
    assert_eq!(
        schema.pointer("/properties/scenario/$ref"),
        Some(&json!("#/$defs/ScenarioSpec"))
    );
}

/// Read at run time, not with `include_str!`, so the generator binaries in
/// this test build still compile before the artifact is first written.
fn checked_in_spec_schema() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas/scenarios/1/scenario_spec.schema.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("read scenario spec schema"))
        .expect("checked-in scenario spec schema parses")
}

#[test]
fn scenario_spec_artifact_is_the_envelope_scenario_standing_alone() {
    let schema = checked_in_spec_schema();
    let generated = finstack_quant_scenarios::schema::ARTIFACTS
        .iter()
        .find(|artifact| artifact.id.ends_with("/scenario_spec.schema.json"))
        .expect("scenario_spec artifact is registered")
        .generate()
        .expect("scenario spec schema generates");

    assert_eq!(schema, generated);
    assert_eq!(
        schema["$id"],
        "https://finstack_quant.dev/schemas/scenarios/1/scenario_spec.schema.json"
    );
    assert!(schema
        .pointer("/$defs/OperationSpec/oneOf")
        .and_then(Value::as_array)
        .is_some_and(|variants| variants.len() > 10));

    let example = schema["examples"][0].clone();
    assert_eq!(
        example,
        checked_in_schema()["examples"][0]["scenario"],
        "the standalone example is the envelope example's scenario payload"
    );
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_value(example.clone()).expect("example deserializes");
    assert_eq!(
        serde_json::to_value(&spec).expect("spec serializes"),
        example,
        "example round-trips through the Rust type"
    );

    let fixture: Value = serde_json::from_str(include_str!("data/canonical/scenario.json"))
        .expect("canonical scenario fixture parses");
    validate(&schema, &fixture["scenario"])
        .unwrap_or_else(|errors| panic!("canonical scenario spec failed validation: {errors:#?}"));
    assert!(
        validate(&schema, &fixture).is_err(),
        "the bare spec schema must reject the persistence envelope"
    );

    let mut unknown_field = schema["examples"][0].clone();
    unknown_field["unexpected"] = json!(true);
    assert!(validate(&schema, &unknown_field).is_err());
}
