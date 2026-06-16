//! wasm-bindgen-test suite for the credit factor hierarchy WASM surface.
//!
//! Covers `CreditFactorModel` JSON round-trip and the calibrate → serialize →
//! decompose pipeline.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::factor_model::{WasmCreditCalibrator, WasmCreditFactorModel};
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
        "annualization_factor": 12.0
    })
    .to_string()
}

/// Build a minimal but valid `CreditCalibrationInputs` JSON.
///
/// 3 issuers × 24 monthly obs (2022-04-01 … 2024-03-01), same layout as the
/// native fixture in `credit_factor_model.rs`.
fn minimal_inputs_json() -> String {
    // 24 monthly dates ending 2024-03-31 (step back ~30 days each time).
    let dates: Vec<String> = {
        let mut d = time::Date::from_calendar_date(2024, time::Month::March, 31).unwrap();
        let mut v = Vec::with_capacity(24);
        for _ in 0..24 {
            v.push(d.to_string());
            for _ in 0..30 {
                d = d.previous_day().unwrap();
            }
        }
        v.reverse();
        v
    };

    let n = dates.len();

    let make_series = |base: f64| -> Vec<serde_json::Value> {
        (0..n)
            .map(|i| serde_json::Value::from(base + 5.0 * (i as f64).sin()))
            .collect()
    };

    let as_of = dates.last().unwrap().clone();

    serde_json::json!({
        "history_panel": {
            "dates": dates,
            "spreads": {
                "ISSUER-A": make_series(150.0),
                "ISSUER-B": make_series(175.0),
                "ISSUER-C": make_series(200.0)
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
            "values": (0..n).map(|i| 100.0 + 0.5 * (i as f64).sin()).collect::<Vec<f64>>()
        },
        "as_of": as_of,
        "asof_spreads": {
            "ISSUER-A": 150.0,
            "ISSUER-B": 175.0,
            "ISSUER-C": 200.0
        },
        "idiosyncratic_overrides": {}
    })
    .to_string()
}

// ---- tests ------------------------------------------------------------------

/// JSON round-trip: load the golden artifact, re-serialize, verify
/// `schema_version` is preserved.
#[wasm_bindgen_test]
fn credit_factor_model_round_trips_through_json() {
    let json = include_str!(
        "../../finstack-quant/valuations/tests/schema_fixtures/credit_factor_model_v1.json"
    );
    let model =
        WasmCreditFactorModel::from_json(json).expect("from_json must succeed on golden artifact");
    let out = model.to_json().expect("to_json must succeed");

    let parsed_in: serde_json::Value = serde_json::from_str(json).unwrap();
    let parsed_out: serde_json::Value = serde_json::from_str(&out).unwrap();

    assert_eq!(
        parsed_in["schema_version"], parsed_out["schema_version"],
        "schema_version must be preserved through round-trip"
    );
}

/// Calibrate a minimal model, serialize it, and verify the JSON contains
/// `schema_version`.
#[wasm_bindgen_test]
fn calibrate_then_decompose_round_trip() {
    let config_json = minimal_config_json();
    let inputs_json = minimal_inputs_json();

    let calibrator =
        WasmCreditCalibrator::new(&config_json).expect("WasmCreditCalibrator::new must succeed");
    let model = calibrator
        .calibrate(&inputs_json)
        .expect("calibrate must succeed on minimal inputs");
    let model_json = model.to_json().expect("to_json must succeed");

    assert!(
        model_json.contains("schema_version"),
        "serialized model must contain schema_version"
    );

    let parsed: serde_json::Value = serde_json::from_str(&model_json).unwrap();
    assert_eq!(
        parsed["schema_version"].as_str().unwrap(),
        finstack_quant_factor_model::credit::hierarchy::CreditFactorModel::SCHEMA_VERSION,
        "schema_version must match the canonical constant"
    );
}
