//! wasm-bindgen-test suite for `api::features`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::features::{
    transform_cross_sectional, transform_panel, transform_timeseries,
};
use serde_json::json;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn transform_timeseries_and_cross_sectional_return_js_arrays() {
    let values =
        serde_wasm_bindgen::to_value(&vec![Some(12.0), Some(10.0), Some(21.0), Some(20.0)])
            .expect("values");
    let entity = serde_wasm_bindgen::to_value(&vec!["A", "A", "B", "B"]).expect("entity");
    let order = serde_wasm_bindgen::to_value(&vec![
        "2026-01-02",
        "2026-01-01",
        "2026-01-02",
        "2026-01-01",
    ])
    .expect("order");
    let params = serde_wasm_bindgen::to_value(&json!({"periods": 1})).expect("params");

    let returns =
        transform_timeseries(values, entity, order, "returns", Some(params)).expect("returns");
    let returns: Vec<Option<f64>> = serde_wasm_bindgen::from_value(returns).expect("returns vec");
    assert!((returns[0].expect("A return") - 0.2).abs() < 1e-12);
    assert_eq!(returns[1], None);
    assert!((returns[2].expect("B return") - 0.05).abs() < 1e-12);

    let values = serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(100.0), Some(5.0)])
        .expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec![
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
        "2026-01-02",
    ])
    .expect("time key");
    let ranks = transform_cross_sectional(values, time_key, "rank", None).expect("rank");
    let ranks: Vec<Option<f64>> = serde_wasm_bindgen::from_value(ranks).expect("rank vec");
    assert_eq!(ranks, vec![Some(0.0), Some(0.5), Some(1.0), Some(0.0)]);
}

#[wasm_bindgen_test]
fn transform_panel_returns_json_result() {
    let spec = json!({
        "values": [10.0, 12.0, 20.0, 21.0],
        "entity": ["A", "A", "B", "B"],
        "order": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
        "time_key": ["2026-01-01", "2026-01-02", "2026-01-01", "2026-01-02"],
        "operations": [
            {"name": "ret1", "family": "timeseries", "op": "returns", "params": {"periods": 1}},
            {"name": "rank", "family": "cross_sectional", "op": "rank"}
        ]
    });

    let out = transform_panel(&spec.to_string()).expect("panel");
    let result: serde_json::Value = serde_json::from_str(&out).expect("panel JSON");
    assert!((result["columns"]["ret1"][1].as_f64().expect("ret1") - 0.2).abs() < 1e-12);
    assert_eq!(result["columns"]["rank"][2], 1.0);
}
