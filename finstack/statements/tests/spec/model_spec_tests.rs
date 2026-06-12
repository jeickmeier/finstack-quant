//! Tests for FinancialModelSpec operations and methods.

use finstack_statements::prelude::*;

#[test]
fn test_model_spec_add_node() {
    use finstack_core::dates::build_periods;
    use finstack_statements::types::{FinancialModelSpec, NodeSpec};

    let periods = build_periods("2025Q1..2025Q2", None).unwrap().periods;
    let mut model = FinancialModelSpec::new("test", periods);

    let node = NodeSpec::new("revenue", NodeType::Value);
    model.add_node(node);

    assert_eq!(model.nodes.len(), 1);
    assert!(model.has_node("revenue"));
}

#[test]
fn test_model_spec_get_node_mut() {
    use finstack_core::dates::build_periods;
    use finstack_statements::types::{FinancialModelSpec, NodeSpec};

    let periods = build_periods("2025Q1..2025Q2", None).unwrap().periods;
    let mut model = FinancialModelSpec::new("test", periods);

    let node = NodeSpec::new("revenue", NodeType::Value);
    model.add_node(node);

    // Get mutable reference and modify
    if let Some(node_mut) = model.get_node_mut("revenue") {
        node_mut.name = Some("Total Revenue".into());
    }

    assert_eq!(
        model.get_node("revenue").unwrap().name.as_ref().unwrap(),
        "Total Revenue"
    );
}

#[test]
fn test_model_spec_get_node_immutable() {
    use finstack_core::dates::build_periods;
    use finstack_statements::types::{FinancialModelSpec, NodeSpec};

    let periods = build_periods("2025Q1..2025Q2", None).unwrap().periods;
    let mut model = FinancialModelSpec::new("test", periods);

    let node = NodeSpec::new("revenue", NodeType::Value).with_name("Revenue");
    model.add_node(node);

    let node_ref = model.get_node("revenue");
    assert!(node_ref.is_some());
    assert_eq!(node_ref.unwrap().name.as_ref().unwrap(), "Revenue");
}

#[test]
fn test_model_spec_has_node() {
    use finstack_core::dates::build_periods;
    use finstack_statements::types::{FinancialModelSpec, NodeSpec};

    let periods = build_periods("2025Q1..2025Q2", None).unwrap().periods;
    let mut model = FinancialModelSpec::new("test", periods);

    assert!(!model.has_node("revenue"));

    let node = NodeSpec::new("revenue", NodeType::Value);
    model.add_node(node);

    assert!(model.has_node("revenue"));
    assert!(!model.has_node("cogs"));
}

#[test]
fn test_model_spec_get_node_none() {
    use finstack_core::dates::build_periods;
    use finstack_statements::types::FinancialModelSpec;

    let periods = build_periods("2025Q1..2025Q2", None).unwrap().periods;
    let mut model = FinancialModelSpec::new("test", periods);

    assert!(model.get_node("nonexistent").is_none());
    assert!(model.get_node_mut("nonexistent").is_none());
}

// ============================================================================
// Results Method Tests
// ============================================================================

#[test]
fn test_results_get_node() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let revenue_map = results.get_node("revenue");
    assert!(revenue_map.is_some());
    assert_eq!(revenue_map.unwrap().len(), 2);
}

#[test]
fn test_results_all_periods() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q3", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(120.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let all_periods: Vec<_> = results.all_periods("revenue").collect();
    assert_eq!(all_periods.len(), 3);
}

#[test]
fn test_results_get_or() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    let q2 = PeriodId::quarter(2025, 2);

    // Existing value
    assert_eq!(results.get_or("revenue", &q1, 0.0), 100.0);

    // Missing value - use default
    assert_eq!(results.get_or("revenue", &q2, 0.0), 0.0);

    // Missing node - use default
    assert_eq!(results.get_or("nonexistent", &q1, -1.0), -1.0);
}

// ============================================================================
// FinancialModelSpec Metadata Tests
// ============================================================================

#[test]
fn test_model_spec_with_metadata() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .with_meta("author", serde_json::json!("Test Author"))
        .with_meta("version", serde_json::json!("1.0.0"))
        .with_meta("created_at", serde_json::json!("2025-01-01"))
        .build()
        .unwrap();

    assert_eq!(model.meta.len(), 3);
    assert_eq!(model.meta.get("author").unwrap(), "Test Author");
    assert_eq!(model.meta.get("version").unwrap(), "1.0.0");
    assert_eq!(model.meta.get("created_at").unwrap(), "2025-01-01");
}

// ============================================================================
// Where Clause Tests
// ============================================================================

