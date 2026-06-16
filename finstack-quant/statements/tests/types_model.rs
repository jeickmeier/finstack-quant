//! Model spec integration tests.
#![allow(clippy::expect_used)]

use finstack_quant_core::dates::build_periods;
use finstack_quant_statements::types::{FinancialModelSpec, NodeSpec, NodeType};

#[test]
fn test_model_spec_creation() {
    let periods = build_periods("2025Q1..Q4", None)
        .expect("valid period range")
        .periods;
    let model = FinancialModelSpec::new("test_model", periods);

    assert_eq!(model.id, "test_model");
    assert_eq!(model.periods.len(), 4);
    assert_eq!(model.schema_version, 2);
    assert!(model.nodes.is_empty());
}

#[test]
fn test_add_and_get_node() {
    let periods = build_periods("2025Q1..Q2", None)
        .expect("valid period range")
        .periods;
    let mut model = FinancialModelSpec::new("test", periods);

    let node = NodeSpec::new("revenue", NodeType::Value);
    model.add_node(node);

    assert!(model.has_node("revenue"));
    assert_eq!(
        model
            .get_node("revenue")
            .expect("revenue node should exist")
            .node_id,
        "revenue"
    );
}

#[test]
fn test_serialization_roundtrip() {
    let periods = build_periods("2025Q1..Q2", None)
        .expect("valid period range")
        .periods;
    let model = FinancialModelSpec::new("test", periods);

    let json = serde_json::to_string(&model).expect("model should serialize");
    let deserialized: FinancialModelSpec =
        serde_json::from_str(&json).expect("model should deserialize");

    assert_eq!(model, deserialized);
}
