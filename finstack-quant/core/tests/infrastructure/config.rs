//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::config::{
    rounding_context_from, CurrencyScalePolicy, FinstackConfig, RoundingContext, RoundingMode,
    ToleranceConfig, ZeroKind,
};
use finstack_quant_core::currency::Currency;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn currency_scale_rejects_invalid_construction_and_preserves_policy_on_failed_update() {
    let mut policy = CurrencyScalePolicy::new(BTreeMap::from([(Currency::USD, 4)]))
        .expect("supported decimal scale");
    for scale in [29, 1 << 31, u32::MAX] {
        assert!(CurrencyScalePolicy::new(BTreeMap::from([(Currency::USD, scale)])).is_err());
        assert!(policy.set_scale(Currency::USD, scale).is_err());
        assert!(policy.set_scale(Currency::EUR, scale).is_err());
        assert_eq!(
            policy.get_overrides(),
            &BTreeMap::from([(Currency::USD, 4)])
        );
    }
}

#[test]
fn currency_scale_rejects_invalid_config_and_context_json() {
    for scale in [29, 1 << 31, u32::MAX] {
        for field in ["ingest_scale", "output_scale"] {
            let mut value = serde_json::to_value(FinstackConfig::default()).expect("config JSON");
            value["rounding"][field]["overrides"]["USD"] = json!(scale);
            let error = serde_json::from_value::<FinstackConfig>(value)
                .expect_err("unsupported decimal scale");
            assert!(error.to_string().contains("0..=28"));
        }
        for field in ["ingest_scale_by_currency", "output_scale_by_currency"] {
            let mut value = serde_json::to_value(RoundingContext::default()).expect("context JSON");
            value[field]["USD"] = json!(scale);
            let error = serde_json::from_value::<RoundingContext>(value)
                .expect_err("unsupported snapshot decimal scale");
            assert!(error.to_string().contains("0..=28"));
        }
    }
}

#[test]
fn currency_scale_boundaries_roundtrip_and_keep_money_epsilon_finite_positive() {
    let mut cfg = FinstackConfig::default();
    for scale in [0, 28] {
        cfg.rounding
            .ingest_scale
            .set_scale(Currency::USD, scale)
            .unwrap();
        cfg.rounding
            .output_scale
            .set_scale(Currency::USD, scale)
            .unwrap();
        let encoded = serde_json::to_value(&cfg).expect("config JSON");
        assert_eq!(
            encoded["rounding"]["output_scale"]["overrides"]["USD"],
            scale
        );
        let decoded: FinstackConfig = serde_json::from_value(encoded).expect("valid config");
        assert_eq!(decoded.ingest_scale(Currency::USD), scale);
        let context = rounding_context_from(&decoded);
        let restored: RoundingContext =
            serde_json::from_value(serde_json::to_value(&context).expect("context JSON"))
                .expect("valid context");
        assert_eq!(restored, context);
        let epsilon = restored.money_epsilon(Currency::USD);
        assert!(epsilon.is_finite() && epsilon > 0.0);
        assert!(!restored.is_effectively_zero_money(1.0, Currency::USD));
        assert!(restored.is_effectively_zero_money(epsilon, Currency::USD));
        assert!(!restored.is_effectively_zero_money(2.0 * epsilon, Currency::USD));
    }
}

#[cfg(feature = "json-schema")]
#[test]
fn currency_scale_schema_bounds_policy_and_snapshot_values() {
    let policy = serde_json::to_value(schemars::schema_for!(CurrencyScalePolicy)).unwrap();
    let context = serde_json::to_value(schemars::schema_for!(RoundingContext)).unwrap();
    for map in [
        &policy["properties"]["overrides"],
        &context["properties"]["ingest_scale_by_currency"],
        &context["properties"]["output_scale_by_currency"],
    ] {
        assert_eq!(map["additionalProperties"]["minimum"], 0);
        assert_eq!(map["additionalProperties"]["maximum"], 28);
    }
}

#[test]
fn config_extensions_roundtrip() {
    let mut cfg = FinstackConfig::default();
    cfg.rounding.mode = RoundingMode::AwayFromZero;
    cfg.extensions
        .insert("custom.section.v1", json!({ "alpha": 1, "beta": true }))
        .expect("valid extension key");

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
            "mode": "bankers",
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

/// A misspelled top-level config key must not silently select defaults.
#[test]
fn finstack_config_rejects_unknown_top_level_field() {
    let json = r#"{
        "rounding": {
            "mode": "bankers",
            "ingest_scale": {"overrides": {}},
            "output_scale": {"overrides": {}}
        },
        "roundingmode": "floor"
    }"#;
    let error = serde_json::from_str::<FinstackConfig>(json)
        .expect_err("a misspelled top-level key must be rejected");
    assert!(
        error.to_string().contains("roundingmode"),
        "error must name the offending key: {error}"
    );
}

#[test]
fn rounding_policy_rejects_unknown_field() {
    let json = r#"{
        "rounding": {
            "mode": "bankers",
            "ingest_scale": {"overrides": {}},
            "output_scale": {"overrides": {}},
            "output_scaal": {"overrides": {}}
        }
    }"#;
    let error = serde_json::from_str::<FinstackConfig>(json)
        .expect_err("a misspelled rounding-policy key must be rejected");
    assert!(error.to_string().contains("output_scaal"));
}

#[test]
fn tolerance_config_rejects_unknown_field() {
    let json = r#"{
        "rounding": {
            "mode": "bankers",
            "ingest_scale": {"overrides": {}},
            "output_scale": {"overrides": {}}
        },
        "tolerances": {"rate_epsilon": 1e-12, "rate_epsilonn": 1e-9}
    }"#;
    let error = serde_json::from_str::<FinstackConfig>(json)
        .expect_err("a misspelled tolerance key must be rejected");
    assert!(error.to_string().contains("rate_epsilonn"));
}

#[test]
fn finstack_config_still_accepts_valid_minimal_config() {
    let json = r#"{
        "rounding": {
            "mode": "bankers",
            "ingest_scale": {"overrides": {}},
            "output_scale": {"overrides": {}}
        }
    }"#;
    let config: serde_json::Result<FinstackConfig> = serde_json::from_str(json);
    assert!(
        config.is_ok(),
        "minimal valid config must deserialize: {config:?}"
    );
}

#[test]
fn finstack_config_still_accepts_valid_extensions() {
    let json = r#"{
        "rounding": {
            "mode": "bankers",
            "ingest_scale": {"overrides": {}},
            "output_scale": {"overrides": {}}
        },
        "extensions": {"calibration.config.v1": {"anything": 1}}
    }"#;
    let config: serde_json::Result<FinstackConfig> = serde_json::from_str(json);
    assert!(
        config.is_ok(),
        "valid extensions must deserialize: {config:?}"
    );
}
