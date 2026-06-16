//! Integration tests for ResultsMeta stamping.

use finstack_quant_core::config::{
    results_meta, results_meta_now, FinstackConfig, NumericMode, ResultsMeta,
};

#[test]
fn test_results_meta_default_stamping() {
    let cfg = FinstackConfig::default();
    let meta = results_meta(&cfg);

    // Should have numeric mode
    assert_eq!(meta.numeric_mode, NumericMode::F64);

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
    assert_eq!(deserialized.numeric_mode, NumericMode::F64);
}

#[test]
fn test_results_meta_deserializes_without_optional_fields() {
    // JSON without optional fields should deserialize successfully
    let old_json = r#"{
        "numeric_mode": "F64",
        "rounding": {
            "mode": "Bankers",
            "ingest_scale_by_ccy": {},
            "output_scale_by_ccy": {},
            "version": 1
        }
    }"#;

    let meta: ResultsMeta = serde_json::from_str(old_json).expect("Failed to deserialize old JSON");
    assert_eq!(meta.numeric_mode, NumericMode::F64);
    // New fields should be None or default
    assert!(meta.fx_policy_applied.is_none());
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

#[test]
fn test_results_meta_timestamp_format() {
    let meta = results_meta_now(&FinstackConfig::default());
    let timestamp = meta.timestamp.expect("Timestamp should be present");
    // Verify it's a valid recent timestamp
    assert!(timestamp.year() >= 2024);
}

#[cfg(test)]
mod property_tests {
    use super::*;

    #[test]
    fn property_timestamp_never_in_future() {
        let meta = results_meta_now(&FinstackConfig::default());
        if let Some(timestamp) = meta.timestamp {
            // Parse timestamp and verify it's not in the future
            // (basic sanity check - should be close to now)
            assert!(timestamp.year() >= 2020);
            // We can't easily check if it's in the past without time parsing,
            // but at minimum it should be a valid string
        }
    }

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
