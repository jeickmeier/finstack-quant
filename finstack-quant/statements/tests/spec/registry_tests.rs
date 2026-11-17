//! Integration tests for the dynamic metric registry.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::registry::{MetricDefinition, MetricRegistry, Registry, UnitType};
use finstack_quant_statements::types::AmountOrScalar;
use indexmap::IndexMap;

#[test]
fn test_load_builtins() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    // Should have loaded metrics from all four files
    assert!(!registry.is_empty());

    // Check that fin namespace exists
    assert!(registry.all_metrics().any(|(id, _)| id.starts_with("fin.")));
}

/// STATEMENTS-CONTRACTS-007: the accepted six-row earnings table, four
/// margins and two cost ratios.
#[test]
fn builtin_earnings_and_margins_follow_the_reported_cost_table() {
    let registry = Registry::with_builtins().unwrap();
    for (id, formula) in [
        ("fin.gross_profit", "revenue - cogs"),
        ("fin.operating_income", "gross_profit - opex"),
        ("fin.ebit", "operating_income"),
        ("fin.ebitda", "ebit + depreciation + amortization"),
        ("fin.ebt", "ebit - interest_expense"),
        ("fin.net_income", "ebt - tax_expense"),
        ("fin.gross_margin", "ratio(gross_profit, revenue)"),
        ("fin.operating_margin", "ratio(operating_income, revenue)"),
        ("fin.ebitda_margin", "ratio(ebitda, revenue)"),
        ("fin.net_margin", "ratio(net_income, revenue)"),
        ("fin.cogs_as_pct_revenue", "ratio(cogs, revenue)"),
        ("fin.opex_as_pct_revenue", "ratio(opex, revenue)"),
    ] {
        assert_eq!(
            registry.get(id).unwrap().definition.formula,
            formula,
            "{id}"
        );
    }
    assert_eq!(
        registry
            .get("fin.gross_margin")
            .unwrap()
            .definition
            .unit_type,
        Some(UnitType::Percentage)
    );
    // Each row references the row above it, so the closure walks the table.
    assert_eq!(
        registry.get_dependencies("fin.net_margin").unwrap(),
        [
            "fin.gross_profit",
            "fin.operating_income",
            "fin.ebit",
            "fin.ebt",
            "fin.net_income"
        ]
    );
    // The leverage ratios read the bundled EBITDA, not a model input.
    assert!(
        registry
            .get_dependencies("fin.debt_to_ebitda")
            .unwrap()
            .contains(&"fin.ebitda".to_string())
    );
}

/// The return ratios read the same reported-cost lines: D&A sits inside
/// `cogs` and `opex` and is not subtracted again.
#[test]
fn builtin_return_metrics_do_not_subtract_da_twice() {
    let registry = Registry::with_builtins().unwrap();
    for (id, formula) in [
        (
            "fin.roe",
            "(revenue - cogs - opex - interest_expense - taxes) / total_equity",
        ),
        (
            "fin.roa",
            "(revenue - cogs - opex - interest_expense - taxes) / total_assets",
        ),
        // ROIC uses NOPAT (EBIT less taxes), not pre-tax EBIT.
        (
            "fin.roic",
            "(revenue - cogs - opex - taxes) / (total_debt + total_equity)",
        ),
        (
            "fin.roce",
            "(revenue - cogs - opex) / (total_assets - current_liabilities)",
        ),
    ] {
        assert_eq!(
            registry.get(id).unwrap().definition.formula,
            formula,
            "{id}"
        );
    }
}

/// One quarter of the bundled income statement with the named inputs.
fn income_statement(
    inputs: &[(&str, Option<f64>)],
) -> finstack_quant_statements::evaluator::StatementResult {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;

    let registry = Registry::with_builtins().unwrap();
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let mut builder = ModelBuilder::new("income-statement");
    builder.periods("2025Q1..Q1", None).unwrap();
    for (node, value) in inputs {
        let observations: Vec<_> = value.iter().map(|value| (q1, *value)).collect();
        builder.value_scalar(*node, &observations);
    }
    for metric in [
        "fin.ebitda_margin",
        "fin.net_margin",
        "fin.gross_margin",
        "fin.operating_margin",
        "fin.cogs_as_pct_revenue",
        "fin.opex_as_pct_revenue",
    ] {
        builder.add_metric_from_registry(metric, &registry).unwrap();
    }
    Evaluator::new()
        .evaluate(&builder.build().unwrap())
        .unwrap()
}

