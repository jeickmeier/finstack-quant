//! wasm-bindgen-test suite for `api::statements`.
//!
//! Covers JsValue-returning functions (node enumeration) and the evaluator
//! / validator / DSL entry points.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::statements::*;
use wasm_bindgen_test::*;

fn model_with_nodes(nodes: &[&str]) -> String {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::types::AmountOrScalar;

    let q1 = PeriodId::quarter(2024, 1);
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
    let result = model_node_ids(&json).unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert_eq!(ids, vec!["revenue"]);
}

#[wasm_bindgen_test]
fn model_node_ids_empty_model() {
    let model = finstack_quant_statements::FinancialModelSpec::new("empty", vec![]);
    let json = serde_json::to_string(&model).unwrap();
    let result = model_node_ids(&json).unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(ids.is_empty());
}

#[wasm_bindgen_test]
fn model_node_ids_multiple_nodes() {
    let json = model_with_nodes(&["revenue", "cogs", "gp"]);
    let result = model_node_ids(&json).unwrap();
    let ids: Vec<String> = serde_wasm_bindgen::from_value(result).unwrap();
    assert_eq!(ids.len(), 3);
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn evaluate_model_produces_computed_nodes() {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::types::AmountOrScalar;

    let q1 = PeriodId::quarter(2024, 1);
    let q2 = PeriodId::quarter(2024, 2);
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

    let out = evaluate_model(&model_json).unwrap();
    let result: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(&out).unwrap();
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

// ---------------------------------------------------------------------------
// DSL
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn parse_formula_returns_non_empty_ast() {
    let out = parse_formula("revenue - cogs").unwrap();
    assert!(!out.is_empty());
}

#[wasm_bindgen_test]
fn validate_formula_accepts_valid() {
    assert!(validate_formula("a + b").unwrap());
}

// ---------------------------------------------------------------------------
// Capital structure / waterfall validators
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn validate_waterfall_spec_roundtrips_default() {
    let spec = finstack_quant_statements::capital_structure::WaterfallSpec::default();
    let json = serde_json::to_string(&spec).unwrap();
    let out = validate_waterfall_spec(&json).unwrap();
    assert!(out.contains("priority_of_payments"));
}

#[wasm_bindgen_test]
fn validate_ecf_sweep_spec_accepts_minimal() {
    let json = r#"{"ebitda_node":"ebitda","sweep_percentage":0.5}"#;
    let out = validate_ecf_sweep_spec(json).unwrap();
    assert!(out.contains("ebitda_node"));
}

#[wasm_bindgen_test]
fn validate_pik_toggle_spec_accepts_minimal() {
    let json = r#"{"liquidity_metric":"cash","threshold":1000000.0}"#;
    let out = validate_pik_toggle_spec(json).unwrap();
    assert!(out.contains("liquidity_metric"));
}

#[wasm_bindgen_test]
fn validate_capital_structure_spec_accepts_empty() {
    let json = r#"{}"#;
    let out = validate_capital_structure_spec(json).unwrap();
    // Empty spec serializes with default fields.
    assert!(!out.is_empty());
}
