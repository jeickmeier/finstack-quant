//! JSON Schema contract tests for factor-model configuration envelopes.

use finstack_quant_models::factor::schema::{
    credit_calibration_config_schema, credit_calibration_inputs_schema, credit_factor_model_schema,
    factor_model_config_schema, ARTIFACTS,
};
use finstack_quant_models::factor::FactorModelConfigEnvelope;
use serde_json::{json, Value};

/// Render through the registry: only that path applies the packager, the
/// single-branch-union collapse and examples, which is what is checked in.
const FACTOR_MODEL_SCHEMA_FILENAME: &str = "factor_model_config.schema.json";
const CREDIT_FACTOR_MODEL_SCHEMA_FILENAME: &str = "credit_factor_model.schema.json";
const CREDIT_CALIBRATION_CONFIG_SCHEMA_FILENAME: &str = "credit_calibration_config.schema.json";
const CREDIT_CALIBRATION_INPUTS_SCHEMA_FILENAME: &str = "credit_calibration_inputs.schema.json";

fn registry_artifact(filename: &str) -> &'static finstack_quant_core::schema::SchemaArtifact {
    ARTIFACTS
        .iter()
        .find(|artifact| artifact.relative_path.ends_with(filename))
        .unwrap_or_else(|| panic!("no registry entry for {filename}"))
}

fn registry_schema(filename: &str) -> Value {
    registry_artifact(filename)
        .generate()
        .expect("registry artifact renders")
}

fn generated_schema() -> Value {
    registry_schema(FACTOR_MODEL_SCHEMA_FILENAME)
}

fn checked_in_schema() -> Value {
    serde_json::from_str(include_str!(
        "../schemas/factor_model/1/factor_model_config.schema.json"
    ))
    .expect("checked-in factor-model schema parses")
}

