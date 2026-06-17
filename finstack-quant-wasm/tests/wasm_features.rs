//! wasm-bindgen-test suite for `api::features`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::features::{
    clean_signal, neutralize, neutralize_and_zscore, normalize_signal, rank_to_weights,
    risk_scaled_weights, transform_cross_sectional, transform_cross_sectional_grouped,
    transform_panel, transform_timeseries, transform_timeseries_pairwise,
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
fn transform_expanded_feature_ops_return_js_arrays() {
    let values = serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(2.0), Some(4.0)])
        .expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec![
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
    ])
    .expect("time key");

    let ranks =
        transform_cross_sectional(values, time_key, "percentile_rank", None).expect("percentile");
    let ranks: Vec<Option<f64>> = serde_wasm_bindgen::from_value(ranks).expect("rank vec");
    assert_eq!(ranks, vec![Some(0.2), Some(0.5), Some(0.5), Some(0.8)]);

    let values =
        serde_wasm_bindgen::to_value(&vec![Some(1.0), None, Some(f64::NAN)]).expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec!["2026-01-01", "2026-01-01", "2026-01-01"])
        .expect("time key");
    let params = serde_wasm_bindgen::to_value(&json!({"value": 7.0})).expect("params");
    let filled =
        transform_cross_sectional(values, time_key, "fill_missing", Some(params)).expect("filled");
    let filled: Vec<Option<f64>> = serde_wasm_bindgen::from_value(filled).expect("filled vec");
    assert_eq!(filled, vec![Some(1.0), Some(7.0), Some(7.0)]);

    let values =
        serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(3.0), Some(6.0)]).expect("values");
    let entity = serde_wasm_bindgen::to_value(&vec!["A", "A", "A"]).expect("entity");
    let order = serde_wasm_bindgen::to_value(&vec!["2026-01-01", "2026-01-02", "2026-01-03"])
        .expect("order");
    let diff = transform_timeseries(values, entity, order, "diff", None).expect("diff");
    let diff: Vec<Option<f64>> = serde_wasm_bindgen::from_value(diff).expect("diff vec");
    assert_eq!(diff, vec![None, Some(2.0), Some(3.0)]);
}

#[wasm_bindgen_test]
fn finance_specific_feature_ops_return_js_arrays() {
    let values = serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(3.0), Some(10.0), Some(14.0)])
        .expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec![
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
    ])
    .expect("time key");
    let groups = serde_wasm_bindgen::to_value(&vec!["tech", "tech", "fin", "fin"]).expect("groups");
    let grouped = transform_cross_sectional_grouped(values, time_key, groups, "zscore", None)
        .expect("grouped");
    let grouped: Vec<Option<f64>> = serde_wasm_bindgen::from_value(grouped).expect("grouped vec");
    assert_eq!(grouped, vec![Some(-1.0), Some(1.0), Some(-1.0), Some(1.0)]);

    let values = serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(2.0), Some(4.0)])
        .expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec![
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
    ])
    .expect("time key");
    let exposures =
        serde_wasm_bindgen::to_value(&vec![vec![Some(0.0), Some(1.0), Some(0.0), Some(1.0)]])
            .expect("exposures");
    let residual = neutralize(values, time_key, exposures, None).expect("neutralize");
    let residual: Vec<Option<f64>> = serde_wasm_bindgen::from_value(residual).expect("residual");
    assert_eq!(residual, vec![Some(-0.5), Some(-1.0), Some(0.5), Some(1.0)]);

    let values =
        serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(3.0)]).expect("values");
    let other =
        serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(4.0)]).expect("other");
    let entity = serde_wasm_bindgen::to_value(&vec!["A", "A", "A"]).expect("entity");
    let order = serde_wasm_bindgen::to_value(&vec!["2026-01-01", "2026-01-02", "2026-01-03"])
        .expect("order");
    let params =
        serde_wasm_bindgen::to_value(&json!({"window": 3, "min_periods": 3})).expect("params");
    let beta =
        transform_timeseries_pairwise(values, other, entity, order, "rolling_beta", Some(params))
            .expect("beta");
    let beta: Vec<Option<f64>> = serde_wasm_bindgen::from_value(beta).expect("beta vec");
    assert_eq!(beta[0], None);
    assert_eq!(beta[1], None);
    assert!((beta[2].expect("beta") - 9.0 / 14.0).abs() < 1e-12);

    let values = serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0)]).expect("values");
    let time_key =
        serde_wasm_bindgen::to_value(&vec!["2026-01-01", "2026-01-01"]).expect("time key");
    let volatility = serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0)]).expect("volatility");
    let weights = risk_scaled_weights(values, time_key, volatility, None).expect("weights");
    let weights: Vec<Option<f64>> = serde_wasm_bindgen::from_value(weights).expect("weights vec");
    assert_eq!(weights, vec![Some(0.5), Some(0.5)]);
}

#[wasm_bindgen_test]
fn pipeline_helper_feature_ops_return_js_arrays() {
    let values =
        serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(100.0)]).expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec!["2026-01-01", "2026-01-01", "2026-01-01"])
        .expect("time key");
    let params =
        serde_wasm_bindgen::to_value(&json!({"lower": 0.0, "upper": 0.5})).expect("params");
    let cleaned = clean_signal(values, time_key, Some(params)).expect("cleaned");
    let cleaned: Vec<Option<f64>> = serde_wasm_bindgen::from_value(cleaned).expect("cleaned vec");
    assert_eq!(cleaned, vec![Some(1.0), Some(2.0), Some(2.0)]);

    let values =
        serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(100.0)]).expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec!["2026-01-01", "2026-01-01", "2026-01-01"])
        .expect("time key");
    let params = serde_wasm_bindgen::to_value(&json!({"method": "rank"})).expect("params");
    let normalized = normalize_signal(values, time_key, Some(params)).expect("normalized");
    let normalized: Vec<Option<f64>> =
        serde_wasm_bindgen::from_value(normalized).expect("normalized vec");
    assert_eq!(normalized, vec![Some(0.0), Some(0.5), Some(1.0)]);

    let values =
        serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(100.0)]).expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec!["2026-01-01", "2026-01-01", "2026-01-01"])
        .expect("time key");
    let weights = rank_to_weights(values, time_key, None).expect("weights");
    let weights: Vec<Option<f64>> = serde_wasm_bindgen::from_value(weights).expect("weights vec");
    assert_eq!(weights, vec![Some(-0.5), Some(0.0), Some(0.5)]);

    let values = serde_wasm_bindgen::to_value(&vec![Some(1.0), Some(2.0), Some(2.0), Some(4.0)])
        .expect("values");
    let time_key = serde_wasm_bindgen::to_value(&vec![
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
        "2026-01-01",
    ])
    .expect("time key");
    let exposures =
        serde_wasm_bindgen::to_value(&vec![vec![Some(0.0), Some(1.0), Some(0.0), Some(1.0)]])
            .expect("exposures");
    let scored = neutralize_and_zscore(values, time_key, exposures, None).expect("scored");
    let scored: Vec<Option<f64>> = serde_wasm_bindgen::from_value(scored).expect("scored vec");
    assert!((scored[0].expect("score") + 0.632_455_532_033_675_9).abs() < 1e-12);
    assert!((scored[3].expect("score") - 1.264_911_064_067_351_8).abs() < 1e-12);
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
