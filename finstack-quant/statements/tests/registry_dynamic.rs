//! Dynamic registry integration tests.
#![allow(clippy::expect_used, clippy::panic)]

use finstack_quant_statements::registry::Registry;

#[test]
fn rejected_documents_preserve_existing_metrics_and_allow_corrected_retry() {
    let anchor = r#"{"namespace":"test","schema_version":1,"metrics":[{"id":"anchor","name":"Anchor","formula":"input"}]}"#;
    let corrected = r#"{"namespace":"test","schema_version":1,"metrics":[{"id":"total","name":"Total","formula":"leaf + anchor"},{"id":"leaf","name":"Leaf","formula":"anchor * 2"}]}"#;
    for rejected_metrics in [
        serde_json::json!([
            {"id":"leaf","name":"Leaf","formula":"anchor * 2"},
            {"id":"broken","name":"Broken","formula":"sum()"}
        ]),
        serde_json::json!([
            {"id":"leaf","name":"Leaf","formula":"anchor * 2"},
            {"id":"leaf","name":"Duplicate","formula":"anchor * 3"}
        ]),
        serde_json::json!([
            {"id":"leaf","name":"Leaf","formula":"total * 2"},
            {"id":"total","name":"Total","formula":"leaf + anchor"}
        ]),
    ] {
        let mut registry = Registry::new();
        registry.load_from_json_str(anchor).expect("load anchor");
        let rejected = serde_json::json!({
            "namespace": "test", "schema_version": 1, "metrics": rejected_metrics
        });
        assert!(registry.load_from_json_str(&rejected.to_string()).is_err());
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry
                .get("test.anchor")
                .expect("anchor survives")
                .definition
                .formula,
            "input"
        );
        registry
            .load_from_json_str(corrected)
            .expect("corrected retry");
        let ids: Vec<_> = registry.all_metrics().map(|(id, _)| id).collect();
        assert_eq!(ids, ["test.anchor", "test.leaf", "test.total"]);
    }
}

#[test]
fn test_load_from_json_str() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "gross_margin",
                "name": "Gross Margin",
                "formula": "gross_profit / revenue"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json)
        .expect("should load valid JSON");

    assert!(registry.has("test.gross_margin"));
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_get_metric() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "gross_margin",
                "name": "Gross Margin",
                "formula": "gross_profit / revenue"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json)
        .expect("should load valid JSON");

    let metric = registry
        .get("test.gross_margin")
        .expect("metric should exist");
    assert_eq!(metric.definition.formula, "gross_profit / revenue");
}

#[test]
fn test_namespace_listing() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "metric1",
                "name": "Metric 1",
                "formula": "a + b"
            },
            {
                "id": "metric2",
                "name": "Metric 2",
                "formula": "c - d"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json)
        .expect("should load valid JSON");

    let test_metrics: Vec<_> = registry.namespace("test").collect();
    assert_eq!(test_metrics.len(), 2);
}

#[test]
fn test_duplicate_metric_error() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "gross_margin",
                "name": "Gross Margin",
                "formula": "gross_profit / revenue"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json)
        .expect("should load valid JSON");

    let result = registry.load_from_json_str(json);
    assert!(result.is_err());
}

#[test]
fn test_namespaces() {
    let json1 = r#"{
        "namespace": "test1",
        "schema_version": 1,
        "metrics": [
            {"id": "m1", "name": "M1", "formula": "a + b"}
        ]
    }"#;

    let json2 = r#"{
        "namespace": "test2",
        "schema_version": 1,
        "metrics": [
            {"id": "m2", "name": "M2", "formula": "c - d"}
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json1)
        .expect("should load valid JSON");
    registry
        .load_from_json_str(json2)
        .expect("should load valid JSON");

    assert!(
        registry
            .all_metrics()
            .any(|(id, _)| id.starts_with("test1."))
    );
    assert!(
        registry
            .all_metrics()
            .any(|(id, _)| id.starts_with("test2."))
    );
}

#[test]
fn test_inter_metric_dependencies() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "gross_profit",
                "name": "Gross Profit",
                "formula": "revenue - cogs"
            },
            {
                "id": "gross_margin",
                "name": "Gross Margin",
                "formula": "gross_profit / revenue"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json)
        .expect("should load valid JSON");

    assert!(registry.has("test.gross_profit"));
    assert!(registry.has("test.gross_margin"));

    let margin = registry
        .get("test.gross_margin")
        .expect("metric should exist");
    assert!(margin.definition.formula.contains("gross_profit"));
}

#[test]
fn test_metric_dependency_order() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "net_margin",
                "name": "Net Margin",
                "formula": "net_income / revenue"
            },
            {
                "id": "net_income",
                "name": "Net Income",
                "formula": "gross_profit - opex"
            },
            {
                "id": "gross_profit",
                "name": "Gross Profit",
                "formula": "revenue - cogs"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    let result = registry.load_from_json_str(json);

    assert!(result.is_ok());
    assert!(registry.has("test.gross_profit"));
    assert!(registry.has("test.net_income"));
    assert!(registry.has("test.net_margin"));
}

