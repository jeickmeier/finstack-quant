//! Integration tests for the dynamic metric registry.

use finstack_quant_statements::prelude::*;
use finstack_quant_statements::registry::{MetricDefinition, MetricRegistry, Registry, UnitType};
use indexmap::IndexMap;

#[test]
fn test_load_builtins() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    // Should have loaded metrics from all four files
    assert!(!registry.is_empty());

    // Check that fin namespace exists
    let namespaces = registry.namespaces();
    assert!(namespaces.contains(&"fin"));
}

#[test]
fn test_builtin_basic_metrics() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    // Test some basic metrics
    assert!(registry.has("fin.gross_profit"));
    assert!(registry.has("fin.operating_income"));
    assert!(registry.has("fin.ebitda"));
    assert!(registry.has("fin.net_income"));

    let gross_profit = registry.get("fin.gross_profit").unwrap();
    assert_eq!(gross_profit.definition.formula, "revenue - cogs");

    let ebit = registry.get("fin.ebit").unwrap();
    assert_eq!(
        ebit.definition.formula,
        "ebitda - depreciation - amortization"
    );
}

#[test]
fn test_builtin_margin_metrics() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    // Test margin metrics
    assert!(registry.has("fin.gross_margin"));
    assert!(registry.has("fin.operating_margin"));
    assert!(registry.has("fin.ebitda_margin"));
    assert!(registry.has("fin.net_margin"));

    let gross_margin = registry.get("fin.gross_margin").unwrap();
    assert_eq!(gross_margin.definition.formula, "gross_profit / revenue");
    assert_eq!(
        gross_margin.definition.unit_type,
        Some(UnitType::Percentage)
    );
}

#[test]
fn test_builtin_return_metrics() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    // Test return metrics
    assert!(registry.has("fin.roe"));
    assert!(registry.has("fin.roa"));
    assert!(registry.has("fin.roic"));

    let roe = registry.get("fin.roe").unwrap();
    assert_eq!(
        roe.definition.formula,
        "(revenue - cogs - opex - interest_expense - taxes) / total_equity"
    );
}

#[test]
fn test_builtin_leverage_metrics() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    // Test leverage metrics
    assert!(registry.has("fin.debt_to_equity"));
    assert!(registry.has("fin.debt_to_ebitda"));
    assert!(registry.has("fin.interest_coverage"));

    let interest_coverage = registry.get("fin.interest_coverage").unwrap();
    assert_eq!(
        interest_coverage.definition.formula,
        "ttm(ebitda) / ttm(interest_expense)"
    );
}

#[test]
fn builtin_credit_ratios_use_ttm_flows() {
    let registry = Registry::with_builtins().unwrap();

    let debt_to_ebitda = registry.get("fin.debt_to_ebitda").unwrap();
    assert_eq!(
        debt_to_ebitda.definition.formula,
        "total_debt / ttm(ebitda)"
    );

    let debt_service_coverage = registry.get("fin.debt_service_coverage").unwrap();
    assert_eq!(
        debt_service_coverage.definition.formula,
        "ttm(ebitda) / ttm(interest_expense + principal_payment)"
    );
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
                "formula": "gross_profit / revenue",
                "description": "Margin percentage",
                "category": "margins",
                "unit_type": "percentage"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry.load_from_json_str(json).unwrap();

    assert!(registry.has("test.gross_margin"));
    assert_eq!(registry.len(), 1);
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
    registry.load_from_json_str(json).unwrap();

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
    registry.load_from_json_str(json).unwrap();

    // Try to load again
    let result = registry.load_from_json_str(json);
    assert!(result.is_err());
}

#[test]
fn test_invalid_formula_error() {
    let json = r#"{
        "namespace": "test",
        "metrics": [
            {
                "id": "invalid",
                "name": "Invalid",
                "formula": "a + + b"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    let result = registry.load_from_json_str(json);
    assert!(result.is_err());
}

#[test]
fn test_compile_time_formula_error() {
    let json = r#"{
        "namespace": "test",
        "metrics": [
            {
                "id": "invalid",
                "name": "Invalid",
                "formula": "sum()"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    let result = registry.load_from_json_str(json);
    assert!(result.is_err());
}

#[test]
fn test_multiple_namespaces() {
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
    registry.load_from_json_str(json1).unwrap();
    registry.load_from_json_str(json2).unwrap();

    let namespaces = registry.namespaces();
    assert_eq!(namespaces.len(), 2);
    assert!(namespaces.contains(&"test1"));
    assert!(namespaces.contains(&"test2"));
}

#[test]
fn test_model_builder_with_builtin_metrics() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
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
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        )
        .value(
            "cogs",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(66_000.0)),
            ],
        )
        .with_builtin_metrics()
        .unwrap()
        .build()
        .unwrap();

    // Should have revenue, cogs, and all built-in metrics
    assert!(model.has_node("revenue"));
    assert!(model.has_node("cogs"));
    assert!(model.has_node("fin.gross_profit"));
    assert!(model.has_node("fin.gross_margin"));
}

