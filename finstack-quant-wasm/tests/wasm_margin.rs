//! wasm-bindgen-test suite for `api::margin`.
//!
//! Covers calculate_vm, which returns the canonical `VmResult` serde as a JsValue.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::margin::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn calculate_vm_usd_regulatory() {
    let csa_json = csa_usd_regulatory_json().unwrap();
    let result = calculate_vm(
        JsValue::from(&csa_json),
        1_000_000.0,
        500_000.0,
        JsValue::from("USD"),
        JsValue::from("2024-06-15"),
    )
    .unwrap();
    let vm: finstack_quant_margin::VmResult = serde_wasm_bindgen::from_value(result).unwrap();
    assert_eq!(vm.gross_exposure.amount(), 1_000_000.0);
    assert_eq!(vm.gross_exposure.currency().to_string(), "USD");
    assert!(vm.requires_call());
}

#[wasm_bindgen_test]
fn calculate_vm_eur_regulatory() {
    let csa_json = csa_eur_regulatory_json().unwrap();
    let result = calculate_vm(
        JsValue::from(&csa_json),
        500_000.0,
        600_000.0,
        JsValue::from("EUR"),
        JsValue::from("2024-03-01"),
    )
    .unwrap();
    let vm: finstack_quant_margin::VmResult = serde_wasm_bindgen::from_value(result).unwrap();
    assert_eq!(vm.gross_exposure.currency().to_string(), "EUR");
}

#[wasm_bindgen_test]
fn calculate_vm_zero_exposure() {
    let csa_json = csa_usd_regulatory_json().unwrap();
    let result = calculate_vm(
        JsValue::from(&csa_json),
        0.0,
        0.0,
        JsValue::from("USD"),
        JsValue::from("2024-01-15"),
    )
    .unwrap();
    let vm: finstack_quant_margin::VmResult = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(vm.gross_exposure.amount().abs() < 1e-10);
    assert!(!vm.requires_call());
}

#[wasm_bindgen_test]
fn validate_csa_json_round_trips_usd_regulatory() {
    let Ok(original) = csa_usd_regulatory_json() else {
        panic!("csa_usd_regulatory should succeed");
    };
    let Ok(parsed_once) = serde_json::from_str::<finstack_quant_margin::CsaSpec>(&original) else {
        panic!("original JSON should deserialize to CsaSpec");
    };
    let Ok(canonical) = validate_csa_json(JsValue::from(&original)) else {
        panic!("validate_csa_json should succeed on regulatory CSA JSON");
    };
    let Ok(parsed_twice) = serde_json::from_str::<finstack_quant_margin::CsaSpec>(&canonical)
    else {
        panic!("canonical JSON should deserialize to CsaSpec");
    };
    assert_eq!(parsed_once, parsed_twice);
}
