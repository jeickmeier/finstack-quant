//! wasm-bindgen-test suite for `api::statements`.
//!
//! Covers JsValue-returning functions (node enumeration) and the evaluator
//! / validator / DSL entry points.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::statements::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn model_with_nodes(nodes: &[&str]) -> String {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::types::AmountOrScalar;

    let q1 = PeriodId::quarter(2024, 1).expect("valid period fixture");
    let mut builder = ModelBuilder::new("test")
        .periods("2024Q1..Q1", None)
        .unwrap();
    for &name in nodes {
        builder = builder.value(name, &[(q1, AmountOrScalar::scalar(100.0))]);
    }
    let model = builder.build().unwrap();
    serde_json::to_string(&model).unwrap()
}

#[wasm_bindgen_test]
fn model_node_ids_returns_array() {
    let json = model_with_nodes(&["revenue"]);
    let result = model_node_ids(JsValue::from(&json)).unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert_eq!(ids, vec!["revenue"]);
}

#[wasm_bindgen_test]
fn model_node_ids_empty_model() {
    let model = finstack_quant_statements::FinancialModelSpec::new("empty", vec![]);
    let json = serde_json::to_string(&model).unwrap();
    let result = model_node_ids(JsValue::from(&json)).unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(ids.is_empty());
}

#[wasm_bindgen_test]
fn model_node_ids_multiple_nodes() {
    let json = model_with_nodes(&["revenue", "cogs", "gp"]);
    let result = model_node_ids(JsValue::from(&json)).unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert_eq!(ids.len(), 3);
}

// Evaluator

#[wasm_bindgen_test]
fn evaluate_model_produces_computed_nodes() {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::types::AmountOrScalar;

    let q1 = PeriodId::quarter(2024, 1).expect("valid period fixture");
    let q2 = PeriodId::quarter(2024, 2).expect("valid period fixture");
    let model = ModelBuilder::new("demo")
        .periods("2024Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (q1, AmountOrScalar::scalar(100.0)),
                (q2, AmountOrScalar::scalar(110.0)),
            ],
        )
        .compute("gross_profit", "revenue * 0.4")
        .unwrap()
        .build()
        .unwrap();
    let model_json = serde_json::to_string(&model).unwrap();

    // `evaluate_model` returns a structured JS object; decode it back into the
    // canonical Rust type to assert the evaluated values.
    let out = evaluate_model(JsValue::from(&model_json)).unwrap();
    let result: finstack_quant_statements::evaluator::StatementResult =
        serde_wasm_bindgen::from_value(out).unwrap();
    assert!(result.nodes.contains_key("revenue"));
    assert!(result.nodes.contains_key("gross_profit"));
    let gp_q1 = result
        .nodes
        .get("gross_profit")
        .and_then(|m| m.get(&q1))
        .copied()
        .unwrap();
    assert!((gp_q1 - 40.0).abs() < 1e-9);
}

// DSL

#[wasm_bindgen_test]
fn parse_formula_returns_canonical_text() {
    assert_eq!(
        parse_formula(JsValue::from("revenue-cogs")).unwrap(),
        "revenue - cogs"
    );
}

#[wasm_bindgen_test]
fn parse_and_compile_accepts_valid() {
    parse_and_compile(JsValue::from("a + b")).expect("should accept valid formula");
}

// Capital structure / waterfall validators

#[wasm_bindgen_test]
fn validate_waterfall_spec_roundtrips_minimal_spec() {
    let spec = finstack_quant_statements::capital_structure::WaterfallSpec {
        priority_of_payments:
            finstack_quant_statements::capital_structure::default_priority_of_payments(),
        available_cash_node: "cash".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };
    let json = serde_json::to_string(&spec).unwrap();
    let out = validate_waterfall_spec_json(JsValue::from(&json)).unwrap();
    assert!(out.contains("priority_of_payments"));
}

#[wasm_bindgen_test]
fn validate_ecf_sweep_spec_accepts_minimal() {
    let json = r#"{"ebitda_node":"ebitda","sweep_percentage":0.5}"#;
    let out = validate_ecf_sweep_spec_json(JsValue::from(json)).unwrap();
    assert!(out.contains("ebitda_node"));
}

#[wasm_bindgen_test]
fn validate_pik_toggle_spec_accepts_minimal() {
    let json = r#"{"liquidity_metric":"cash","threshold":1000000.0}"#;
    let out = validate_pik_toggle_spec_json(JsValue::from(json)).unwrap();
    assert!(out.contains("liquidity_metric"));
}

#[wasm_bindgen_test]
fn validate_capital_structure_spec_accepts_empty() {
    let json = r#"{}"#;
    let out = validate_capital_structure_spec_json(JsValue::from(json)).unwrap();
    // Empty spec serializes with default fields.
    assert!(!out.is_empty());
}

#[wasm_bindgen_test]
fn validate_financial_model_json_accepts_valid_model() {
    let periods = finstack_quant_core::dates::build_periods("2025Q1..Q1", None)
        .expect("valid periods")
        .periods;
    let model = finstack_quant_statements::FinancialModelSpec::new("test", periods);
    let json = serde_json::to_string(&model).expect("model should serialize to JSON");
    let out = validate_financial_model_json(JsValue::from(&json))
        .expect("validate_financial_model_json should accept valid model");
    let round_trip = serde_json::from_str::<finstack_quant_statements::FinancialModelSpec>(&out)
        .expect("validated JSON should deserialize");
    assert_eq!(round_trip.id, "test");
    assert!(round_trip.nodes.is_empty());
}

#[wasm_bindgen_test]
fn validate_check_suite_spec_roundtrip() {
    let spec = finstack_quant_statements::checks::CheckSuiteSpec {
        name: "test".to_string(),
        description: None,
        builtin_checks: vec![],
        formula_checks: vec![],
        config: finstack_quant_statements::checks::CheckConfig::default(),
    };
    let json = serde_json::to_string(&spec).expect("serialize");
    let out =
        validate_check_suite_spec_json(JsValue::from(&json)).expect("should accept valid spec");
    let rt = serde_json::from_str::<finstack_quant_statements::checks::CheckSuiteSpec>(&out)
        .expect("should roundtrip");
    assert_eq!(rt.name, "test");
}

#[wasm_bindgen_test]
fn validate_waterfall_spec_accepts_minimal_spec() {
    let spec = finstack_quant_statements::capital_structure::WaterfallSpec {
        priority_of_payments: vec![
            finstack_quant_statements::capital_structure::PaymentPriority::Fees,
            finstack_quant_statements::capital_structure::PaymentPriority::Interest,
            finstack_quant_statements::capital_structure::PaymentPriority::Amortization,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };
    let json = serde_json::to_string(&spec).expect("serialize");
    let out =
        validate_waterfall_spec_json(JsValue::from(&json)).expect("should accept default spec");
    assert!(out.contains("priority_of_payments"));
}

#[wasm_bindgen_test]
fn parse_formula_returns_canonical_text_binding() {
    let out = parse_formula(JsValue::from("revenue-cogs")).expect("parse_formula should succeed");
    assert_eq!(out, "revenue - cogs");
    // The canonical text parses back to itself.
    assert_eq!(parse_formula(JsValue::from(&out)).expect("reparse"), out);
}

#[wasm_bindgen_test]
fn parse_and_compile_accepts_valid_binding() {
    parse_and_compile(JsValue::from("revenue * 0.5")).expect("should accept valid formula");
}
