//! Contract tests for checked-in attribution JSON schemas.

#![allow(clippy::expect_used)]

use finstack_quant_attribution::{
    AttributionEnvelope, AttributionMethod, AttributionResult, AttributionResultEnvelope,
    PnlAttribution,
};
use finstack_quant_core::config::ResultsMeta;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use time::macros::date;

const JSON_SCHEMA_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";
const ATTRIBUTION_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/attribution/1/";
const CANONICAL_DECIMAL_PATTERN: &str = r"^-?\d+(\.\d+)?([eE][+-]?\d+)?$";

fn schema_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("attribution")
        .join("1")
}

fn read_schema(path: &Path) -> Value {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&content).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()))
}

fn schema_files() -> Vec<PathBuf> {
    let mut files = std::fs::read_dir(schema_root())
        .expect("read attribution schema directory")
        .map(|entry| {
            entry
                .expect("read attribution schema directory entry")
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn validate_fixture(schema: &Value, fixture: &Value) {
    let validator = jsonschema::validator_for(schema).expect("checked-in schema must compile");
    let errors: Vec<_> = validator
        .iter_errors(fixture)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "fixture failed schema validation: {errors:#?}"
    );
}

#[test]
fn checked_in_attribution_schemas_match_derived_types() {
    // Render through the registry, not `generated_schema`: only the registry
    // path applies the packager, the single-branch-union collapse and examples,
    // which is what the checked-in bytes contain.
    let expected_spec = finstack_quant_attribution::schema::ARTIFACTS[0]
        .generate()
        .expect("attribution schema generates");
    let expected_result = finstack_quant_attribution::schema::ARTIFACTS[1]
        .generate()
        .expect("attribution result schema generates");

    assert_eq!(
        read_schema(&schema_root().join("attribution.schema.json")),
        expected_spec
    );
    assert_eq!(
        read_schema(&schema_root().join("attribution_result.schema.json")),
        expected_result
    );
}

#[test]
fn attribution_schemas_use_canonical_id_host() {
    for path in schema_files() {
        let schema = read_schema(&path);
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("schema filename must be UTF-8");
        let Some(id) = schema.get("$id").and_then(Value::as_str) else {
            panic!("{} is missing $id", path.display());
        };
        assert_eq!(
            id,
            format!("{ATTRIBUTION_SCHEMA_BASE}{filename}"),
            "{} has a non-canonical $id",
            path.display(),
        );
    }
}

#[test]
fn attribution_schemas_declare_2020_12_dialect() {
    for path in schema_files() {
        let schema = read_schema(&path);
        assert_eq!(
            schema.get("$schema").and_then(Value::as_str),
            Some(JSON_SCHEMA_2020_12),
            "{} declares the wrong JSON Schema dialect",
            path.display()
        );
    }
}

#[test]
fn attribution_schema_inventory_is_exact() {
    let filenames: Vec<_> = schema_files()
        .into_iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .expect("schema filename must be UTF-8")
                .to_string()
        })
        .collect();
    assert_eq!(
        filenames,
        ["attribution.schema.json", "attribution_result.schema.json"]
    );
}

