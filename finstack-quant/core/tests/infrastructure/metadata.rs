//! Integration tests for ResultsMeta stamping.

use finstack_quant_core::config::{results_meta, FinstackConfig, ResultsMeta, NUMERIC_MODE_F64};

#[test]
fn test_results_meta_default_stamping() {
    let cfg = FinstackConfig::default();
    let meta = results_meta(&cfg);

    // Should have numeric mode
    assert_eq!(meta.numeric_mode, NUMERIC_MODE_F64);

    // Deterministic by default (timestamp is an opt-in at IO boundaries)
    assert!(meta.timestamp.is_none());

    // Should have version
    assert!(meta.version.is_some());
    let version = meta.version.unwrap();
    assert!(!version.is_empty());
}

#[test]
fn test_results_meta_serialization() {
    let meta = results_meta(&FinstackConfig::default());
    let json = serde_json::to_string(&meta).expect("Failed to serialize");

    // Should contain essential fields
    assert!(json.contains("numeric_mode"));
    assert!(json.contains("rounding"));

    // Roundtrip
    let deserialized: ResultsMeta = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized.numeric_mode, NUMERIC_MODE_F64);
}

#[test]
fn test_results_meta_deserializes_without_optional_fields() {
    // Canonical JSON without optional fields should deserialize successfully.
    let json = r#"{
        "numeric_mode": "f64",
        "rounding": {
            "mode": "bankers",
            "ingest_scale_by_currency": {},
            "output_scale_by_currency": {},
            "version": 1
        }
    }"#;

    let meta: ResultsMeta = serde_json::from_str(json).expect("Failed to deserialize JSON");
    assert_eq!(meta.numeric_mode, NUMERIC_MODE_F64);
    // New fields should be None or default
    assert!(meta.fx_policy_applied.is_none());
}

/// `numeric_mode` is a deliberately unvalidated wire string (result envelopes
/// stay deserializable across host-language versions); producers always stamp
/// [`NUMERIC_MODE_F64`]. Pin both halves of that contract.
#[test]
fn test_results_meta_numeric_mode_is_plain_wire_string() {
    // Deserialization accepts any string value without validation...
    let json = r#"{
        "numeric_mode": "decimal",
        "rounding": {
            "mode": "bankers",
            "ingest_scale_by_currency": {},
            "output_scale_by_currency": {},
            "version": 1
        }
    }"#;
    let meta: ResultsMeta =
        serde_json::from_str(json).expect("numeric_mode is an unvalidated wire string");
    assert_eq!(meta.numeric_mode, "decimal");

    // ...while the producer side keeps stamping the canonical constant.
    let stamped = results_meta(&FinstackConfig::default());
    assert_eq!(stamped.numeric_mode, NUMERIC_MODE_F64);

    // And the derived schema carries no const constraint on the property.
    let schema = serde_json::to_value(schemars::schema_for!(ResultsMeta)).expect("schema");
    assert_eq!(schema["properties"]["numeric_mode"]["type"], "string");
    assert!(schema["properties"]["numeric_mode"].get("const").is_none());
}

#[test]
fn test_results_meta_with_fx_policy() {
    let mut meta = results_meta(&FinstackConfig::default());
    meta.fx_policy_applied = Some("SPOT_RATE".to_string());

    let json = serde_json::to_string(&meta).expect("Failed to serialize");
    assert!(json.contains("SPOT_RATE"));

    let deserialized: ResultsMeta = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(
        deserialized.fx_policy_applied,
        Some("SPOT_RATE".to_string())
    );
}

#[cfg(test)]
mod property_tests {
    use super::*;

    #[test]
    fn property_version_matches_cargo_package() {
        let meta = results_meta(&FinstackConfig::default());
        let version = meta.version.expect("Version should be present");

        // Should match the package version from Cargo.toml
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn property_serialization_roundtrip_preserves_data() {
        let original = results_meta(&FinstackConfig::default());
        let json = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: ResultsMeta = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(original.numeric_mode, deserialized.numeric_mode);
        assert_eq!(original.fx_policy_applied, deserialized.fx_policy_applied);
        // Timestamp and version should roundtrip
        assert_eq!(original.timestamp, deserialized.timestamp);
        assert_eq!(original.version, deserialized.version);
    }
}