/// An income statement worked by hand. Reported COGS of 400 includes 20 of
/// depreciation; reported OPEX of 300 includes 30 of depreciation and 30 of
/// amortization. The included D&A of 80 is added back once, in EBITDA.
#[test]
fn reported_costs_include_da_and_ebitda_adds_it_back_once() {
    use finstack_quant_core::dates::PeriodId;
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let result = income_statement(&[
        ("revenue", Some(1_000.0)),
        ("cogs", Some(400.0)),
        ("opex", Some(300.0)),
        ("depreciation", Some(50.0)),
        ("amortization", Some(30.0)),
        ("interest_expense", Some(40.0)),
        ("tax_expense", Some(65.0)),
    ]);
    for (node, expected) in [
        ("fin.gross_profit", 600.0),
        ("fin.operating_income", 300.0),
        ("fin.ebit", 300.0),
        ("fin.ebitda", 380.0),
        ("fin.ebt", 260.0),
        ("fin.net_income", 195.0),
        ("fin.gross_margin", 0.6),
        ("fin.operating_margin", 0.3),
        ("fin.ebitda_margin", 0.38),
        ("fin.net_margin", 0.195),
        ("fin.cogs_as_pct_revenue", 0.4),
        ("fin.opex_as_pct_revenue", 0.3),
    ] {
        assert_eq!(result.get(node, &q1), Some(expected), "{node}");
    }
}

/// An expense credit keeps its negative sign, a known zero is a zero, and a
/// loss keeps its sign in a margin on positive revenue.
#[test]
fn expense_credits_and_losses_keep_their_sign() {
    use finstack_quant_core::dates::PeriodId;
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let result = income_statement(&[
        ("revenue", Some(1_000.0)),
        ("cogs", Some(1_200.0)),
        // A reversal larger than the period's charges.
        ("opex", Some(-50.0)),
        ("depreciation", Some(0.0)),
        ("amortization", Some(0.0)),
        ("interest_expense", Some(0.0)),
        ("tax_expense", Some(-30.0)),
    ]);
    for (node, expected) in [
        ("fin.gross_profit", -200.0),
        ("fin.operating_income", -150.0),
        ("fin.ebitda", -150.0),
        ("fin.net_income", -120.0),
        ("fin.gross_margin", -0.2),
        ("fin.net_margin", -0.12),
        ("fin.opex_as_pct_revenue", -0.05),
    ] {
        assert_eq!(result.get(node, &q1), Some(expected), "{node}");
    }
}

/// A margin is unavailable, with a reason, when revenue is missing, zero or
/// negative; a missing dependency makes the affected earnings unavailable.
#[test]
fn margins_are_unavailable_without_positive_revenue() {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::evaluator::{CellStatus, UnavailableReason};
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let statement = |revenue: Option<f64>, depreciation: Option<f64>| {
        income_statement(&[
            ("revenue", revenue),
            ("cogs", Some(400.0)),
            ("opex", Some(300.0)),
            ("depreciation", depreciation),
            ("amortization", Some(30.0)),
            ("interest_expense", Some(40.0)),
            ("tax_expense", Some(65.0)),
        ])
    };
    let reason = |result: &finstack_quant_statements::evaluator::StatementResult, node: &str| {
        let cell = result.get_cell(node, &q1).expect("cell");
        assert_eq!(cell.status, CellStatus::Unavailable, "{node}");
        cell.reason
    };
    for (revenue, expected) in [
        (Some(0.0), UnavailableReason::UndefinedBase),
        (Some(-10.0), UnavailableReason::UndefinedBase),
        (None, UnavailableReason::MissingInput),
    ] {
        let result = statement(revenue, Some(50.0));
        for margin in [
            "fin.gross_margin",
            "fin.operating_margin",
            "fin.ebitda_margin",
            "fin.net_margin",
            "fin.cogs_as_pct_revenue",
            "fin.opex_as_pct_revenue",
        ] {
            assert_eq!(
                reason(&result, margin),
                Some(expected),
                "{margin} at {revenue:?}"
            );
        }
    }
    // Zero revenue still gives the earnings lines themselves.
    assert_eq!(
        statement(Some(0.0), Some(50.0)).get("fin.operating_income", &q1),
        Some(-700.0)
    );
    // A missing add-back leaves EBITDA and its margin unavailable, and
    // nothing else.
    let result = statement(Some(1_000.0), None);
    for node in ["fin.ebitda", "fin.ebitda_margin"] {
        assert_eq!(reason(&result, node), Some(UnavailableReason::MissingInput));
    }
    assert_eq!(result.get("fin.net_margin", &q1), Some(0.195));
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
    registry.load_from_json_str(json).unwrap();

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
    registry.load_from_json_str(json).unwrap();

    // Try to load again
    let result = registry.load_from_json_str(json);
    assert!(result.is_err());
}

