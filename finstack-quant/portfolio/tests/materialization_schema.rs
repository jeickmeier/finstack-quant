//! JSON Schema contract tests for portfolio materialization bundles.

use std::fs;
use std::path::{Path, PathBuf};

use finstack_quant_portfolio::materialization::PortfolioMaterializationEnvelope;
use finstack_quant_portfolio::PositionUnit;
use serde_json::{json, Value};

const INSTRUMENT_SCHEMA_URI: &str =
    "https://finstack_quant.dev/schemas/instrument/1/instrument.schema.json";

fn collect_schema_files(directory: &Path, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read schema directory {}: {error}", directory.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| {
                    panic!("read schema entry in {}: {error}", directory.display())
                })
                .path()
        })
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_schema_files(&path, paths);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            paths.push(path);
        }
    }
}

/// Every published schema a portfolio artifact may reference by `$id`.
pub(crate) fn external_schema_resources() -> Vec<(String, jsonschema::Resource)> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let roots = [
        manifest_dir.join("../cashflows/schemas/cashflow/1"),
        manifest_dir.join("../valuations/schemas/common/1"),
        manifest_dir.join("../valuations/schemas/instruments/1"),
        manifest_dir.join("../valuations/schemas/results/1"),
    ];
    let mut paths = Vec::new();
    for root in roots {
        collect_schema_files(&root, &mut paths);
    }
    paths
        .into_iter()
        .map(|path| {
            let raw = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read schema {}: {error}", path.display()));
            let schema: Value = serde_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("parse schema {}: {error}", path.display()));
            let id = schema
                .get("$id")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("schema {} is missing $id", path.display()))
                .to_string();
            let resource = jsonschema::Resource::from_contents(schema)
                .unwrap_or_else(|error| panic!("build resource {}: {error}", path.display()));
            (id, resource)
        })
        .collect()
}

fn checked_in_schema() -> Value {
    serde_json::from_str(include_str!(
        "../schemas/portfolio/1/portfolio_materialization.schema.json"
    ))
    .expect("checked-in portfolio materialization schema parses")
}

fn deposit_envelope() -> Value {
    serde_json::from_str(include_str!(
        "../../valuations/tests/instruments/json_examples/deposit.json"
    ))
    .expect("canonical deposit example parses")
}

fn materialization_fixture(instrument_envelope: Value) -> Value {
    json!({
        "schema": "finstack_quant.portfolio_materialization/1",
        "portfolio": {
            "id": "schema-test",
            "base_currency": "USD",
            "as_of": "2025-01-01",
            "entities": {}
        },
        "instruments": [{
            "artifact_id": "deposit-artifact",
            "envelope": instrument_envelope
        }],
        "positions": []
    })
}

fn position(quantity: f64, unit: Value) -> Value {
    json!({
        "id": "position-1",
        "entity_id": "entity-1",
        "instrument_id": "USD-DEPOSIT-3M",
        "artifact_id": "deposit-artifact",
        "quantity": quantity,
        "unit": unit
    })
}

fn validation_errors(instance: &Value) -> Vec<String> {
    let schema = checked_in_schema();
    let validator = jsonschema::options()
        .with_resources(external_schema_resources().into_iter())
        .build(&schema)
        .expect("portfolio schema and canonical instrument resources compile");
    validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect()
}

#[test]
fn typed_runtime_rejects_schema_invalid_instrument_content() {
    let malformed = materialization_fixture(json!({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "not_a_real_instrument",
            "spec": {}
        }
    }));

    assert!(serde_json::from_value::<PortfolioMaterializationEnvelope>(malformed).is_err());
}

#[test]
fn checked_in_schema_references_and_validates_canonical_instruments() {
    let schema = checked_in_schema();
    assert_eq!(
        schema.pointer("/$defs/InstrumentArtifact/properties/envelope/$ref"),
        Some(&Value::String(INSTRUMENT_SCHEMA_URI.to_string()))
    );

    let valid = materialization_fixture(deposit_envelope());
    assert!(
        validation_errors(&valid).is_empty(),
        "representative deposit envelope must satisfy the materialization schema"
    );

    let mut malformed_type = valid.clone();
    malformed_type["instruments"][0]["envelope"]["instrument"]["type"] =
        json!("not_a_real_instrument");
    assert!(
        !validation_errors(&malformed_type).is_empty(),
        "unknown instrument type must be rejected"
    );

    let mut malformed_spec = valid;
    malformed_spec["instruments"][0]["envelope"]["instrument"]["spec"] = json!({});
    assert!(
        !validation_errors(&malformed_spec).is_empty(),
        "malformed typed instrument spec must be rejected"
    );
}

#[test]
fn percentage_positions_enforce_quantity_bounds_in_serde_and_schema() {
    for quantity in [-100.0, 100.0] {
        let mut valid = materialization_fixture(deposit_envelope());
        valid["positions"] = json!([position(quantity, json!("percentage"))]);
        assert!(
            serde_json::from_value::<PortfolioMaterializationEnvelope>(valid.clone()).is_ok(),
            "percentage boundary {quantity} must deserialize"
        );
        assert!(
            validation_errors(&valid).is_empty(),
            "percentage boundary {quantity} must satisfy the schema"
        );
    }

    for quantity in [-100.01, 100.01] {
        let mut invalid = materialization_fixture(deposit_envelope());
        invalid["positions"] = json!([position(quantity, json!("percentage"))]);
        assert!(
            serde_json::from_value::<PortfolioMaterializationEnvelope>(invalid.clone()).is_err(),
            "out-of-range percentage {quantity} must fail deserialization"
        );
        assert!(
            !validation_errors(&invalid).is_empty(),
            "out-of-range percentage {quantity} must fail schema validation"
        );
    }

    let mut units = materialization_fixture(deposit_envelope());
    units["positions"] = json!([position(1_000_000.0, json!("units"))]);
    assert!(
        serde_json::from_value::<PortfolioMaterializationEnvelope>(units.clone()).is_ok(),
        "non-percentage quantities must not inherit percentage bounds"
    );
    assert!(validation_errors(&units).is_empty());
}

#[test]
fn position_serialization_and_notional_shape_match_the_schema() {
    for unit in [json!({"notional": null}), json!({"notional": "USD"})] {
        let mut valid = materialization_fixture(deposit_envelope());
        valid["positions"] = json!([position(1_000_000.0, unit)]);
        assert!(serde_json::from_value::<PortfolioMaterializationEnvelope>(valid.clone()).is_ok());
        assert!(validation_errors(&valid).is_empty());
    }

    let mut missing_notional = materialization_fixture(deposit_envelope());
    missing_notional["positions"] = json!([position(1_000_000.0, json!({}))]);
    assert!(
        serde_json::from_value::<PortfolioMaterializationEnvelope>(missing_notional.clone())
            .is_err()
    );
    assert!(!validation_errors(&missing_notional).is_empty());

    let mut valid = materialization_fixture(deposit_envelope());
    valid["positions"] = json!([position(50.0, json!("percentage"))]);
    let mut envelope = serde_json::from_value::<PortfolioMaterializationEnvelope>(valid)
        .expect("valid percentage materialization");
    envelope.positions[0].quantity = 100.01;
    envelope.positions[0].unit = PositionUnit::Percentage;
    assert!(serde_json::to_value(envelope).is_err());
}
