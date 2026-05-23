//! Schema parity tests for checked-in margin JSON schemas.

use serde_json::Value;

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
fn margin_im_methodology_schema_parity() {
    let schema_json = include_str!("../schemas/margin/1/margin.schema.json");
    let schema: Value = serde_json::from_str(schema_json).expect("Schema JSON should be valid");

    let im = schema
        .pointer("/$defs/ImMethodology")
        .or_else(|| schema.pointer("/definitions/ImMethodology"))
        .expect("ImMethodology should exist in schema");

    let values = extract_enum_values(im);
    assert_enum_parity("ImMethodology", values, CANONICAL_IM_METHODOLOGIES);
}

#[test]
fn margin_tenor_schema_parity() {
    let schema_json = include_str!("../schemas/margin/1/margin.schema.json");
    let schema: Value = serde_json::from_str(schema_json).expect("Schema JSON should be valid");

    let mt = schema
        .pointer("/$defs/MarginTenor")
        .or_else(|| schema.pointer("/definitions/MarginTenor"))
        .expect("MarginTenor should exist in schema");

    let values = extract_enum_values(mt);
    assert_enum_parity("MarginTenor", values, CANONICAL_MARGIN_TENORS);
}