#[test]
fn attribution_schemas_keep_meaningful_nested_payloads() {
    let spec = read_schema(&schema_root().join("attribution.schema.json"));
    let result = read_schema(&schema_root().join("attribution_result.schema.json"));

    assert!(
        spec.pointer("/$defs/InstrumentJson/oneOf")
            .and_then(Value::as_array)
            .is_some_and(|variants| variants.len() > 10),
        "instrument payload must retain its typed union"
    );
    for field in [
        "curves",
        "surfaces",
        "series",
        "inflation_indices",
        "dividends",
        "credit_indices",
        "fx_delta_vol_surfaces",
        "vol_cubes",
    ] {
        let field_schema = spec
            .pointer(&format!("/$defs/MarketContextState/properties/{field}"))
            .unwrap_or_else(|| panic!("market-state schema is missing {field}"));
        assert_eq!(
            field_schema.get("type").and_then(Value::as_str),
            Some("array"),
            "market-state {field} must be typed as an array"
        );
        assert!(
            field_schema.get("items").is_some_and(Value::is_object),
            "market-state {field} must define an item schema"
        );
    }
    assert!(
        spec.pointer("/$defs/MarketContextState/properties/prices/additionalProperties")
            .is_some(),
        "market prices must define typed map values"
    );
    assert!(
        spec.pointer("/$defs/MarketContextState/properties/fx/anyOf")
            .and_then(Value::as_array)
            .is_some(),
        "optional FX state must define object and null alternatives"
    );
    assert!(
        spec.pointer("/$defs/AttributionSpec/properties/credit_factor_model")
            .is_some_and(Value::is_object),
        "optional credit-factor model must retain its typed schema"
    );
    // One-variant marker enums lose schemars' single-branch `oneOf` wrapper in
    // the emitter, so the `const` sits directly on the definition.
    assert!(
        spec.pointer("/$defs/CreditFactorModelSchema/const")
            .is_some(),
        "credit-factor model schema must retain its exact version marker"
    );
    assert!(
        result
            .pointer("/$defs/PnlAttribution/properties/credit_factor_detail")
            .is_some(),
        "result schema must retain optional credit-factor detail"
    );
    assert!(
        result
            .pointer("/$defs/CreditFactorAttribution/properties/levels")
            .is_some(),
        "credit-factor detail must retain its nested level payload"
    );
}

#[test]
fn attribution_schemas_apply_canonical_decimal_and_date_normalization() {
    let schema = read_schema(&schema_root().join("attribution.schema.json"));

    assert_eq!(
        schema.pointer("/$defs/DecimalWire/pattern"),
        Some(&json!(CANONICAL_DECIMAL_PATTERN))
    );
    assert_eq!(
        schema.pointer("/$defs/DateWire/format"),
        Some(&json!("date"))
    );
}

#[test]
fn attribution_schema_validates_canonical_fixture() {
    let schema = read_schema(&schema_root().join("attribution.schema.json"));
    let fixture: Value = serde_json::from_str(include_str!(
        "json_examples/bond_attribution_parallel.example.json"
    ))
    .expect("canonical attribution fixture parses");

    validate_fixture(&schema, &fixture);
}

#[test]
fn attribution_schema_validates_non_empty_market_data() {
    let schema = read_schema(&schema_root().join("attribution.schema.json"));
    let mut fixture: Value = serde_json::from_str(include_str!(
        "json_examples/bond_attribution_parallel.example.json"
    ))
    .expect("canonical attribution fixture parses");
    let market = &mut fixture["attribution"]["market_t0"];
    market["curves"] = json!([{
        "allow_non_monotonic": false,
        "base": "2025-01-15",
        "day_count": "act_365f",
        "extrapolation": "flat_forward",
        "id": "USD-OIS",
        "interp_style": "log_linear",
        "knot_points": [[0.0, 1.0], [30.0, 0.30]],
        "min_forward_rate": null,
        "min_forward_tenor": 0.000001,
        "rate_calibration": null,
        "type": "discount"
    }]);
    market["fx"] = json!({
        "config": {
            "pivot_currency": "USD",
            "enable_triangulation": true,
            "cache_capacity": 256
        },
        "quotes": [["EUR", "USD", 1.1]],
        "provider_quotes": [],
        "pinned_quotes": []
    });
    market["prices"]["ACME-SPOT"] = json!({"unitless": 101.25});
    market["collateral"]["ACME-CSA"] = json!("USD-OIS");

    validate_fixture(&schema, &fixture);
}

#[test]
fn attribution_schema_and_serde_accept_credit_factor_model_payload() {
    let schema = read_schema(&schema_root().join("attribution.schema.json"));
    let mut fixture: Value = serde_json::from_str(include_str!(
        "json_examples/bond_attribution_parallel.example.json"
    ))
    .expect("canonical attribution fixture parses");
    let credit_factor_model: Value = serde_json::from_str(include_str!(
        "../../../models/tests/data/canonical/credit_factor_model.json"
    ))
    .expect("canonical credit-factor model fixture parses");
    fixture["attribution"]["credit_factor_model"] = credit_factor_model;

    validate_fixture(&schema, &fixture);
    serde_json::from_value::<AttributionEnvelope>(fixture)
        .expect("typed attribution payload with credit model must deserialize");
}

