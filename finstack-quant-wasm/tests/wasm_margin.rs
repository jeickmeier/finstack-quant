//! wasm-bindgen-test suite for `api::margin`.
//!
//! Covers calculate_vm which returns JsValue.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::margin::*;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn calculate_vm_usd_regulatory() {
    let csa_json = csa_usd_regulatory().unwrap();
    let result = calculate_vm(&csa_json, 1_000_000.0, 500_000.0, "USD", 2024, 6, 15).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["gross_exposure"].as_f64().is_some());
    assert!(obj["net_exposure"].as_f64().is_some());
    assert!(obj["delivery_amount"].as_f64().is_some());
    assert!(obj["return_amount"].as_f64().is_some());
    assert!(obj["net_margin"].as_f64().is_some());
    assert!(obj["requires_call"].as_bool().is_some());
}

#[wasm_bindgen_test]
fn calculate_vm_eur_regulatory() {
    let csa_json = csa_eur_regulatory().unwrap();
    let result = calculate_vm(&csa_json, 500_000.0, 600_000.0, "EUR", 2024, 3, 1).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["gross_exposure"].as_f64().is_some());
}

#[wasm_bindgen_test]
fn calculate_vm_zero_exposure() {
    let csa_json = csa_usd_regulatory().unwrap();
    let result = calculate_vm(&csa_json, 0.0, 0.0, "USD", 2024, 1, 15).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!((obj["gross_exposure"].as_f64().unwrap()).abs() < 1e-10);
}