#[test]
fn test_where_clause_masking() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(800_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(1_200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(900_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(1_500_000.0),
                ),
            ],
        )
        .compute("bonus", "revenue * 0.01")
        .unwrap()
        .where_clause("revenue > 1000000")
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: revenue < 1M, bonus should be 0 (masked by where clause)
    assert_eq!(
        results.get("bonus", &PeriodId::quarter(2025, 1)).unwrap(),
        0.0
    );

    // Q2: revenue > 1M, bonus should be calculated
    assert_eq!(
        results.get("bonus", &PeriodId::quarter(2025, 2)).unwrap(),
        12_000.0
    );

    // Q3: revenue < 1M, bonus should be 0
    assert_eq!(
        results.get("bonus", &PeriodId::quarter(2025, 3)).unwrap(),
        0.0
    );

    // Q4: revenue > 1M, bonus should be calculated
    assert_eq!(
        results.get("bonus", &PeriodId::quarter(2025, 4)).unwrap(),
        15_000.0
    );
}

#[test]
fn test_where_clause_with_complex_condition() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(120_000.0),
                ),
            ],
        )
        .value(
            "margin",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.1)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.2)),
            ],
        )
        .compute("incentive", "revenue * 0.05")
        .unwrap()
        .where_clause("revenue > 100000 and margin > 0.15")
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: revenue > 100k but margin < 0.15 - should be 0
    assert_eq!(
        results
            .get("incentive", &PeriodId::quarter(2025, 1))
            .unwrap(),
        0.0
    );

    // Q2: revenue > 100k AND margin > 0.15 - should be calculated
    assert_eq!(
        results
            .get("incentive", &PeriodId::quarter(2025, 2))
            .unwrap(),
        6_000.0
    );
}

// ============================================================================
// Error Constructor Tests
// ============================================================================

#[test]
fn test_error_constructors() {
    use finstack_statements::error::Error;

    let build_err = Error::build("test build error");
    assert!(build_err.to_string().contains("build error"));

    let eval_err = Error::eval("test eval error");
    assert!(eval_err.to_string().contains("eval error"));

    let formula_err = Error::formula_parse("test parse error");
    assert!(formula_err.to_string().contains("parse error"));

    let period_err = Error::period("test period error");
    assert!(period_err.to_string().contains("period error"));

    let missing_err = Error::missing_data("test data");
    assert!(missing_err.to_string().contains("test data"));

    let invalid_err = Error::invalid_input("test input");
    assert!(invalid_err.to_string().contains("test input"));

    let registry_err = Error::registry("test registry error");
    assert!(registry_err.to_string().contains("registry error"));

    let forecast_err = Error::forecast("test forecast error");
    assert!(forecast_err.to_string().contains("forecast error"));

    let node_err = Error::node_not_found("test_node");
    assert!(node_err.to_string().contains("test_node"));

    let circ_err = Error::circular_dependency(vec!["a".into(), "b".into(), "a".into()]);
    assert!(circ_err.to_string().contains("→"));

    let curr_err = Error::currency_mismatch(Currency::USD, Currency::EUR);
    assert!(curr_err.to_string().contains("USD"));
    assert!(curr_err.to_string().contains("EUR"));

    let cs_err = Error::capital_structure("test cs error");
    assert!(cs_err.to_string().contains("cs error"));
}

// ============================================================================
// NodeId Tests
// ============================================================================

#[test]
fn test_node_id_from_str() {
    use finstack_statements::types::NodeId;

    let id = NodeId::from("revenue");
    assert_eq!(id.as_str(), "revenue");
    assert_eq!(id, "revenue");
    assert_eq!(id, NodeId::from("revenue"));
}

#[test]
fn test_node_id_from_string() {
    use finstack_statements::types::NodeId;

    let id = NodeId::from("gross_profit".to_string());
    assert_eq!(id.as_str(), "gross_profit");
    assert_eq!(id, "gross_profit");
}

#[test]
fn test_node_id_display() {
    use finstack_statements::types::NodeId;

    let id = NodeId::from("cogs");
    assert_eq!(format!("{}", id), "cogs");
}

#[test]
fn test_node_id_eq_str() {
    use finstack_statements::types::NodeId;

    let id = NodeId::from("ebitda");
    assert_eq!(id, "ebitda");
    assert_eq!(id, *"ebitda");
    assert_ne!(id, "revenue");
}

#[test]
fn test_node_id_clone_and_hash() {
    use finstack_statements::types::NodeId;
    use std::collections::HashSet;

    let id1 = NodeId::from("revenue");
    let id2 = id1.clone();
    assert_eq!(id1, id2);

    let mut set = HashSet::new();
    set.insert(id1);
    assert!(set.contains(&id2));
}