#[test]
fn attribution_schema_rejects_malformed_discount_curve_internals() {
    let schema = read_schema(&schema_root().join("attribution.schema.json"));
    let validator = jsonschema::validator_for(&schema).expect("checked-in schema must compile");
    let mut fixture: Value = serde_json::from_str(include_str!(
        "json_examples/bond_attribution_parallel.example.json"
    ))
    .expect("canonical attribution fixture parses");
    fixture["attribution"]["market_t0"]["curves"] = json!([{
        "allow_non_monotonic": false,
        "base": "2025-01-15",
        "day_count": "act_365f",
        "extrapolation": "flat_forward",
        "id": "USD-OIS",
        "interp_style": "log_linear",
        "knot_points": [[0.0, 1.0], [30.0, 0.30]],
        "min_forward_rate": null,
        "min_forward_tenor": 0.000001,
        "rate_calibration": null,
        "type": "discount"
    }]);
    assert!(
        validator.is_valid(&fixture),
        "representative discount curve must satisfy the generated schema"
    );

    let mut scalar_knots = fixture.clone();
    scalar_knots["attribution"]["market_t0"]["curves"][0]["knot_points"] = json!(0.95);
    assert!(
        !validator.is_valid(&scalar_knots),
        "discount knot_points must reject scalar payloads"
    );

    let curve = fixture["attribution"]["market_t0"]["curves"][0]
        .as_object_mut()
        .expect("discount curve fixture must be an object");
    curve.remove("interp_style");
    curve.remove("extrapolation");
    assert!(
        !validator.is_valid(&fixture),
        "discount curves must require interpolation and extrapolation policies"
    );
}

#[test]
fn attribution_schema_rejects_malformed_market_payloads() {
    let schema = read_schema(&schema_root().join("attribution.schema.json"));
    let validator = jsonschema::validator_for(&schema).expect("checked-in schema must compile");
    let fixture: Value = serde_json::from_str(include_str!(
        "json_examples/bond_attribution_parallel.example.json"
    ))
    .expect("canonical attribution fixture parses");
    let malformed_fields = [
        ("curves", json!([42])),
        ("fx", json!("not-an-fx-matrix")),
        ("surfaces", json!([42])),
        ("prices", json!({"ACME-SPOT": 42})),
        ("series", json!([42])),
        ("inflation_indices", json!([42])),
        ("dividends", json!([42])),
        ("credit_indices", json!([42])),
        ("fx_delta_vol_surfaces", json!([42])),
        ("vol_cubes", json!([42])),
        ("hierarchy", json!(42)),
    ];

    for (field, malformed) in malformed_fields {
        let mut candidate = fixture.clone();
        candidate["attribution"]["market_t0"][field] = malformed;
        assert!(
            !validator.is_valid(&candidate),
            "malformed market_t0.{field} must fail schema validation"
        );
    }
}

#[test]
fn attribution_result_schema_validates_serialized_result_fixture() {
    let schema = read_schema(&schema_root().join("attribution_result.schema.json"));
    let attribution = PnlAttribution::new(
        Money::new(125.0, Currency::USD).expect("valid money fixture"),
        "BOND-RESULT-FIXTURE",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );
    let fixture = serde_json::to_value(AttributionResultEnvelope::new(AttributionResult {
        attribution,
        results_meta: ResultsMeta::default(),
    }))
    .expect("representative result fixture serializes");

    validate_fixture(&schema, &fixture);
}

#[test]
fn attribution_schema_rejects_unknown_contract_version() {
    let schema = read_schema(&schema_root().join("attribution.schema.json"));
    let mut fixture: Value = serde_json::from_str(include_str!(
        "json_examples/bond_attribution_parallel.example.json"
    ))
    .expect("canonical attribution fixture parses");
    fixture["schema"] = Value::String("finstack_quant.attribution/999".to_string());

    let validator = jsonschema::validator_for(&schema).expect("checked-in schema must compile");
    assert!(
        !validator.is_valid(&fixture),
        "unknown attribution schema versions must be rejected"
    );
    assert!(
        serde_json::from_value::<AttributionEnvelope>(fixture).is_err(),
        "raw serde must enforce the same exact attribution schema marker"
    );
}