#[test]
fn test_invalid_formula_error() {
    let json = r#"{
        "namespace": "test",
        "schema_version": 1,
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
        "schema_version": 1,
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
    registry.load_from_json_str(json1).unwrap();
    registry.load_from_json_str(json2).unwrap();

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
fn test_model_builder_with_builtin_metrics() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(60_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(66_000.0),
                ),
            ],
        );
        model_builder
    };

    // The whole catalogue needs inputs this model does not have, and a
    // runnable model resolves every reference.
    let mut whole_catalogue = ModelBuilder::from_spec({
        let mut selected = ModelBuilder::from_spec(model.build().unwrap()).unwrap();
        let registry = finstack_quant_statements::registry::Registry::with_builtins().unwrap();
        selected
            .add_metric_from_registry("fin.gross_margin", &registry)
            .unwrap();
        let selected = selected.build().unwrap();
        assert!(selected.has_node("fin.gross_profit"));
        assert!(selected.has_node("fin.gross_margin"));
        selected
    })
    .unwrap();
    whole_catalogue.with_builtin_metrics().unwrap();
    let error = whole_catalogue
        .build()
        .expect_err("catalogue metrics with missing inputs leave unresolved references");
    assert!(error.to_string().contains("Unknown identifier"), "{error}");
}

#[test]
fn test_evaluate_model_with_select_metrics() {
    // Don't load ALL metrics with .with_builtin_metrics() - just add the ones we need
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(60_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(66_000.0),
                ),
            ],
        );
        model_builder
            .add_metric_from_registry("fin.gross_profit", &registry)
            .unwrap();
        model_builder
            .add_metric_from_registry("fin.gross_margin", &registry)
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check that fin.gross_profit was calculated correctly
    let q1_gross_profit = results
        .get(
            "fin.gross_profit",
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
        )
        .unwrap();
    assert_eq!(q1_gross_profit, 40_000.0); // 100,000 - 60,000

    let q2_gross_profit = results
        .get(
            "fin.gross_profit",
            &PeriodId::quarter(2025, 2).expect("valid period fixture"),
        )
        .unwrap();
    assert_eq!(q2_gross_profit, 44_000.0); // 110,000 - 66,000

    // Check that fin.gross_margin was calculated correctly
    let q1_gross_margin = results
        .get(
            "fin.gross_margin",
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
        )
        .unwrap();
    assert!((q1_gross_margin - 0.4).abs() < 0.0001); // 40,000 / 100,000 = 0.4

    let q2_gross_margin = results
        .get(
            "fin.gross_margin",
            &PeriodId::quarter(2025, 2).expect("valid period fixture"),
        )
        .unwrap();
    assert!((q2_gross_margin - 0.4).abs() < 0.0001); // 44,000 / 110,000 = 0.4
}

