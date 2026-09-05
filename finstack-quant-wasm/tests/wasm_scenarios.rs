//! wasm-bindgen-test suite for `api::scenarios`.
//!
//! Covers list_builtin_templates, list_template_components,
//! apply_scenario, and apply_scenario_to_market which return JsValue.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::scenarios::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

fn empty_market_json() -> String {
    let ctx = finstack_quant_core::market_data::context::MarketContext::new();
    serde_json::to_string(&ctx).unwrap()
}

fn empty_model_json() -> String {
    let model = finstack_quant_statements::FinancialModelSpec::new("test", vec![]);
    serde_json::to_string(&model).unwrap()
}

fn built_scenario_json(resolution_mode: Option<String>) -> String {
    let operations =
        serde_wasm_bindgen::to_value(&Vec::<finstack_quant_scenarios::OperationSpec>::new())
            .unwrap();
    let value =
        build_scenario_spec("test", operations, None, None, None, resolution_mode, None).unwrap();
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_wasm_bindgen::from_value(value).unwrap();
    serde_json::to_string(&spec).unwrap()
}

#[wasm_bindgen_test]
fn list_builtin_templates_returns_array() {
    let result = list_builtin_templates().unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(!ids.is_empty());
}

#[wasm_bindgen_test]
fn list_template_components_for_gfc() {
    let result = list_template_components("gfc_2008").unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(!ids.is_empty());
}

#[wasm_bindgen_test]
fn apply_scenario_empty_spec() {
    let scenario = built_scenario_json(None);
    let market = empty_market_json();
    let model = empty_model_json();
    let result = apply_scenario(&scenario, &market, &model, "2024-01-15", None).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    // `market`/`model` are nested objects now, not serialized strings: the
    // envelope used to hand back JSON-inside-JSON.
    assert!(
        obj["market"].is_object(),
        "market should be a nested object"
    );
    assert!(obj["model"].is_object(), "model should be a nested object");
    assert_eq!(obj["operations_applied"].as_u64().unwrap(), 0);
}

#[wasm_bindgen_test]
fn apply_scenario_to_market_empty_spec() {
    let scenario = built_scenario_json(None);
    let market = empty_market_json();
    let result = apply_scenario_to_market(&scenario, &market, "2024-06-01", None).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(
        obj["market"].is_object(),
        "market should be a nested object"
    );
    assert!(obj["model"].is_null(), "no model was supplied");
    assert_eq!(obj["operations_applied"].as_u64().unwrap(), 0);
}

#[wasm_bindgen_test]
fn build_scenario_spec_preserves_cumulative_resolution_mode() {
    let scenario = built_scenario_json(Some("cumulative".to_string()));
    let value: serde_json::Value = serde_json::from_str(&scenario).unwrap();
    assert_eq!(value["resolution_mode"], "cumulative");
}

#[wasm_bindgen_test]
fn compose_scenarios_rejects_mixed_hazard_bump_modes_as_javascript_error() {
    let operations =
        serde_wasm_bindgen::to_value(&Vec::<finstack_quant_scenarios::OperationSpec>::new())
            .expect("operations");
    let first_order = build_scenario_spec(
        "first-order",
        operations.clone(),
        None,
        None,
        Some(0),
        None,
        Some("first_order_shift".to_string()),
    )
    .expect("first-order scenario");
    let solve_to_par = build_scenario_spec(
        "solve-to-par",
        operations,
        None,
        None,
        Some(1),
        None,
        Some("solve_to_par".to_string()),
    )
    .expect("solve-to-par scenario");
    let first_order: finstack_quant_scenarios::ScenarioSpec =
        serde_wasm_bindgen::from_value(first_order).expect("typed first-order scenario");
    let solve_to_par: finstack_quant_scenarios::ScenarioSpec =
        serde_wasm_bindgen::from_value(solve_to_par).expect("typed solve-to-par scenario");
    let specs =
        serde_wasm_bindgen::to_value(&vec![first_order, solve_to_par]).expect("scenario array");

    let error = compose_scenarios(specs).expect_err("mixed modes should be rejected");
    let message: String = error
        .dyn_into::<js_sys::Error>()
        .expect("binding errors should be JavaScript Error objects")
        .message()
        .into();
    assert!(
        message.contains("first-order")
            && message.contains("first_order_shift")
            && message.contains("solve-to-par")
            && message.contains("solve_to_par"),
        "unexpected error: {message}"
    );
}

#[wasm_bindgen_test]
fn instrument_copies_are_returned_and_missing_inventory_is_rejected() {
    use finstack_quant_scenarios::{InstrumentType, OperationSpec, ScenarioSpec};
    use finstack_quant_valuations::instruments::{Bond, Instrument, InstrumentEnvelope};
    let bond = Bond::example().unwrap();
    let inventory = serde_json::to_string(&vec![InstrumentEnvelope::new(
        bond.to_instrument_json().unwrap(),
    )])
    .unwrap();
    let scenario = ScenarioSpec {
        id: "price".into(),
        operations: vec![
            OperationSpec::InstrumentPricePctByType {
                instrument_types: vec![InstrumentType::Bond],
                pct: -60.0,
            },
            OperationSpec::InstrumentPricePctByType {
                instrument_types: vec![InstrumentType::Bond],
                pct: -60.0,
            },
        ],
        ..Default::default()
    };
    let scenario = serde_json::to_string(&scenario).unwrap();
    assert!(apply_scenario_to_market(&scenario, &empty_market_json(), "2025-01-15", None).is_err());
    for with_model in [false, true] {
        let result = if with_model {
            apply_scenario(
                &scenario,
                &empty_market_json(),
                &empty_model_json(),
                "2025-01-15",
                Some(inventory.clone()),
            )
        } else {
            apply_scenario_to_market(
                &scenario,
                &empty_market_json(),
                "2025-01-15",
                Some(inventory.clone()),
            )
        }
        .unwrap();
        let result: finstack_quant_scenarios::ApplicationEnvelope =
            serde_wasm_bindgen::from_value(result).unwrap();
        let returned = result.instruments.unwrap().remove(0).into_boxed().unwrap();
        let shock = returned
            .get_scenario_pricing_overrides()
            .unwrap()
            .scenario_price_shock_pct
            .unwrap();
        assert!((100.0 * (1.0 + shock) - 16.0).abs() < 1e-12);
    }
}
