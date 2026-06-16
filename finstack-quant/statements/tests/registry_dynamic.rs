//! Dynamic registry integration tests.
#![allow(clippy::expect_used, clippy::panic)]

use finstack_quant_statements::registry::dynamic::Registry;

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
        "metrics": [
            {"id": "m1", "name": "M1", "formula": "a + b"}
        ]
    }"#;

    let json2 = r#"{
        "namespace": "test2",
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

    let namespaces = registry.namespaces();
    assert_eq!(namespaces.len(), 2);
    assert!(namespaces.contains(&"test1"));
    assert!(namespaces.contains(&"test2"));
}

#[test]
fn test_inter_metric_dependencies() {
    let json = r#"{
        "namespace": "test",
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
fn test_get_metric_dependencies() {
    let json = r#"{
        "namespace": "test",
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
        .get_metric_dependencies("test.c")
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
        .get_metric_dependencies("test.level3")
        .expect("metric should exist");
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&"test.level1".to_string()));
    assert!(deps.contains(&"test.level2".to_string()));
}

#[test]
fn test_mixed_dependencies() {
    let json = r#"{
        "namespace": "test",
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
        .get_metric_dependencies("test.ebitda_margin")
        .expect("metric should exist");
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&"test.gross_profit".to_string()));
    assert!(deps.contains(&"test.ebitda".to_string()));
}
