//! wasm-bindgen tests for `api::models::liquidity`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::models::liquidity::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn as_json(value: &JsValue) -> serde_json::Value {
    let text = js_sys::JSON::stringify(value)
        .expect("result must be JSON.stringify-able")
        .as_string()
        .expect("JSON.stringify yields a string");
    assert!(
        text.len() > 2,
        "structured result must not be empty: {text}"
    );
    serde_json::from_str(&text).expect("stringified result must be valid JSON")
}

fn get_f64(value: &JsValue, key: &str) -> f64 {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .unwrap_or_else(|_| panic!("property read for {key}"))
        .as_f64()
        .unwrap_or_else(|| panic!("property {key} must be directly readable as a number"))
}

#[wasm_bindgen_test]
fn estimators_return_none_for_missing_estimates() {
    assert_eq!(
        roll_effective_spread(JsValue::from("[0.01]")).unwrap(),
        None
    );
    assert_eq!(
        amihud_illiquidity(JsValue::from("[0.01]"), JsValue::from("[0.0]")).unwrap(),
        None
    );
    assert_eq!(
        kyle_lambda(JsValue::from("[0.01]"), JsValue::from("[0.0]"), 100.0).unwrap(),
        None
    );
}

#[wasm_bindgen_test]
fn kyle_lambda_calibrates_in_price_space() {
    let lambda = kyle_lambda(
        JsValue::from("[0.01, -0.02]"),
        JsValue::from("[100.0, 200.0]"),
        50.0,
    )
    .unwrap()
    .expect("valid price-space inputs");
    assert!((lambda - 0.005).abs() < 1e-15);
}

#[wasm_bindgen_test]
fn lvar_bangia_returns_the_python_dict_shape() {
    let result = lvar_bangia(-100_000.0, 0.002, 0.0005, 0.99, 1_000_000.0).unwrap();
    let var = get_f64(&result, "var");
    let spread_cost = get_f64(&result, "spread_cost");
    let lvar = get_f64(&result, "lvar");
    let ratio = get_f64(&result, "lvar_ratio");
    assert!((var - -100_000.0).abs() < 1e-9);
    assert!(spread_cost >= 0.0);
    assert!(lvar <= var);
    assert!((ratio - lvar / var).abs() < 1e-12);
    assert!(as_json(&result).is_object());
}

#[wasm_bindgen_test]
fn almgren_chriss_impact_preserves_fields_and_price_scaling() {
    let unit = almgren_chriss_impact(10_000.0, 1_000_000.0, 0.02, 1.0, 0.0, 0.01, None).unwrap();
    let priced =
        almgren_chriss_impact(10_000.0, 1_000_000.0, 0.02, 1.0, 0.0, 0.01, Some(100.0)).unwrap();

    let object = as_json(&priced);
    assert_eq!(object.as_object().expect("impact object").len(), 5);
    for key in [
        "permanent_impact",
        "temporary_impact",
        "total_cost",
        "cost_bp",
        "execution_risk",
    ] {
        assert!(object.get(key).is_some(), "missing key {key}");
    }

    let unit_bp = get_f64(&unit, "cost_bp");
    let priced_bp = get_f64(&priced, "cost_bp");
    assert!((priced_bp - unit_bp).abs() < 1e-12 * unit_bp.abs().max(1.0));
    let unit_cost = get_f64(&unit, "total_cost");
    let priced_cost = get_f64(&priced, "total_cost");
    assert!((priced_cost - 100.0 * unit_cost).abs() < 1e-9 * priced_cost.abs().max(1.0));
}

#[wasm_bindgen_test]
fn estimators_return_none_for_missing_estimates_binding() {
    assert_eq!(
        roll_effective_spread(JsValue::from("[0.01]")).expect("valid JSON"),
        None
    );
    assert_eq!(
        amihud_illiquidity(JsValue::from("[0.01]"), JsValue::from("[0.0]")).expect("valid JSON"),
        None
    );
    assert_eq!(
        kyle_lambda(JsValue::from("[0.01]"), JsValue::from("[0.0]"), 100.0).expect("valid JSON"),
        None
    );
}

#[wasm_bindgen_test]
fn kyle_lambda_calibrates_in_price_space_binding() {
    let lambda = kyle_lambda(
        JsValue::from("[0.01, -0.02]"),
        JsValue::from("[100.0, 200.0]"),
        50.0,
    )
    .expect("valid JSON")
    .expect("valid price-space inputs");
    assert!((lambda - 0.005).abs() < 1e-15);
}