#[test]
fn test_evaluate_model_with_select_metrics() {
    // Don't load ALL metrics with .with_builtin_metrics() - just add the ones we need
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
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
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        )
        .value(
            "cogs",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(66_000.0)),
            ],
        )
        .add_metric_from_registry("fin.gross_profit", &registry)
        .unwrap()
        .add_metric_from_registry("fin.gross_margin", &registry)
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check that fin.gross_profit was calculated correctly
    let q1_gross_profit = results
        .get("fin.gross_profit", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert_eq!(q1_gross_profit, 40_000.0); // 100,000 - 60,000

    let q2_gross_profit = results
        .get("fin.gross_profit", &PeriodId::quarter(2025, 2))
        .unwrap();
    assert_eq!(q2_gross_profit, 44_000.0); // 110,000 - 66,000

    // Check that fin.gross_margin was calculated correctly
    let q1_gross_margin = results
        .get("fin.gross_margin", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert!((q1_gross_margin - 0.4).abs() < 0.0001); // 40,000 / 100,000 = 0.4

    let q2_gross_margin = results
        .get("fin.gross_margin", &PeriodId::quarter(2025, 2))
        .unwrap();
    assert!((q2_gross_margin - 0.4).abs() < 0.0001); // 44,000 / 110,000 = 0.4
}

#[test]
fn test_add_metric_from_registry() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
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
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        )
        .value(
            "cogs",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(66_000.0)),
            ],
        )
        .add_metric_from_registry("fin.gross_profit", &registry)
        .unwrap()
        .add_metric_from_registry("fin.gross_margin", &registry)
        .unwrap()
        .build()
        .unwrap();

    // Should only have revenue, cogs, and the two explicitly added metrics
    assert!(model.has_node("revenue"));
    assert!(model.has_node("cogs"));
    assert!(model.has_node("fin.gross_profit"));
    assert!(model.has_node("fin.gross_margin"));
    assert!(!model.has_node("fin.operating_income")); // Not added
}

#[test]
fn test_metric_definition_qualified_id() {
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

#[test]
fn test_metric_registry_serialization() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
        "metrics": [
            {
                "id": "test_metric",
                "name": "Test Metric",
                "formula": "a + b",
                "description": "A test metric",
                "category": "test",
                "unit_type": "ratio",
                "requires": ["a", "b"],
                "tags": ["test"]
            }
        ]
    }"#;

    let registry: MetricRegistry = serde_json::from_str(json).unwrap();
    assert_eq!(registry.namespace, "test");
    assert_eq!(registry.schema_version, 1);
    assert_eq!(registry.metrics.len(), 1);

    let metric = &registry.metrics[0];
    assert_eq!(metric.id, "test_metric");
    assert_eq!(metric.name, "Test Metric");
    assert_eq!(metric.formula, "a + b");
    assert_eq!(metric.unit_type, Some(UnitType::Ratio));
}

#[test]
fn test_complete_pl_model_with_select_registry_metrics() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    let model = ModelBuilder::new("P&L Model")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        )
        .value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(600_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(660_000.0),
                ),
            ],
        )
        .value(
            "opex",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(220_000.0),
                ),
            ],
        )
        .value(
            "depreciation",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(50_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(50_000.0)),
            ],
        )
        .value(
            "amortization",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(10_000.0)),
            ],
        )
        // Only add the metrics we can calculate with the given inputs
        .add_metric_from_registry("fin.gross_profit", &registry)
        .unwrap()
        .add_metric_from_registry("fin.operating_income", &registry)
        .unwrap()
        .add_metric_from_registry("fin.ebitda", &registry)
        .unwrap()
        .add_metric_from_registry("fin.gross_margin", &registry)
        .unwrap()
        .add_metric_from_registry("fin.operating_margin", &registry)
        .unwrap()
        .add_metric_from_registry("fin.ebitda_margin", &registry)
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Test Q1 calculations
    let q1 = PeriodId::quarter(2025, 1);
    assert_eq!(results.get("fin.gross_profit", &q1).unwrap(), 400_000.0);
    assert_eq!(results.get("fin.operating_income", &q1).unwrap(), 200_000.0);
    assert_eq!(results.get("fin.ebitda", &q1).unwrap(), 260_000.0);
    assert!((results.get("fin.gross_margin", &q1).unwrap() - 0.4).abs() < 0.0001);
    assert!((results.get("fin.operating_margin", &q1).unwrap() - 0.2).abs() < 0.0001);

    // Test Q2 calculations
    let q2 = PeriodId::quarter(2025, 2);
    assert_eq!(results.get("fin.gross_profit", &q2).unwrap(), 440_000.0);
    assert_eq!(results.get("fin.operating_income", &q2).unwrap(), 220_000.0);
    assert_eq!(results.get("fin.ebitda", &q2).unwrap(), 280_000.0);
    assert!((results.get("fin.gross_margin", &q2).unwrap() - 0.4).abs() < 0.0001);
    assert!((results.get("fin.operating_margin", &q2).unwrap() - 0.2).abs() < 0.0001);
}