#[test]
fn test_circular_dependency_detection() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "metric_a",
                "name": "Metric A",
                "formula": "metric_b + 1"
            },
            {
                "id": "metric_b",
                "name": "Metric B",
                "formula": "metric_a + 1"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    let result = registry.load_from_json_str(json);

    assert!(result.is_err());
    let err_msg = result
        .expect_err("should fail with circular dependency")
        .to_string();
    assert!(err_msg.contains("Circular dependency"));
}

#[test]
fn test_get_dependencies() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "a",
                "name": "A",
                "formula": "x + y"
            },
            {
                "id": "b",
                "name": "B",
                "formula": "a * 2"
            },
            {
                "id": "c",
                "name": "C",
                "formula": "b + a"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json)
        .expect("should load valid JSON");

    let deps = registry
        .get_dependencies("test.c")
        .expect("metric should exist");
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&"test.a".to_string()));
    assert!(deps.contains(&"test.b".to_string()));

    let a_pos = deps
        .iter()
        .position(|d| d == "test.a")
        .expect("test.a should be in dependencies");
    let b_pos = deps
        .iter()
        .position(|d| d == "test.b")
        .expect("test.b should be in dependencies");
    assert!(a_pos < b_pos);
}

#[test]
fn test_transitive_dependencies() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "level1",
                "name": "Level 1",
                "formula": "base_value * 2"
            },
            {
                "id": "level2",
                "name": "Level 2",
                "formula": "level1 + 10"
            },
            {
                "id": "level3",
                "name": "Level 3",
                "formula": "level2 / 2"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json)
        .expect("should load valid JSON");

    let deps = registry
        .get_dependencies("test.level3")
        .expect("metric should exist");
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&"test.level1".to_string()));
    assert!(deps.contains(&"test.level2".to_string()));
}

#[test]
fn test_mixed_dependencies() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "gross_profit",
                "name": "Gross Profit",
                "formula": "revenue - cogs"
            },
            {
                "id": "ebitda",
                "name": "EBITDA",
                "formula": "gross_profit - opex"
            },
            {
                "id": "ebitda_margin",
                "name": "EBITDA Margin",
                "formula": "ebitda / revenue"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry
        .load_from_json_str(json)
        .expect("should load valid JSON");

    assert!(registry.has("test.gross_profit"));
    assert!(registry.has("test.ebitda"));
    assert!(registry.has("test.ebitda_margin"));

    let deps = registry
        .get_dependencies("test.ebitda_margin")
        .expect("metric should exist");
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&"test.gross_profit".to_string()));
    assert!(deps.contains(&"test.ebitda".to_string()));
}

/// A metric in a dotted namespace refers to its own siblings. The builder
/// used to take the text before the first dot as the namespace, so
/// `fin.adj.ebitda_margin` read `fin.ebitda` (0.30) instead of
/// `fin.adj.ebitda` (0.40).
#[test]
fn dotted_namespaces_qualify_references_to_their_own_metrics() {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;

    let adjusted = r#"{"namespace":"fin.adj","schema_version":1,"metrics":[
        {"id":"ebitda","name":"Adjusted EBITDA","formula":"revenue - cogs - opex + addbacks"},
        {"id":"ebitda_margin","name":"Adjusted EBITDA margin","formula":"ebitda / revenue"}]}"#;
    let period = PeriodId::quarter(2025, 1).expect("valid quarter");
    let inputs = [
        ("revenue", 100.0),
        ("cogs", 40.0),
        ("opex", 30.0),
        ("addbacks", 10.0),
    ];

    // One metric at a time, next to the built-in `fin.ebitda`.
    let mut registry = Registry::with_builtins().expect("built-in metrics");
    registry
        .load_from_json_str(adjusted)
        .expect("adjusted metrics");
    let mut builder = ModelBuilder::new("dotted-namespace");
    builder.periods("2025Q1..Q1", None).expect("valid periods");
    for (node, value) in inputs {
        builder.value(node, &[(period, value.into())]);
    }
    builder
        .add_metric_from_registry("fin.ebitda", &registry)
        .expect("built-in metric");
    builder
        .add_metric_from_registry("fin.adj.ebitda_margin", &registry)
        .expect("adjusted metric");
    let model = builder.build().expect("model");
    assert_eq!(
        model
            .get_node("fin.adj.ebitda_margin")
            .and_then(|node| node.formula_text.as_deref()),
        Some("fin.adj.ebitda / revenue")
    );
    let result = Evaluator::new().evaluate(&model).expect("evaluation");
    assert_eq!(result.get("fin.ebitda", &period), Some(30.0));
    assert_eq!(result.get("fin.adj.ebitda_margin", &period), Some(0.4));

    // Every metric at once, from a registry holding only the dotted namespace.
    let mut registry = Registry::new();
    registry
        .load_from_json_str(adjusted)
        .expect("adjusted metrics");
    let mut builder = ModelBuilder::new("dotted-namespace-only");
    builder.periods("2025Q1..Q1", None).expect("valid periods");
    for (node, value) in inputs {
        builder.value(node, &[(period, value.into())]);
    }
    for (qualified_id, _) in registry.all_metrics() {
        builder
            .add_metric_from_registry(qualified_id, &registry)
            .expect("adjusted metric");
    }
    let model = builder.build().expect("model");
    let result = Evaluator::new().evaluate(&model).expect("evaluation");
    assert_eq!(result.get("fin.adj.ebitda_margin", &period), Some(0.4));
}
