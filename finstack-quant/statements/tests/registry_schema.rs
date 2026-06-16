//! Registry schema integration tests.
#![allow(clippy::expect_used)]

use finstack_quant_statements::registry::{MetricDefinition, MetricRegistry};
use indexmap::IndexMap;

#[test]
fn test_deserialize_metric_registry() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "gross_margin",
                "name": "Gross Margin",
                "formula": "gross_profit / revenue",
                "description": "Margin percentage",
                "category": "margins",
                "unit_type": "percentage",
                "requires": ["gross_profit", "revenue"]
            }
        ]
    }"#;

    let registry: MetricRegistry =
        serde_json::from_str(json).expect("should deserialize valid JSON");
    assert_eq!(registry.namespace, "test");
    assert_eq!(registry.metrics.len(), 1);
    assert_eq!(registry.metrics[0].id, "gross_margin");
}

#[test]
fn test_qualified_id() {
    let metric = MetricDefinition {
        id: "gross_margin".into(),
        name: "Gross Margin".into(),
        formula: "gross_profit / revenue".into(),
        description: None,
        category: None,
        unit_type: None,
        requires: vec![],
        tags: vec![],
        meta: IndexMap::new(),
    };

    assert_eq!(metric.qualified_id("fin"), "fin.gross_margin");
}
