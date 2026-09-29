//! wasm-bindgen-test suite for the credit factor hierarchy WASM surface.
//!
//! Covers `CreditFactorModel` JSON round-trip and the calibrate → serialize →
//! decompose pipeline.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::models::factor::{
    JsCreditCalibrator, JsCreditFactorModel, JsFactorCovarianceForecast,
};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

// ---- helpers ----------------------------------------------------------------

fn minimal_config_json() -> String {
    serde_json::json!({
        "policy": "globally_off",
        "hierarchy": { "levels": ["rating", "region"] },
        "min_bucket_size_per_level": { "per_level": [1, 1] },
        "vol_model": "sample",
        "covariance_strategy": "diagonal",
        "beta_shrinkage": "none",
        "use_returns_or_levels": "returns",
        "panel_frequency": "monthly",
        "bucket_weighting": "equal"
    })
    .to_string()
}

/// Build a minimal but valid `CreditCalibrationInputs` JSON.
///
/// 3 issuers × 24 monthly obs (2022-04-01 … 2024-03-01), same layout as the
/// native fixture in `credit_factor_model.rs`.
fn minimal_inputs_json() -> String {
    // 24 regular month-end dates ending 2024-03-31.
    let dates: Vec<String> = {
        let end = time::Date::from_calendar_date(2024, time::Month::March, 31).unwrap();
        let mut d = end;
        let mut v = Vec::with_capacity(24);
        v.push(d.to_string());
        for _ in 1..24 {
            d = d
                .replace_day(1)
                .unwrap()
                .checked_sub(time::Duration::days(1))
                .unwrap();
            v.push(d.to_string());
        }
        v.reverse();
        v
    };

    let n = dates.len();

    let make_series = |base: f64| -> Vec<serde_json::Value> {
        (0..n)
            .map(|i| serde_json::Value::from(base + 0.0005 * (i as f64).sin()))
            .collect()
    };

    let as_of = dates.last().unwrap().clone();

    serde_json::json!({
        "history_panel": {
            "dates": dates,
            "spreads": {
                "ISSUER-A": make_series(0.0150),
                "ISSUER-B": make_series(0.0175),
                "ISSUER-C": make_series(0.0200)
            }
        },
        "issuer_tags": {
            "tags": {
                "ISSUER-A": { "rating": "IG", "region": "EU" },
                "ISSUER-B": { "rating": "IG", "region": "NA" },
                "ISSUER-C": { "rating": "HY", "region": "EU" }
            }
        },
        "generic_factor": {
            "spec": { "name": "CDX IG 5Y", "series_id": "cdx.ig.5y" },
            "values": (0..n).map(|i| 0.0100 + 0.00005 * (i as f64).sin()).collect::<Vec<f64>>()
        },
        "as_of": as_of,
        "as_of_spreads": {
            "ISSUER-A": 0.0150,
            "ISSUER-B": 0.0175,
            "ISSUER-C": 0.0200
        },
        "idiosyncratic_overrides": {}
    })
    .to_string()
}

// ---- tests ------------------------------------------------------------------

/// JSON round-trip: load the golden artifact, re-serialize, verify
/// the namespaced `schema` marker is preserved.
#[wasm_bindgen_test]
fn credit_factor_model_round_trips_through_json() {
    let json =
        include_str!("../../finstack-quant/models/tests/data/canonical/credit_factor_model.json");
    let model = JsCreditFactorModel::from_json(JsValue::from(json))
        .expect("from_json must succeed on golden artifact");
    let out = model.to_json().expect("to_json must succeed");

    let parsed_in: serde_json::Value = serde_json::from_str(json).unwrap();
    let parsed_out: serde_json::Value = serde_json::from_str(&out).unwrap();

    assert_eq!(
        parsed_in["schema"], parsed_out["schema"],
        "schema must be preserved through round-trip"
    );
}

/// Calibrate a minimal model, serialize it, and verify the JSON contains
/// the canonical namespaced `schema` marker.
#[wasm_bindgen_test]
fn calibrate_then_decompose_round_trip() {
    let config_json = minimal_config_json();
    let inputs_json = minimal_inputs_json();

    let calibrator = JsCreditCalibrator::new(JsValue::from(&config_json))
        .expect("JsCreditCalibrator::new must succeed");
    let model = calibrator
        .calibrate(JsValue::from(&inputs_json))
        .expect("calibrate must succeed on minimal inputs");
    let model_json = model.to_json().expect("to_json must succeed");

    let parsed: serde_json::Value = serde_json::from_str(&model_json).unwrap();
    assert_eq!(
        parsed["schema"].as_str().unwrap(),
        "finstack_quant.credit_factor_model/1",
        "schema must match the canonical v1 marker"
    );
    assert_eq!(model.schema(), "finstack_quant.credit_factor_model/1");
}

#[wasm_bindgen_test]
fn covariance_forecast_returns_structured_objects() {
    let calibrator =
        JsCreditCalibrator::new(JsValue::from(&minimal_config_json())).expect("calibrator");
    let model = calibrator
        .calibrate(JsValue::from(&minimal_inputs_json()))
        .expect("model");
    let forecast = JsFactorCovarianceForecast::new(&model);

    let covariance = forecast
        .covariance_at(JsValue::from("one_step"))
        .expect("covariance");
    let covariance: serde_json::Value = serde_wasm_bindgen::from_value(covariance).unwrap();
    assert!(covariance["factor_ids"].is_array());
    assert!(covariance["data"].is_array());

    let config = forecast
        .factor_model_at(JsValue::from("one_step"), JsValue::from("\"variance\""))
        .expect("factor model");
    let config: serde_json::Value = serde_wasm_bindgen::from_value(config).unwrap();
    assert!(config["factors"].is_array());
    assert!(config["covariance"].is_object());
    assert_eq!(config["risk_measure"], "variance");
}
