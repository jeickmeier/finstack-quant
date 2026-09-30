//! Generated statements-analytics schemas describe exactly what serde writes.
//!
//! Covers the contracts with a custom wire shape: non-finite `f64` sentinels
//! (forecast metrics, explanation values) and the validating `try_from` PD
//! curve. Each value is serialized, validated against the schema the registry
//! publishes for its type (a root artifact or a `$defs` entry of one) and read
//! back.

use finstack_quant_statements_analytics::analysis::{
    backtest_forecast, Explanation, RatingPdMap, RawPdCurve,
};
use finstack_quant_statements_analytics::schema::ARTIFACTS;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

/// Schema for `type_name`: its root artifact, or a `$defs` entry of a
/// registered artifact wrapped so its local references still resolve.
fn schema_for(type_name: &str) -> Value {
    let mut nested = None;
    for artifact in ARTIFACTS {
        let schema = artifact.generate().expect("schema renders");
        if artifact.type_name() == type_name {
            return schema;
        }
        if nested.is_none() && schema["$defs"].get(type_name).is_some() {
            nested = Some(json!({
                "$id": schema["$id"].clone(),
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "$ref": format!("#/$defs/{type_name}"),
                "$defs": schema["$defs"].clone(),
            }));
        }
    }
    nested.unwrap_or_else(|| panic!("{type_name} is not published by the registry"))
}

fn assert_wire_matches_schema<T: Serialize + DeserializeOwned>(type_name: &str, value: &T) {
    let wire = serde_json::to_value(value).expect("serialize");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema_for(type_name))
        .expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&wire)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{type_name}: {errors:?}\n{wire}");
    let back: T = serde_json::from_value(wire.clone()).expect("deserialize");
    assert_eq!(
        serde_json::to_value(&back).expect("reserialize"),
        wire,
        "{type_name} round trip"
    );
}

#[test]
fn forecast_metrics_with_undefined_mape_validate() {
    // A zero actual leaves MAPE without an effective observation.
    let metrics = backtest_forecast(&[0.0, 0.0], &[1.0, 2.0]).expect("metrics");
    let wire = serde_json::to_value(&metrics).expect("serialize");
    assert!(
        wire["mape"].is_string(),
        "undefined MAPE is a sentinel: {wire}"
    );
    assert_wire_matches_schema("ForecastMetrics", &metrics);
}

#[test]
fn explanation_with_non_finite_values_validates() {
    let explanation: Explanation = serde_json::from_value(json!({
        "node_id": "margin",
        "period_id": "2025Q1",
        "final_value": "nan",
        "node_type": "calculated",
        "formula_text": "revenue / cost",
        "breakdown": [
            { "component": "revenue", "value": 100.0 },
            { "component": "cost", "value": "inf", "operation": "/" }
        ]
    }))
    .expect("explanation deserializes");
    assert_wire_matches_schema("Explanation", &explanation);
}

#[test]
fn rating_pd_map_validates_through_its_raw_curve_wire() {
    let mut curves = IndexMap::new();
    curves.insert(
        "BB".to_string(),
        RawPdCurve::new("BB", vec![(0.0, 0.0), (1.0, 0.02), (5.0, 0.09)]).expect("curve"),
    );
    assert_wire_matches_schema("RatingPdMap", &RatingPdMap::new(curves));
}
