//! wasm-bindgen-test suite for `api::monte_carlo`.
//!
//! Covers MC pricing functions that return JsValue.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::monte_carlo::*;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn price_european_call_returns_valid_result() {
    let result =
        price_european_call(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, 1000, 42, Some(50), None).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["mean"].as_f64().unwrap() > 0.0);
    assert_eq!(obj["currency"].as_str().unwrap(), "USD");
    assert_eq!(obj["num_paths"].as_u64().unwrap(), 1000);
    assert!(obj["stderr"].as_f64().unwrap() > 0.0);
    assert!(obj["ci_lower"].as_f64().unwrap() < obj["ci_upper"].as_f64().unwrap());
}

#[wasm_bindgen_test]
fn price_european_put_returns_valid_result() {
    let result = price_european_put(
        100.0,
        100.0,
        0.05,
        0.0,
        0.2,
        1.0,
        1000,
        42,
        Some(50),
        Some("EUR".to_string()),
    )
    .unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["mean"].as_f64().unwrap() > 0.0);
    assert_eq!(obj["currency"].as_str().unwrap(), "EUR");
}

#[wasm_bindgen_test]
fn price_european_call_deep_itm() {
    let result =
        price_european_call(150.0, 100.0, 0.05, 0.0, 0.2, 1.0, 2000, 123, Some(50), None).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["mean"].as_f64().unwrap() > 40.0);
}

#[wasm_bindgen_test]
fn price_asian_call_returns_result() {
    let result =
        price_asian_call(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, 500, 42, Some(12), None).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["mean"].as_f64().unwrap() >= 0.0);
    assert_eq!(obj["num_paths"].as_u64().unwrap(), 500);
}

#[wasm_bindgen_test]
fn price_asian_put_returns_result() {
    let result =
        price_asian_put(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, 500, 42, Some(12), None).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["mean"].as_f64().unwrap() >= 0.0);
}

#[wasm_bindgen_test]
fn price_american_put_returns_result() {
    let result = price_american_put(
        100.0,
        100.0,
        0.05,
        0.0,
        0.2,
        1.0,
        500,
        42,
        Some(20),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["mean"].as_f64().unwrap() > 0.0);
}
