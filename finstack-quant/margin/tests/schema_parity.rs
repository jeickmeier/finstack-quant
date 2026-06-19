//! Schema parity tests for checked-in margin JSON schemas.

use serde_json::Value;

const JSON_SCHEMA_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

fn margin_schema() -> Value {
    let schema_json = include_str!("../schemas/margin/1/margin.schema.json");
    serde_json::from_str(schema_json).expect("Schema JSON should be valid")
}

/// Extract enum variant names from a schemars-generated enum schema.
fn extract_enum_values(schema: &Value) -> Vec<&str> {
    if let Some(arr) = schema.get("enum").and_then(|v| v.as_array()) {
        return arr.iter().filter_map(|v| v.as_str()).collect();
    }
    if let Some(arr) = schema.get("oneOf").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|v| v.get("const").and_then(|c| c.as_str()))
            .collect();
    }
    Vec::new()
}

fn assert_enum_parity(schema_name: &str, mut actual: Vec<&str>, expected: &[&str]) {
    let mut expected: Vec<&str> = expected.to_vec();
    expected.sort();
    actual.sort();

    if actual != expected {
        let missing: Vec<&&str> = expected.iter().filter(|t| !actual.contains(t)).collect();
        let extra: Vec<&&str> = actual.iter().filter(|t| !expected.contains(t)).collect();
        panic!(
            "{schema_name} schema enum mismatch!\n  Expected: {expected:?}\n  Actual:   {actual:?}\n  Missing:  {missing:?}\n  Extra:    {extra:?}"
        );
    }
}

fn assert_described_one_of_enum(schema_name: &str, schema: &Value) {
    assert!(
        schema.get("enum").is_none(),
        "{schema_name} should use oneOf/const variants instead of enum"
    );
    let variants = schema
        .get("oneOf")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{schema_name} should declare oneOf variants"));
    assert!(
        variants.iter().all(|variant| {
            variant.get("const").and_then(Value::as_str).is_some()
                && variant.get("description").and_then(Value::as_str).is_some()
        }),
        "{schema_name} oneOf variants should have const and description"
    );
}

/// Canonical IM methodologies (schemars uses the serde variant names).
const CANONICAL_IM_METHODOLOGIES: &[&str] = &[
    "ClearingHouse",
    "Haircut",
    "InternalModel",
    "Schedule",
    "Simm",
];

const CANONICAL_MARGIN_TENORS: &[&str] = &["Daily", "Monthly", "OnDemand", "Weekly"];

#[test]
fn margin_schema_declares_2020_12_dialect() {
    let schema = margin_schema();
    assert_eq!(
        schema.get("$schema").and_then(Value::as_str),
        Some(JSON_SCHEMA_2020_12),
        "margin schema declares the wrong JSON Schema dialect"
    );
}

#[test]
fn margin_im_methodology_schema_parity() {
    let schema = margin_schema();

    let im = schema
        .pointer("/$defs/ImMethodology")
        .or_else(|| schema.pointer("/definitions/ImMethodology"))
        .expect("ImMethodology should exist in schema");

    let values = extract_enum_values(im);
    assert_enum_parity("ImMethodology", values, CANONICAL_IM_METHODOLOGIES);
}

#[test]
fn margin_tenor_schema_parity() {
    let schema = margin_schema();

    let mt = schema
        .pointer("/$defs/MarginTenor")
        .or_else(|| schema.pointer("/definitions/MarginTenor"))
        .expect("MarginTenor should exist in schema");

    let values = extract_enum_values(mt);
    assert_enum_parity("MarginTenor", values, CANONICAL_MARGIN_TENORS);
}

#[test]
fn described_margin_enums_use_one_of_const_style() {
    let schema = margin_schema();
    for schema_name in ["ImMethodology", "MarginTenor", "MarginCallType"] {
        let enum_schema = schema
            .pointer(&format!("/definitions/{schema_name}"))
            .unwrap_or_else(|| panic!("{schema_name} should exist in schema"));
        assert_described_one_of_enum(schema_name, enum_schema);
    }
}