#[test]
fn test_inter_metric_dependencies_in_model() {
    // Test that metrics can reference other metrics
    let json = r#"{
        "namespace": "custom",
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
    registry.load_from_json_str(json).unwrap();

    // Build a model and add only the top-level metrics
    // Dependencies should be automatically added
    let model = ModelBuilder::new("Test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        )
        .value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(600_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(660_000.0),
                ),
            ],
        )
        .value(
            "opex",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(220_000.0),
                ),
            ],
        )
        // Only add ebitda_margin - should automatically add gross_profit and ebitda
        .add_metric_from_registry("custom.ebitda_margin", &registry)
        .unwrap()
        .build()
        .unwrap();

    // Evaluate the model
    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Verify that all dependencies were added and calculated correctly
    let q1 = PeriodId::quarter(2025, 1);
    assert_eq!(results.get("custom.gross_profit", &q1).unwrap(), 400_000.0);
    assert_eq!(results.get("custom.ebitda", &q1).unwrap(), 200_000.0);
    assert!((results.get("custom.ebitda_margin", &q1).unwrap() - 0.2).abs() < 0.0001);

    let q2 = PeriodId::quarter(2025, 2);
    assert_eq!(results.get("custom.gross_profit", &q2).unwrap(), 440_000.0);
    assert_eq!(results.get("custom.ebitda", &q2).unwrap(), 220_000.0);
    assert!((results.get("custom.ebitda_margin", &q2).unwrap() - 0.2).abs() < 0.0001);
}

#[test]
fn test_deep_dependency_chain() {
    // Test a deep chain of metric dependencies
    let json = r#"{
        "namespace": "chain",
        "metrics": [
            {
                "id": "level1",
                "name": "Level 1",
                "formula": "base * 2"
            },
            {
                "id": "level2",
                "name": "Level 2",
                "formula": "level1 + 10"
            },
            {
                "id": "level3",
                "name": "Level 3",
                "formula": "level2 * 1.5"
            },
            {
                "id": "level4",
                "name": "Level 4",
                "formula": "level3 / 2"
            }
        ]
    }"#;

    let mut registry = Registry::new();
    registry.load_from_json_str(json).unwrap();

    let model = ModelBuilder::new("Test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "base",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        // Only add level4 - should automatically add all dependencies
        .add_metric_from_registry("chain.level4", &registry)
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    // base = 100
    // level1 = 100 * 2 = 200
    // level2 = 200 + 10 = 210
    // level3 = 210 * 1.5 = 315
    // level4 = 315 / 2 = 157.5
    assert_eq!(results.get("chain.level1", &q1).unwrap(), 200.0);
    assert_eq!(results.get("chain.level2", &q1).unwrap(), 210.0);
    assert_eq!(results.get("chain.level3", &q1).unwrap(), 315.0);
    assert_eq!(results.get("chain.level4", &q1).unwrap(), 157.5);
}

// --- Parity: with_builtin_metrics vs add_metric_from_registry ---

