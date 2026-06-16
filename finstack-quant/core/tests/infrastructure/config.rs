use finstack_quant_core::config::{
    rounding_context_from, FinstackConfig, RoundingMode, ToleranceConfig, ZeroKind,
};
use finstack_quant_core::currency::Currency;
use serde_json::json;

#[test]
fn config_extensions_roundtrip() {
    let mut cfg = FinstackConfig::default();
    cfg.rounding.mode = RoundingMode::AwayFromZero;
    cfg.extensions
        .insert("custom.section.v1", json!({ "alpha": 1, "beta": true }));

    let encoded = serde_json::to_string(&cfg).expect("serialize");
    let decoded: FinstackConfig = serde_json::from_str(&encoded).expect("deserialize");

    assert_eq!(decoded.rounding.mode, RoundingMode::AwayFromZero);
    let section = decoded
        .extensions
        .get("custom.section.v1")
        .expect("section exists");
    assert_eq!(section["alpha"], 1);
    assert_eq!(section["beta"], true);
}

#[test]
fn config_extensions_serde_roundtrip() {
    let json = r#"{
        "rounding": {
            "mode": "Bankers",
            "ingest_scale": { "overrides": {} },
            "output_scale": { "overrides": {} }
        }
    }"#;

    let cfg: FinstackConfig = serde_json::from_str(json).expect("deserialize");
    assert_eq!(cfg.rounding.mode, RoundingMode::Bankers);
    assert!(cfg.extensions.is_empty());
    // Tolerances should use defaults
    assert_eq!(cfg.tolerances.rate_epsilon, 1e-12);
    assert_eq!(cfg.tolerances.generic_epsilon, 1e-10);
}

#[test]
fn tolerance_config_defaults() {
    let cfg = FinstackConfig::default();

    assert_eq!(cfg.tolerances.rate_epsilon, 1e-12);
    assert_eq!(cfg.tolerances.generic_epsilon, 1e-10);
}

#[test]
fn tolerance_config_custom_values() {
    let mut cfg = FinstackConfig::default();
    cfg.tolerances.rate_epsilon = 1e-14;
    cfg.tolerances.generic_epsilon = 1e-8;

    assert_eq!(cfg.tolerances.rate_epsilon, 1e-14);
    assert_eq!(cfg.tolerances.generic_epsilon, 1e-8);
}

#[test]
fn tolerance_config_roundtrip_serialization() {
    let original = ToleranceConfig {
        rate_epsilon: 1e-14,
        generic_epsilon: 1e-8,
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let deserialized: ToleranceConfig = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.rate_epsilon, original.rate_epsilon);
    assert_eq!(deserialized.generic_epsilon, original.generic_epsilon);
}

#[test]
fn finstack_config_with_tolerances_roundtrip() {
    let mut cfg = FinstackConfig::default();
    cfg.tolerances.rate_epsilon = 1e-15;
    cfg.tolerances.generic_epsilon = 1e-9;

    let json = serde_json::to_string(&cfg).expect("serialize");
    let decoded: FinstackConfig = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(decoded.tolerances.rate_epsilon, 1e-15);
    assert_eq!(decoded.tolerances.generic_epsilon, 1e-9);
}

#[test]
fn rounding_context_uses_configured_tolerances() {
    let mut cfg = FinstackConfig::default();
    cfg.tolerances.rate_epsilon = 1e-10;
    cfg.tolerances.generic_epsilon = 1e-8;

    let ctx = rounding_context_from(&cfg);

    // Test rate zero check with custom tolerance
    assert!(ctx.is_effectively_zero(5e-11, ZeroKind::Rate)); // Below 1e-10
    assert!(!ctx.is_effectively_zero(5e-9, ZeroKind::Rate)); // Above 1e-10

    // Test generic zero check with custom tolerance
    assert!(ctx.is_effectively_zero(5e-9, ZeroKind::Generic)); // Below 1e-8
    assert!(!ctx.is_effectively_zero(5e-7, ZeroKind::Generic)); // Above 1e-8

    // Money epsilon is unaffected (derived from currency scale)
    assert!(ctx.is_effectively_zero(0.004, ZeroKind::Money(Currency::USD))); // Below 0.005
    assert!(!ctx.is_effectively_zero(0.006, ZeroKind::Money(Currency::USD))); // Above 0.005
}

#[test]
fn tolerance_config_partial_deserialize_uses_defaults() {
    // Only specify one field, the other should use default
    let json = r#"{ "rate_epsilon": 1e-14 }"#;
    let tol: ToleranceConfig = serde_json::from_str(json).expect("deserialize");

    assert_eq!(tol.rate_epsilon, 1e-14);
    assert_eq!(tol.generic_epsilon, 1e-10); // default
}