fn validate(schema: &Value, fixture: &Value) -> Result<(), Vec<String>> {
    let validator = jsonschema::validator_for(schema).expect("factor-model schema compiles");
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

fn credit_calibration_config_fixture() -> Value {
    json!({
        "policy": "globally_off",
        "hierarchy": {"levels": ["rating"]},
        "min_bucket_size_per_level": {"per_level": [5]},
        "vol_model": "sample",
        "covariance_strategy": "diagonal",
        "beta_shrinkage": "none",
        "use_returns_or_levels": "returns",
        "panel_frequency": "monthly",
        "bucket_weighting": "equal"
    })
}

#[test]
fn checked_in_envelope_schema_validates_canonical_fixture() {
    let fixture: Value =
        serde_json::from_str(include_str!("data/canonical/factor_model_config.json"))
            .expect("canonical factor-model fixture parses");

    validate(&checked_in_schema(), &fixture)
        .unwrap_or_else(|errors| panic!("canonical factor-model failed validation: {errors:#?}"));
}

#[test]
fn checked_in_envelope_schema_rejects_malformed_nested_covariance() {
    let mut fixture: Value =
        serde_json::from_str(include_str!("data/canonical/factor_model_config.json"))
            .expect("canonical factor-model fixture parses");
    fixture["config"]["covariance"]["data"] = json!("not-a-matrix");

    assert!(
        validate(&checked_in_schema(), &fixture).is_err(),
        "typed factor-model schema must reject malformed nested covariance data"
    );
}

#[test]
fn checked_in_envelope_schema_rejects_invalid_bump_units() {
    let mut fixture: Value =
        serde_json::from_str(include_str!("data/canonical/factor_model_config.json"))
            .expect("canonical factor-model fixture parses");
    fixture["config"]["factors"] = json!([{
        "id": "rates:usd",
        "factor_type": "rates",
        "market_mapping": {
            "curve_parallel": {
                "curve_ids": ["USD-OIS"],
                "units": "not_a_bump_unit"
            }
        }
    }]);

    assert!(
        validate(&checked_in_schema(), &fixture).is_err(),
        "typed factor-model schema must reject unknown bump units"
    );
}

#[test]
fn checked_in_envelope_schema_accepts_serde_bump_units_value() {
    let mut fixture: Value =
        serde_json::from_str(include_str!("data/canonical/factor_model_config.json"))
            .expect("canonical factor-model fixture parses");
    fixture["config"]["factors"] = json!([{
        "id": "rates:usd",
        "factor_type": "rates",
        "market_mapping": {
            "curve_parallel": {
                "curve_ids": ["USD-OIS"],
                "units": "rate_bp"
            }
        }
    }]);

    validate(&checked_in_schema(), &fixture)
        .unwrap_or_else(|errors| panic!("serde bump units failed validation: {errors:#?}"));
}

#[test]
fn checked_in_envelope_schema_rejects_wrong_contract_marker() {
    let mut fixture: Value =
        serde_json::from_str(include_str!("data/canonical/factor_model_config.json"))
            .expect("canonical factor-model fixture parses");
    fixture["schema"] = json!("finstack_quant.factor_model_config/999");

    assert!(
        validate(&checked_in_schema(), &fixture).is_err(),
        "factor-model schema must require the current contract marker"
    );
}

#[test]
fn checked_in_envelope_schema_rejects_unknown_covariance_field() {
    let mut fixture: Value =
        serde_json::from_str(include_str!("data/canonical/factor_model_config.json"))
            .expect("canonical factor-model fixture parses");
    fixture["config"]["covariance"]["unexpected"] = json!(true);

    assert!(
        validate(&checked_in_schema(), &fixture).is_err(),
        "covariance schema must reject fields denied by custom deserialization"
    );
}

#[test]
fn checked_in_schema_matches_generated_type_and_metadata() {
    let schema = checked_in_schema();

    assert_eq!(schema, generated_schema());
    assert_eq!(
        schema["$id"],
        registry_artifact(FACTOR_MODEL_SCHEMA_FILENAME).id
    );
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn credit_factor_model_schema_uses_explicit_date_wire_schema() {
    let credit_model =
        credit_factor_model_schema().expect("embedded credit factor model schema parses");

    assert_eq!(
        credit_model.pointer("/$defs/DateWire/format"),
        Some(&json!("date"))
    );
    assert_eq!(
        credit_model.pointer("/properties/as_of/$ref"),
        Some(&json!("#/$defs/DateWire"))
    );
}

#[test]
fn checked_in_schema_keeps_meaningful_nested_validation() {
    let schema = checked_in_schema();

    assert!(schema.pointer("/$defs/FactorModelConfig").is_some());
    assert!(schema.pointer("/$defs/FactorCovarianceMatrix").is_some());
    assert_eq!(
        schema.pointer("/$defs/FactorCovarianceMatrix/additionalProperties"),
        Some(&json!(false))
    );
    assert_eq!(
        schema.pointer("/$defs/FactorCovarianceMatrix/required"),
        Some(&json!(["factor_ids", "n", "data"]))
    );
    let bump_units: Vec<_> = schema
        .pointer("/$defs/BumpUnits/oneOf")
        .and_then(Value::as_array)
        .expect("bump units must be a finite union")
        .iter()
        .filter_map(|variant| variant.get("const").and_then(Value::as_str))
        .collect();
    assert_eq!(bump_units, ["rate_bp", "percent", "fraction", "factor"]);
    assert!(schema
        .pointer("/$defs/MatchingConfig/oneOf")
        .and_then(Value::as_array)
        .is_some_and(|variants| variants.len() >= 4));
    assert_eq!(
        schema.pointer("/properties/config/$ref"),
        Some(&json!("#/$defs/FactorModelConfig"))
    );
}

#[test]
fn credit_factor_model_schema_rejects_malformed_nested_config() {
    let schema = credit_factor_model_schema().expect("embedded credit factor model schema parses");
    let mut fixture: Value =
        serde_json::from_str(include_str!("data/canonical/credit_factor_model.json"))
            .expect("canonical credit factor model fixture parses");
    fixture["config"]["covariance"]["data"] = json!("not-a-matrix");

    assert!(
        validate(schema, &fixture).is_err(),
        "credit factor model schema must reject malformed nested factor-model config"
    );
}

#[test]
fn credit_factor_model_schema_matches_generated_type_and_metadata() {
    let schema = credit_factor_model_schema().expect("embedded credit factor model schema");

    assert_eq!(
        schema,
        &registry_schema(CREDIT_FACTOR_MODEL_SCHEMA_FILENAME)
    );
    assert_eq!(
        schema["$id"],
        registry_artifact(CREDIT_FACTOR_MODEL_SCHEMA_FILENAME).id
    );
    // One-variant marker enums lose schemars' single-branch `oneOf` wrapper in
    // the emitter, so the `const` sits directly on the definition.
    assert_eq!(
        schema["$defs"]["CreditFactorModelSchema"]["const"],
        "finstack_quant.credit_factor_model/1"
    );
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["$defs"]["DateRange"]["additionalProperties"], false);
    assert!(
        schema["$defs"]["CreditCalibrationDiagnostics"]
            .get("additionalProperties")
            .is_none(),
        "open diagnostics type must match serde and accept extension fields"
    );
}

#[test]
fn credit_factor_model_schema_validates_canonical_fixture() {
    let fixture: Value =
        serde_json::from_str(include_str!("data/canonical/credit_factor_model.json"))
            .expect("canonical credit factor model fixture parses");

    validate(
        credit_factor_model_schema().expect("embedded credit factor model schema"),
        &fixture,
    )
    .unwrap_or_else(|errors| panic!("canonical credit factor model failed: {errors:#?}"));
}

#[test]
fn credit_factor_model_schema_matches_nested_serde_strictness() {
    let schema = credit_factor_model_schema().expect("embedded credit factor model schema");
    let fixture: Value =
        serde_json::from_str(include_str!("data/canonical/credit_factor_model.json"))
            .expect("canonical credit factor model fixture parses");

    let mut wrong_version = fixture.clone();
    wrong_version["schema"] = json!("finstack_quant.credit_factor_model/999");
    assert!(validate(schema, &wrong_version).is_err());

    let mut closed_nested = fixture.clone();
    closed_nested["calibration_window"]["unexpected"] = json!(true);
    assert!(validate(schema, &closed_nested).is_err());

    let mut open_diagnostics = fixture;
    open_diagnostics["diagnostics"]["future_metric"] = json!({"value": 1});
    validate(schema, &open_diagnostics)
        .unwrap_or_else(|errors| panic!("open diagnostics extension failed: {errors:#?}"));
}

#[test]
fn credit_calibration_schemas_match_generated_types_and_embedded_apis() {
    let config =
        credit_calibration_config_schema().expect("embedded credit calibration config schema");
    let inputs =
        credit_calibration_inputs_schema().expect("embedded credit calibration inputs schema");

    assert_eq!(
        config,
        &registry_schema(CREDIT_CALIBRATION_CONFIG_SCHEMA_FILENAME)
    );
    assert_eq!(
        inputs,
        &registry_schema(CREDIT_CALIBRATION_INPUTS_SCHEMA_FILENAME)
    );
    assert_eq!(
        config["$id"],
        registry_artifact(CREDIT_CALIBRATION_CONFIG_SCHEMA_FILENAME).id
    );
    assert_eq!(
        inputs["$id"],
        registry_artifact(CREDIT_CALIBRATION_INPUTS_SCHEMA_FILENAME).id
    );
    assert_eq!(config["additionalProperties"], false);
    assert_eq!(inputs["additionalProperties"], false);
    assert_eq!(
        factor_model_config_schema().expect("embedded factor-model config schema"),
        &checked_in_schema()
    );
}

#[test]
fn credit_calibration_schemas_reject_malformed_nested_fixtures() {
    let mut config = credit_calibration_config_fixture();
    validate(
        credit_calibration_config_schema().expect("embedded credit calibration config schema"),
        &config,
    )
    .unwrap_or_else(|errors| panic!("valid calibration config failed: {errors:#?}"));
    config["hierarchy"]["unexpected"] = json!(true);
    assert!(
        validate(
            credit_calibration_config_schema().expect("embedded credit calibration config schema"),
            &config,
        )
        .is_err(),
        "closed nested calibration config must reject unknown fields"
    );

    let mut inputs = json!({
        "history_panel": {
            "dates": ["2024-01-31", "2024-02-29"],
            "spreads": {"ISSUER-A": [100.0, null]}
        },
        "issuer_tags": {"tags": {"ISSUER-A": {"rating": "IG"}}},
        "generic_factor": {
            "spec": {"name": "CDX IG", "series_id": "cdx.ig.5y"},
            "values": [99.0, 100.0]
        },
        "as_of": "2024-02-29",
        "as_of_spreads": {"ISSUER-A": 100.0},
        "idiosyncratic_overrides": {}
    });
    validate(
        credit_calibration_inputs_schema().expect("embedded credit calibration inputs schema"),
        &inputs,
    )
    .unwrap_or_else(|errors| panic!("valid calibration inputs failed: {errors:#?}"));
    inputs["history_panel"]["spreads"]["ISSUER-A"] = json!("not-a-series");
    assert!(
        validate(
            credit_calibration_inputs_schema().expect("embedded credit calibration inputs schema"),
            &inputs,
        )
        .is_err(),
        "malformed nested spread series must be rejected"
    );
}

#[test]
fn credit_calibration_schema_rejects_runtime_invalid_numeric_bounds() {
    let schema =
        credit_calibration_config_schema().expect("embedded credit calibration config schema");
    let cases = [
        (
            "ewma lambda zero",
            "/vol_model",
            json!({"ewma": {"lambda": 0.0}}),
        ),
        (
            "ewma lambda one",
            "/vol_model",
            json!({"ewma": {"lambda": 1.0}}),
        ),
        (
            "ridge alpha negative",
            "/covariance_strategy",
            json!({"ridge": {"alpha": -0.01}}),
        ),
        (
            "shrinkage below zero",
            "/beta_shrinkage",
            json!({"toward_one": {"alpha": -0.01}}),
        ),
        (
            "shrinkage above one",
            "/beta_shrinkage",
            json!({"toward_one": {"alpha": 1.01}}),
        ),
        (
            "unknown panel frequency",
            "/panel_frequency",
            json!("weekly"),
        ),
        (
            "unknown bucket weighting",
            "/bucket_weighting",
            json!("market_cap"),
        ),
    ];

    for (name, pointer, invalid) in cases {
        let mut fixture = credit_calibration_config_fixture();
        *fixture
            .pointer_mut(pointer)
            .expect("fixture pointer exists") = invalid;
        assert!(
            validate(schema, &fixture).is_err(),
            "{name} must be rejected by the published schema"
        );
    }
}

#[test]
fn factor_model_schema_rejects_runtime_invalid_risk_confidence() {
    let schema = factor_model_config_schema().expect("embedded factor-model config schema");
    let base: Value = serde_json::from_str(include_str!("data/canonical/factor_model_config.json"))
        .expect("canonical factor-model fixture parses");

    for risk_measure in [
        json!({"var": {"confidence": 0.5}}),
        json!({"var": {"confidence": 1.0}}),
        json!({"expected_shortfall": {"confidence": 0.5}}),
        json!({"expected_shortfall": {"confidence": 1.0}}),
    ] {
        let mut fixture = base.clone();
        fixture["config"]["risk_measure"] = risk_measure;
        assert!(
            validate(schema, &fixture).is_err(),
            "runtime-invalid confidence must be rejected"
        );
    }

    let mut valid = base;
    valid["config"]["risk_measure"] = json!({"var": {"confidence": 0.500_001}});
    validate(schema, &valid)
        .unwrap_or_else(|errors| panic!("valid open-bound confidence failed: {errors:#?}"));
}

#[test]
fn factor_model_schema_rejects_noncanonical_unmatched_policy_values() {
    let schema = factor_model_config_schema().expect("embedded factor-model config schema");
    let base: Value = serde_json::from_str(include_str!("data/canonical/factor_model_config.json"))
        .expect("canonical factor-model fixture parses");

    for alias in ["Strict", "Residual", "Warn", "WARN"] {
        let mut fixture = base.clone();
        fixture["config"]["unmatched_policy"] = json!(alias);
        assert!(
            validate(schema, &fixture).is_err(),
            "noncanonical value {alias} must be rejected"
        );
        assert!(
            serde_json::from_value::<FactorModelConfigEnvelope>(fixture).is_err(),
            "serde must reject noncanonical value {alias}"
        );
    }
}