#[test]
fn parity_add_metric_matches_with_builtin_metrics_for_same_nodes() {
    // Provide enough leaf nodes so with_builtin_metrics() can compute all registry metrics.
    let zero_periods = |v: f64| {
        [
            (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(v)),
            (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(v)),
        ]
    };
    let base_model_fn = || {
        ModelBuilder::new("base")
            .periods("2025Q1..Q2", None)
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
                        AmountOrScalar::scalar(110_000.0),
                    ),
                ],
            )
            .value(
                "cogs",
                &[
                    (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60_000.0)),
                    (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(66_000.0)),
                ],
            )
            .value("opex", &zero_periods(0.0))
            .value("interest_expense", &zero_periods(0.0))
            .value("tax_expense", &zero_periods(0.0))
            .value("depreciation", &zero_periods(0.0))
            .value("amortization", &zero_periods(0.0))
            .value("principal_payment", &zero_periods(0.0))
            .value("total_assets", &zero_periods(1.0))
            .value("total_debt", &zero_periods(0.0))
            .value("total_equity", &zero_periods(1.0))
            .value("taxes", &zero_periods(0.0))
            .value("current_liabilities", &zero_periods(1.0))
    };

    // Path 1: with_builtin_metrics() — adds all registry metrics at once
    let model_bulk = base_model_fn()
        .with_builtin_metrics()
        .unwrap()
        .build()
        .unwrap();

    // Path 2: add_metric_from_registry("fin.gross_profit") — adds just one specific metric
    let registry = Registry::with_builtins().unwrap();
    let model_select = base_model_fn()
        .add_metric_from_registry("fin.gross_profit", &registry)
        .unwrap()
        .build()
        .unwrap();

    // Both paths produce a fin.gross_profit node
    assert!(model_bulk.has_node("fin.gross_profit"));
    assert!(model_select.has_node("fin.gross_profit"));

    // Both evaluate to the same fin.gross_profit value
    let mut eval = Evaluator::new();
    let r_bulk = eval.evaluate(&model_bulk).unwrap();
    let r_select = eval.evaluate(&model_select).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    let bulk_val = r_bulk.get("fin.gross_profit", &q1).unwrap();
    let select_val = r_select.get("fin.gross_profit", &q1).unwrap();

    assert!(
        (bulk_val - select_val).abs() < 1e-9,
        "with_builtin_metrics and add_metric_from_registry must produce the same gross_profit value: {} vs {}",
        bulk_val,
        select_val
    );
}

#[test]
fn registry_with_builtins_matches_new_then_load() {
    // Parity check between the convenience `Registry::with_builtins` and the
    // explicit two-step `Registry::new(); load_builtins()` pattern.
    let base_model_fn = || {
        ModelBuilder::new("base")
            .periods("2025Q1..Q2", None)
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
                        AmountOrScalar::scalar(110_000.0),
                    ),
                ],
            )
            .value(
                "cogs",
                &[
                    (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60_000.0)),
                    (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(66_000.0)),
                ],
            )
    };

    // Path 1: Registry::with_builtins()
    let registry_a = Registry::with_builtins().unwrap();
    let model_a = base_model_fn()
        .add_metric_from_registry("fin.gross_profit", &registry_a)
        .unwrap()
        .build()
        .unwrap();

    // Path 2: explicit registry then load_builtins
    let mut registry_b = Registry::new();
    registry_b.load_builtins().unwrap();
    let model_b = base_model_fn()
        .add_metric_from_registry("fin.gross_profit", &registry_b)
        .unwrap()
        .build()
        .unwrap();

    let mut eval = Evaluator::new();
    let r_a = eval.evaluate(&model_a).unwrap();
    let r_b = eval.evaluate(&model_b).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    let v1 = r_a.get("fin.gross_profit", &q1).unwrap();
    let v2 = r_b.get("fin.gross_profit", &q1).unwrap();

    assert!(
        (v1 - v2).abs() < 1e-9,
        "Registry::with_builtins and Registry::new+load_builtins must be equivalent: {} vs {}",
        v1,
        v2
    );
}

#[test]
fn parity_with_builtin_metrics_qualifies_intra_namespace_refs() {
    // Verify that namespace-qualified refs like fin.gross_profit work in formulas
    // when loaded via with_builtin_metrics
    let model = ModelBuilder::new("namespace-qual-test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(220_000.0),
                ),
            ],
        )
        .value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        )
        .value(
            "opex",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "interest_expense",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "tax_expense",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "depreciation",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "amortization",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "principal_payment",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "total_assets",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(1.0)),
            ],
        )
        .value(
            "total_debt",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "total_equity",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(1.0)),
            ],
        )
        .value(
            "taxes",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "current_liabilities",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(1.0)),
            ],
        )
        .with_builtin_metrics()
        .unwrap()
        .build()
        .unwrap();

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    // fin.gross_margin depends on gross_profit / revenue; both must resolve correctly
    let margin = results.get("fin.gross_margin", &q1).unwrap();
    assert!(
        (margin - 0.5).abs() < 1e-9,
        "gross_margin = gross_profit/revenue = 100k/200k = 0.5, got {}",
        margin
    );
}