#[test]
fn test_add_metric_from_registry() {
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(60_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(66_000.0),
                ),
            ],
        );
        model_builder
            .add_metric_from_registry("fin.gross_profit", &registry)
            .unwrap();
        model_builder
            .add_metric_from_registry("fin.gross_margin", &registry)
            .unwrap();
        model_builder.build().unwrap()
    };

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
        rollup: None,
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
    assert_eq!(u32::from(registry.schema_version), 1);
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

    let model = {
        let mut model_builder = ModelBuilder::new("P&L Model");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(600_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(660_000.0),
                ),
            ],
        );
        model_builder.value(
            "opex",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(220_000.0),
                ),
            ],
        );
        model_builder.value(
            "depreciation",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(50_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(50_000.0),
                ),
            ],
        );
        model_builder.value(
            "amortization",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(10_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(10_000.0),
                ),
            ],
        );
        model_builder
            // Only add the metrics we can calculate with the given inputs
            .add_metric_from_registry("fin.gross_profit", &registry)
            .unwrap();
        model_builder
            .add_metric_from_registry("fin.operating_income", &registry)
            .unwrap();
        model_builder
            .add_metric_from_registry("fin.ebitda", &registry)
            .unwrap();
        model_builder
            .add_metric_from_registry("fin.gross_margin", &registry)
            .unwrap();
        model_builder
            .add_metric_from_registry("fin.operating_margin", &registry)
            .unwrap();
        model_builder
            .add_metric_from_registry("fin.ebitda_margin", &registry)
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Test Q1 calculations. Convention: opex excludes D&A, so
    // EBITDA = 1,000k − 600k − 200k = 200k and
    // operating income (EBIT) = EBITDA − 60k D&A = 140k.
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    assert_eq!(results.get("fin.gross_profit", &q1).unwrap(), 400_000.0);
    // Reported OPEX of 200k includes the 60k of D&A, so operating income is
    // 400k − 200k and EBITDA adds the 60k back once.
    assert_eq!(results.get("fin.operating_income", &q1).unwrap(), 200_000.0);
    assert_eq!(results.get("fin.ebitda", &q1).unwrap(), 260_000.0);
    assert!((results.get("fin.gross_margin", &q1).unwrap() - 0.4).abs() < 0.0001);
    assert!((results.get("fin.operating_margin", &q1).unwrap() - 0.2).abs() < 0.0001);

    // Q2: operating income = 440k − 220k = 220k, EBITDA = 220k + 60k = 280k.
    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
    assert_eq!(results.get("fin.gross_profit", &q2).unwrap(), 440_000.0);
    assert_eq!(results.get("fin.operating_income", &q2).unwrap(), 220_000.0);
    assert_eq!(results.get("fin.ebitda", &q2).unwrap(), 280_000.0);
    assert!((results.get("fin.gross_margin", &q2).unwrap() - 0.4).abs() < 0.0001);
    assert!(
        (results.get("fin.operating_margin", &q2).unwrap() - 220_000.0 / 1_100_000.0).abs()
            < 0.0001
    );
}

#[test]
fn test_inter_metric_dependencies_in_model() {
    // Test that metrics can reference other metrics
    let json = r#"{
        "namespace": "custom",
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
    let model = {
        let mut model_builder = ModelBuilder::new("Test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(600_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(660_000.0),
                ),
            ],
        );
        model_builder.value(
            "opex",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(220_000.0),
                ),
            ],
        );
        model_builder
            // Only add ebitda_margin - should automatically add gross_profit and ebitda
            .add_metric_from_registry("custom.ebitda_margin", &registry)
            .unwrap();
        model_builder.build().unwrap()
    };

    // Evaluate the model
    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Verify that all dependencies were added and calculated correctly
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    assert_eq!(results.get("custom.gross_profit", &q1).unwrap(), 400_000.0);
    assert_eq!(results.get("custom.ebitda", &q1).unwrap(), 200_000.0);
    assert!((results.get("custom.ebitda_margin", &q1).unwrap() - 0.2).abs() < 0.0001);

    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
    assert_eq!(results.get("custom.gross_profit", &q2).unwrap(), 440_000.0);
    assert_eq!(results.get("custom.ebitda", &q2).unwrap(), 220_000.0);
    assert!((results.get("custom.ebitda_margin", &q2).unwrap() - 0.2).abs() < 0.0001);
}