#[test]
fn test_node_id_borrow_str() {
    use finstack_statements::types::NodeId;
    use std::collections::HashMap;

    // Verify Borrow<str> allows HashMap<NodeId, _>.get(&str)
    let mut map: HashMap<NodeId, i32> = HashMap::new();
    map.insert(NodeId::from("revenue"), 100);
    // HashMap::get accepts any Q where K: Borrow<Q>. Because NodeId implements
    // Borrow<str> (and Hash/Eq are consistent), passing a &str key works without
    // an explicit NodeId conversion.
    let key: &str = "revenue";
    assert_eq!(map.get(key), Some(&100));
}

#[test]
fn test_node_id_serde_roundtrip() {
    use finstack_statements::types::NodeId;

    let id = NodeId::from("gross_profit");
    let json = serde_json::to_string(&id).expect("serialize NodeId");
    // Should serialize as a plain string, not {"0": "..."}
    assert_eq!(json, r#""gross_profit""#);

    let deserialized: NodeId = serde_json::from_str(&json).expect("deserialize NodeId");
    assert_eq!(deserialized, "gross_profit");
}

#[test]
fn test_node_id_as_map_key_serde() {
    use finstack_statements::types::NodeId;
    use indexmap::IndexMap;

    let mut map: IndexMap<NodeId, i32> = IndexMap::new();
    map.insert(NodeId::from("revenue"), 1);
    map.insert(NodeId::from("cogs"), 2);

    let json = serde_json::to_string(&map).expect("serialize map");
    // Keys should be plain strings
    assert!(json.contains(r#""revenue""#));
    assert!(json.contains(r#""cogs""#));

    let restored: IndexMap<NodeId, i32> = serde_json::from_str(&json).expect("deserialize map");
    assert_eq!(restored.get("revenue"), Some(&1));
    assert_eq!(restored.get("cogs"), Some(&2));
}

#[test]
fn test_node_spec_node_id_is_node_id_type() {
    use finstack_statements::types::{NodeId, NodeSpec, NodeType};

    let spec = NodeSpec::new("revenue", NodeType::Value);
    // node_id field is now NodeId, not String
    assert_eq!(spec.node_id, NodeId::from("revenue"));
    assert_eq!(spec.node_id.as_str(), "revenue");
}

#[test]
fn test_financial_model_nodes_keyed_by_node_id() {
    use finstack_core::dates::build_periods;
    use finstack_statements::types::{FinancialModelSpec, NodeId, NodeSpec, NodeType};

    let periods = build_periods("2025Q1..2025Q1", None).unwrap().periods;
    let mut model = FinancialModelSpec::new("test", periods);
    model.add_node(NodeSpec::new("revenue", NodeType::Value));

    // nodes map is keyed by NodeId
    let key = NodeId::from("revenue");
    assert!(model.nodes.contains_key(&key));

    // get_node still works with &str (via Borrow)
    assert!(model.has_node("revenue"));
    assert!(model.get_node("revenue").is_some());
}

#[test]
fn test_node_id_hyphenated_roundtrip() {
    use finstack_statements::types::NodeId;

    // Hyphenated IDs (e.g. BOND-001, lease-1) must survive a serde round-trip
    // unchanged and remain comparable to &str.
    for raw in &["lease-1", "BOND-001", "cost-of-goods", "tranche-A"] {
        let id = NodeId::from(*raw);
        let json = serde_json::to_string(&id).expect("serialize");
        let expected = format!(r#""{raw}""#);
        assert_eq!(
            json, expected,
            "hyphenated id should serialize as plain string"
        );
        let deserialized: NodeId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            deserialized, *raw,
            "hyphenated id should survive serde round-trip"
        );
    }
}

#[test]
fn test_node_id_dotted_roundtrip() {
    use finstack_statements::types::NodeId;

    // Dotted IDs (e.g. fin.gross_margin, lease_a.pgi) must survive a serde round-trip
    // unchanged and compare equal to the equivalent &str.
    for raw in &["fin.gross_margin", "lease_a.pgi", "seg.revenue.apac"] {
        let id = NodeId::from(*raw);
        let json = serde_json::to_string(&id).expect("serialize");
        let expected = format!(r#""{raw}""#);
        assert_eq!(json, expected, "dotted id should serialize as plain string");
        let deserialized: NodeId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            deserialized, *raw,
            "dotted id should survive serde round-trip"
        );
    }
}

#[test]
fn test_node_id_hyphenated_builder_accepted() {
    use finstack_core::dates::PeriodId;
    use finstack_statements::builder::ModelBuilder;
    use finstack_statements::types::AmountOrScalar;

    // Hyphenated node IDs must be accepted by the builder and remain retrievable
    // via string lookup after build.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "lease-1",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(500.0))],
        )
        .build()
        .expect("hyphenated node id should be valid");

    assert!(
        model.has_node("lease-1"),
        "model should contain 'lease-1' node"
    );
    let node = model
        .get_node("lease-1")
        .expect("node should be retrievable by hyphenated str");
    assert_eq!(node.node_id.as_str(), "lease-1");
}
