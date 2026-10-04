//! wasm-bindgen-test suite for `api::margin`.
//!
//! Covers `VmCalculator`, which returns the canonical `VmResult` as a JsValue.

use super::calculators::JsVmCalculator;
use super::types::{csa_spec_eur_regulatory, csa_spec_usd_regulatory};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn calculate_vm_usd_regulatory() {
    let calculator = JsVmCalculator::new(csa_spec_usd_regulatory().unwrap()).unwrap();
    let result = calculator
        .calculate(
            JsValue::from(1_000_000.0),
            JsValue::from(500_000.0),
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
    let calculator = JsVmCalculator::new(csa_spec_eur_regulatory().unwrap()).unwrap();
    let result = calculator
        .calculate(
            JsValue::from(500_000.0),
            JsValue::from(600_000.0),
            JsValue::from("EUR"),
            JsValue::from("2024-03-01"),
        )
        .unwrap();
    let vm: finstack_quant_margin::VmResult = serde_wasm_bindgen::from_value(result).unwrap();
    assert_eq!(vm.gross_exposure.currency().to_string(), "EUR");
}

#[wasm_bindgen_test]
fn calculate_vm_zero_exposure() {
    let calculator = JsVmCalculator::new(csa_spec_usd_regulatory().unwrap()).unwrap();
    let result = calculator
        .calculate(
            JsValue::from(0.0),
            JsValue::from(0.0),
            JsValue::from("USD"),
            JsValue::from("2024-01-15"),
        )
        .unwrap();
    let vm: finstack_quant_margin::VmResult = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(vm.gross_exposure.amount().abs() < 1e-10);
    assert!(!vm.requires_call());
}

#[wasm_bindgen_test]
fn vm_calculator_csa_round_trips_usd_regulatory() {
    let original = csa_spec_usd_regulatory().unwrap();
    let parsed_once: finstack_quant_margin::CsaSpec =
        serde_wasm_bindgen::from_value(original.clone()).unwrap();
    let json = js_sys::JSON::stringify(&original)
        .unwrap()
        .as_string()
        .unwrap();
    let calculator = JsVmCalculator::new(JsValue::from(json.as_str())).unwrap();
    let parsed_twice: finstack_quant_margin::CsaSpec =
        serde_wasm_bindgen::from_value(calculator.csa().unwrap()).unwrap();
    assert_eq!(parsed_once, parsed_twice);
}