#[test]
fn test_deep_dependency_chain() {
    // Test a deep chain of metric dependencies
    let json = r#"{
        "namespace": "chain",
        "schema_version": 1,
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

    let model = {
        let mut model_builder = ModelBuilder::new("Test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "base",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder
            // Only add level4 - should automatically add all dependencies
            .add_metric_from_registry("chain.level4", &registry)
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
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
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(v),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(v),
            ),
        ]
    };
    let base_model_fn = || {
        let mut model_builder = ModelBuilder::new("base");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(60_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(66_000.0),
                ),
            ],
        );
        model_builder.value("opex", &zero_periods(0.0));
        model_builder.value("interest_expense", &zero_periods(0.0));
        model_builder.value("tax_expense", &zero_periods(0.0));
        model_builder.value("depreciation", &zero_periods(0.0));
        model_builder.value("amortization", &zero_periods(0.0));
        model_builder.value("principal_payment", &zero_periods(0.0));
        model_builder.value("total_assets", &zero_periods(2.0));
        model_builder.value("total_debt", &zero_periods(0.0));
        model_builder.value("total_equity", &zero_periods(1.0));
        model_builder.value("taxes", &zero_periods(0.0));
        model_builder.value("current_liabilities", &zero_periods(1.0));
        model_builder
    };

    // Path 1: with_builtin_metrics() — adds all registry metrics at once
    let model_bulk = {
        let mut model_builder_1 = base_model_fn();
        model_builder_1.with_builtin_metrics().unwrap();
        model_builder_1.build().unwrap()
    };

    // Path 2: add_metric_from_registry("fin.gross_profit") — adds just one specific metric
    let registry = Registry::with_builtins().unwrap();
    let model_select = {
        let mut model_builder_1 = base_model_fn();
        model_builder_1
            .add_metric_from_registry("fin.gross_profit", &registry)
            .unwrap();
        model_builder_1.build().unwrap()
    };

    // Both paths produce a fin.gross_profit node
    assert!(model_bulk.has_node("fin.gross_profit"));
    assert!(model_select.has_node("fin.gross_profit"));

    // Both evaluate to the same fin.gross_profit value
    let mut eval = Evaluator::new();
    let r_bulk = eval.evaluate(&model_bulk).unwrap();
    let r_select = eval.evaluate(&model_select).unwrap();

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
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
        let mut model_builder = ModelBuilder::new("base");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(60_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(66_000.0),
                ),
            ],
        );
        model_builder
    };

    // Path 1: Registry::with_builtins()
    let registry_a = Registry::with_builtins().unwrap();
    let model_a = {
        let mut model_builder_1 = base_model_fn();
        model_builder_1
            .add_metric_from_registry("fin.gross_profit", &registry_a)
            .unwrap();
        model_builder_1.build().unwrap()
    };

    // Path 2: explicit registry then load_builtins
    let mut registry_b = Registry::new();
    registry_b.load_builtins().unwrap();
    let model_b = {
        let mut model_builder_1 = base_model_fn();
        model_builder_1
            .add_metric_from_registry("fin.gross_profit", &registry_b)
            .unwrap();
        model_builder_1.build().unwrap()
    };

    let mut eval = Evaluator::new();
    let r_a = eval.evaluate(&model_a).unwrap();
    let r_b = eval.evaluate(&model_b).unwrap();

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
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
    let model = {
        let mut model_builder = ModelBuilder::new("namespace-qual-test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(220_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder.value(
            "opex",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        );
        model_builder.value(
            "interest_expense",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        );
        model_builder.value(
            "tax_expense",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        );
        model_builder.value(
            "depreciation",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        );
        model_builder.value(
            "amortization",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        );
        model_builder.value(
            "principal_payment",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        );
        model_builder.value(
            "total_assets",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(2.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(2.0),
                ),
            ],
        );
        model_builder.value(
            "total_debt",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        );
        model_builder.value(
            "total_equity",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1.0),
                ),
            ],
        );
        model_builder.value(
            "taxes",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
            ],
        );
        model_builder.value(
            "current_liabilities",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1.0),
                ),
            ],
        );
        model_builder.with_builtin_metrics().unwrap();
        model_builder.build().unwrap()
    };

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    // fin.gross_margin depends on gross_profit / revenue; both must resolve correctly
    let margin = results.get("fin.gross_margin", &q1).unwrap();
    assert!(
        (margin - 0.5).abs() < 1e-9,
        "gross_margin = gross_profit/revenue = 100k/200k = 0.5, got {}",
        margin
    );
}
