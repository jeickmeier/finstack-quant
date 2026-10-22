//! Evaluator tests for Phase 3

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{
    AmountOrScalar, FinancialModelSpec, ForecastMethod, ForecastSpec,
};
use finstack_quant_statements::Error;
use indexmap::indexmap;

// Evaluation Context Tests

#[test]
fn test_context_set_and_get_value() {
    use finstack_quant_statements::evaluator::EvaluationContext;
    use finstack_quant_statements::types::NodeId;
    use indexmap::IndexMap;

    let mut node_to_column = IndexMap::new();
    node_to_column.insert(NodeId::new("revenue"), 0);
    node_to_column.insert(NodeId::new("cogs"), 1);

    let mut ctx = EvaluationContext::new(
        PeriodId::quarter(2025, 1).expect("valid period fixture"),
        std::sync::Arc::new(node_to_column),
        std::sync::Arc::new(IndexMap::new()),
    );

    ctx.set_value("revenue", 100_000.0).unwrap();
    ctx.set_value("cogs", 60_000.0).unwrap();

    assert_eq!(ctx.get_value("revenue").unwrap(), 100_000.0);
    assert_eq!(ctx.get_value("cogs").unwrap(), 60_000.0);
}

#[test]
fn test_context_unknown_node_error() {
    use finstack_quant_statements::evaluator::EvaluationContext;
    use indexmap::IndexMap;

    let ctx = EvaluationContext::new(
        PeriodId::quarter(2025, 1).expect("valid period fixture"),
        std::sync::Arc::new(IndexMap::new()),
        std::sync::Arc::new(IndexMap::new()),
    );

    let result = ctx.get_value("unknown");
    assert!(result.is_err());
}

// Basic Evaluator Tests

#[test]
fn test_evaluate_value_nodes() {
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
            "opex",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(20_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(21_000.0),
                ),
            ],
        );
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get(
            "revenue",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(100_000.0)
    );
    assert_eq!(
        results.get(
            "revenue",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(110_000.0)
    );
    assert_eq!(
        results.get(
            "opex",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(20_000.0)
    );
    assert_eq!(
        results.get(
            "opex",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(21_000.0)
    );
}

#[test]
fn test_evaluate_calculated_nodes() {
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
        model_builder.compute("cogs", "revenue * 0.6").unwrap();
        model_builder
            .compute("gross_profit", "revenue - cogs")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check COGS (60% of revenue)
    assert!(
        (results
            .get(
                "cogs",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            )
            .unwrap()
            - 60_000.0)
            .abs()
            < 0.01
    );
    assert!(
        (results
            .get(
                "cogs",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            )
            .unwrap()
            - 66_000.0)
            .abs()
            < 0.01
    );

    // Check gross profit
    assert!(
        (results
            .get(
                "gross_profit",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            )
            .unwrap()
            - 40_000.0)
            .abs()
            < 0.01
    );
    assert!(
        (results
            .get(
                "gross_profit",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            )
            .unwrap()
            - 44_000.0)
            .abs()
            < 0.01
    );
}

#[test]
fn test_evaluate_arithmetic_operations() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "a",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(10.0),
            )],
        );
        model_builder.value(
            "b",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(3.0),
            )],
        );
        model_builder.compute("add", "a + b").unwrap();
        model_builder.compute("sub", "a - b").unwrap();
        model_builder.compute("mul", "a * b").unwrap();
        model_builder.compute("div", "a / b").unwrap();
        model_builder.compute("mod", "a % b").unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    assert_eq!(results.get("add", &q1), Some(13.0));
    assert_eq!(results.get("sub", &q1), Some(7.0));
    assert_eq!(results.get("mul", &q1), Some(30.0));
    assert!((results.get("div", &q1).unwrap() - 3.333333).abs() < 0.001);
    assert_eq!(results.get("mod", &q1), Some(1.0));
}

#[test]
fn test_evaluate_comparison_operations() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            )],
        );
        model_builder.compute("gt", "revenue > 50000").unwrap();
        model_builder.compute("lt", "revenue < 200000").unwrap();
        model_builder.compute("eq", "revenue == 100000").unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    assert_eq!(results.get("gt", &q1), Some(1.0)); // true
    assert_eq!(results.get("lt", &q1), Some(1.0)); // true
    assert_eq!(results.get("eq", &q1), Some(1.0)); // true
}

#[test]
fn test_evaluate_conditional() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(80_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(120_000.0),
                ),
            ],
        );
        model_builder
            .compute("bonus", "if(revenue > 100000, revenue * 0.1, 0)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: revenue < 100k, no bonus
    assert_eq!(
        results.get(
            "bonus",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(0.0)
    );

    // Q2: revenue > 100k, get bonus
    assert!(
        (results
            .get(
                "bonus",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            )
            .unwrap()
            - 12_000.0)
            .abs()
            < 0.01
    );
}

#[test]
fn test_evaluate_complex_expression() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            )],
        );
        model_builder.value(
            "cogs",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(60_000.0),
            )],
        );
        model_builder
            .compute("gross_margin", "(revenue - cogs) / revenue")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let margin = results
        .get(
            "gross_margin",
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
        )
        .unwrap();
    assert!((margin - 0.4).abs() < 0.001); // 40% margin
}

// DAG Construction Tests

#[test]
fn test_dag_simple_chain() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "a",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(10.0),
            )],
        );
        model_builder.compute("b", "a * 2").unwrap();
        model_builder.compute("c", "b + 5").unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    assert_eq!(results.get("a", &q1), Some(10.0));
    assert_eq!(results.get("b", &q1), Some(20.0));
    assert_eq!(results.get("c", &q1), Some(25.0));
}

#[test]
fn test_dag_multiple_dependencies() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.value(
            "opex",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(20.0),
            )],
        );
        model_builder.compute("cogs", "revenue * 0.6").unwrap();
        model_builder
            .compute("gross_profit", "revenue - cogs")
            .unwrap();
        model_builder
            .compute("operating_income", "gross_profit - opex")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    assert_eq!(results.get("revenue", &q1), Some(100.0));
    assert_eq!(results.get("cogs", &q1), Some(60.0));
    assert_eq!(results.get("gross_profit", &q1), Some(40.0));
    assert_eq!(results.get("opex", &q1), Some(20.0));
    assert_eq!(results.get("operating_income", &q1), Some(20.0));
}

#[test]
fn test_circular_dependency_detection() {
    // Cycles are now caught at build time by ModelBuilder::build()
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.compute("a", "b + 1").unwrap();
        model_builder.compute("b", "c + 1").unwrap();
        model_builder.compute("c", "a + 1").unwrap();
        model_builder.build()
    };

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::CircularDependency { .. }));
}

#[test]
fn test_self_reference_cycle() {
    // Self-referencing cycles are now caught at build time
    let result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.compute("a", "a + 1").unwrap();
        model_builder.build()
    };

    assert!(result.is_err());
}

// Integration Tests

#[test]
fn test_complete_pl_model() {
    let model = {
        let mut model_builder = ModelBuilder::new("P&L Model");
        model_builder.periods("2025Q1..2025Q2", None).unwrap();
        model_builder // Just Q1-Q2 for Phase 3 (no forecasts yet)
            // Revenue
            .value(
                "revenue",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(10_000_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(11_000_000.0),
                    ),
                ],
            );
        model_builder
            // COGS as 60% of revenue
            .compute("cogs", "revenue * 0.6")
            .unwrap();
        model_builder
            // Operating expenses
            .value(
                "opex",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(2_000_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(2_100_000.0),
                    ),
                ],
            );
        model_builder
            // Derived metrics
            .compute("gross_profit", "revenue - cogs")
            .unwrap();
        model_builder
            .compute("operating_income", "gross_profit - opex")
            .unwrap();
        model_builder
            .compute("gross_margin", "gross_profit / revenue")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check Q1 results
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    assert_eq!(results.get("revenue", &q1), Some(10_000_000.0));
    assert_eq!(results.get("cogs", &q1), Some(6_000_000.0));
    assert_eq!(results.get("gross_profit", &q1), Some(4_000_000.0));
    assert_eq!(results.get("opex", &q1), Some(2_000_000.0));
    assert_eq!(results.get("operating_income", &q1), Some(2_000_000.0));
    assert!((results.get("gross_margin", &q1).unwrap() - 0.4).abs() < 0.001);

    // Check Q2 results
    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
    assert_eq!(results.get("revenue", &q2), Some(11_000_000.0));
    assert_eq!(results.get("cogs", &q2), Some(6_600_000.0));
    assert_eq!(results.get("gross_profit", &q2), Some(4_400_000.0));
    assert_eq!(results.get("opex", &q2), Some(2_100_000.0));
    assert_eq!(results.get("operating_income", &q2), Some(2_300_000.0));
    assert!((results.get("gross_margin", &q2).unwrap() - 0.4).abs() < 0.001);

    // Check metadata
    assert_eq!(results.meta.num_nodes, 6);
    assert_eq!(results.meta.num_periods, 2);
    assert!(results.meta.eval_time_ms.is_some());
}

#[test]
fn test_multiple_periods_sequential() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q4", None).unwrap();
        model_builder.value(
            "base",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(121.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(133.1),
                ),
            ],
        );
        model_builder.compute("doubled", "base * 2").unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get(
            "doubled",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(200.0)
    );
    assert_eq!(
        results.get(
            "doubled",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(220.0)
    );
    assert_eq!(
        results.get(
            "doubled",
            &PeriodId::quarter(2025, 3).expect("valid period fixture")
        ),
        Some(242.0)
    );
    assert!(
        (results
            .get(
                "doubled",
                &PeriodId::quarter(2025, 4).expect("valid period fixture")
            )
            .unwrap()
            - 266.2)
            .abs()
            < 0.01
    );
}

#[test]
fn test_nested_parentheses() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.value(
            "cogs",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(60.0),
            )],
        );
        model_builder.value(
            "opex",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(20.0),
            )],
        );
        model_builder
            .compute("result", "((revenue - cogs) - opex) / revenue")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let expected = ((100.0 - 60.0) - 20.0) / 100.0; // 0.2
    let actual = results
        .get(
            "result",
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
        )
        .unwrap();
    assert!((actual - expected).abs() < 0.001);
}

#[test]
fn test_results_metadata() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "a",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(2.0),
                ),
            ],
        );
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(results.meta.num_nodes, 1);
    assert_eq!(results.meta.num_periods, 2);
    assert!(results.meta.eval_time_ms.is_some());
    assert!(results.meta.eval_time_ms.unwrap() < 1000); // Should be fast
}

// evaluate_with_market Tests

#[test]
fn test_evaluate_with_market_basic() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use time::Month;

    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q2", None).unwrap();
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
        model_builder.build().unwrap()
    };

    let market_ctx = MarketContext::new();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let results = evaluator
        .evaluate_with_market(&model, &market_ctx, as_of)
        .unwrap();

    assert_eq!(
        results.get(
            "revenue",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(100_000.0)
    );
}

#[test]
fn test_evaluate_with_market_no_capital_structure() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use time::Month;

    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.build().unwrap()
    };

    let market_ctx = MarketContext::new();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let results = evaluator
        .evaluate_with_market(&model, &market_ctx, as_of)
        .unwrap();

    assert_eq!(
        results.get(
            "revenue",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(100.0)
    );
}

// Formula Edge Cases

#[test]
fn test_shift_function() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q4", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(120.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(130.0),
                ),
            ],
        );
        model_builder
            .compute("shifted", "shift(revenue, 2)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // shift(revenue, 2) should return revenue from 2 periods ago
    assert!(results
        .get(
            "shifted",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        )
        .unwrap()
        .is_nan());
    assert!(results
        .get(
            "shifted",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        )
        .unwrap()
        .is_nan());
    assert_eq!(
        results
            .get(
                "shifted",
                &PeriodId::quarter(2025, 3).expect("valid period fixture")
            )
            .unwrap(),
        100.0
    );
    assert_eq!(
        results
            .get(
                "shifted",
                &PeriodId::quarter(2025, 4).expect("valid period fixture")
            )
            .unwrap(),
        110.0
    );
}

#[test]
fn test_ttm_function_supports_expression_arguments() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2024Q1..2025Q1", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2024, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1000.0),
                ),
                (
                    PeriodId::quarter(2024, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1100.0),
                ),
                (
                    PeriodId::quarter(2024, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(1200.0),
                ),
                (
                    PeriodId::quarter(2024, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(1300.0),
                ),
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1400.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2024, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(600.0),
                ),
                (
                    PeriodId::quarter(2024, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(650.0),
                ),
                (
                    PeriodId::quarter(2024, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(700.0),
                ),
                (
                    PeriodId::quarter(2024, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(750.0),
                ),
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(800.0),
                ),
            ],
        );
        model_builder
            .compute("ttm_gross_profit", "ttm(revenue - cogs)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // TTM requires a full 4-quarter trailing window. Partial
    // windows (Q1-Q3 2024) return NaN rather than silently summing what's
    // available. Only Q4 2024 (Q1-Q4 2024) and Q1 2025 (Q2 2024-Q1 2025)
    // have the full 4-period history needed.
    for partial in [
        PeriodId::quarter(2024, 1).expect("valid period fixture"),
        PeriodId::quarter(2024, 2).expect("valid period fixture"),
        PeriodId::quarter(2024, 3).expect("valid period fixture"),
    ] {
        let value = results.get("ttm_gross_profit", &partial).unwrap();
        assert!(
            value.is_nan(),
            "partial TTM window at {partial} should be NaN under C18 guard, got {value}"
        );
    }
    assert_eq!(
        results
            .get(
                "ttm_gross_profit",
                &PeriodId::quarter(2024, 4).expect("valid period fixture")
            )
            .unwrap(),
        1900.0
    );
    assert_eq!(
        results
            .get(
                "ttm_gross_profit",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            )
            .unwrap(),
        2100.0
    );
}

#[test]
fn test_coalesce_treats_zero_as_present_value() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q3", None).unwrap();
        model_builder.value(
            "value1",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(300.0),
                ),
            ],
        );
        model_builder.value(
            "value2",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(50.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(200.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(350.0),
                ),
            ],
        );
        model_builder
            .compute("result", "coalesce(value1, value2)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get(
            "result",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(100.0)
    );
    assert_eq!(
        results.get(
            "result",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(0.0)
    );
    assert_eq!(
        results.get(
            "result",
            &PeriodId::quarter(2025, 3).expect("valid period fixture")
        ),
        Some(300.0)
    );
}

#[test]
fn test_quantile_does_not_double_count_current_period() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q4", None).unwrap();
        model_builder.value(
            "series",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(10.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(20.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(30.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(40.0),
                ),
            ],
        );
        model_builder
            .compute("median", "quantile(series, 0.5)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get(
            "median",
            &PeriodId::quarter(2025, 4).expect("valid period fixture")
        ),
        Some(25.0)
    );
}

#[test]
fn test_lag_rejects_non_integer_offsets() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q2", None).unwrap();
        model_builder.value(
            "series",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(10.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(20.0),
                ),
            ],
        );
        model_builder.compute("lagged", "lag(series, 1.5)").unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let err = evaluator.evaluate(&model).unwrap_err();
    let message = err.to_string();

    assert!(
        message.contains("integer"),
        "expected integer validation error, got: {message}"
    );
}

#[test]
fn test_shift_zero_periods() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
            ],
        );
        model_builder
            .compute("shifted", "shift(revenue, 0)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // shift by 0 should return the value itself
    assert_eq!(
        results
            .get(
                "shifted",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            )
            .unwrap(),
        100.0
    );
    assert_eq!(
        results
            .get(
                "shifted",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            )
            .unwrap(),
        110.0
    );
}

#[test]
fn test_shift_negative_returns_nan() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
            ],
        );
        model_builder
            .compute("future_shifted", "shift(revenue, -1)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Negative shift (forward-looking) should return NaN
    assert!(results
        .get(
            "future_shifted",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        )
        .unwrap()
        .is_nan());
}

#[test]
fn test_rank_function() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q4", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(150.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(120.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(180.0),
                ),
            ],
        );
        model_builder
            .compute("revenue_rank", "rank(revenue)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Rank is cumulative - Q1 only has itself (rank 1)
    assert_eq!(
        results
            .get(
                "revenue_rank",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            )
            .unwrap(),
        1.0
    );

    // Q2 has seen [100, 150] - 150 is rank 2
    assert_eq!(
        results
            .get(
                "revenue_rank",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            )
            .unwrap(),
        2.0
    );

    // Q3 has seen [100, 150, 120] sorted to [100, 120, 150] - 120 is rank 2
    assert_eq!(
        results
            .get(
                "revenue_rank",
                &PeriodId::quarter(2025, 3).expect("valid period fixture")
            )
            .unwrap(),
        2.0
    );

    // Q4 has seen [100, 150, 120, 180] sorted to [100, 120, 150, 180] - 180 is rank 4
    assert_eq!(
        results
            .get(
                "revenue_rank",
                &PeriodId::quarter(2025, 4).expect("valid period fixture")
            )
            .unwrap(),
        4.0
    );
}

#[test]
fn test_quantile_function_edge_values() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q4", None).unwrap();
        model_builder.value(
            "data",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(10.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(20.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(30.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(40.0),
                ),
            ],
        );
        model_builder.compute("q0", "quantile(data, 0.0)").unwrap();
        model_builder
            .compute("q25", "quantile(data, 0.25)")
            .unwrap();
        model_builder.compute("q50", "quantile(data, 0.5)").unwrap();
        model_builder
            .compute("q100", "quantile(data, 1.0)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q4 = PeriodId::quarter(2025, 4).expect("valid period fixture");

    // Q0 should be minimum
    assert_eq!(results.get("q0", &q4).unwrap(), 10.0);

    // Q100 should be maximum
    assert_eq!(results.get("q100", &q4).unwrap(), 40.0);

    // Q50 should be median (between 20 and 30)
    let q50 = results.get("q50", &q4).unwrap();
    assert!((20.0..=30.0).contains(&q50));
}

#[test]
fn test_ewm_mean_function() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q4", None).unwrap();
        model_builder.value(
            "returns",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.05),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.08),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.06),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.07),
                ),
            ],
        );
        model_builder
            .compute("ewm", "ewm_mean(returns, 0.3)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let ewm = results
        .get(
            "ewm",
            &PeriodId::quarter(2025, 4).expect("valid period fixture"),
        )
        .unwrap();
    assert!(ewm > 0.0);
    assert!(!ewm.is_nan());
}

#[test]
fn test_cumulative_functions() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q4", None).unwrap();
        model_builder.value(
            "base_values",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(10.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(15.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(12.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(18.0),
                ),
            ],
        );
        model_builder
            .compute("cumsum_result", "cumsum(base_values)")
            .unwrap();
        model_builder
            .compute("cumprod_result", "cumprod(base_values)")
            .unwrap();
        model_builder
            .compute("cummin_result", "cummin(base_values)")
            .unwrap();
        model_builder
            .compute("cummax_result", "cummax(base_values)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q4 = PeriodId::quarter(2025, 4).expect("valid period fixture");

    // cumsum: 10 + 15 + 12 + 18 = 55
    assert_eq!(results.get("cumsum_result", &q4).unwrap(), 55.0);

    // cumprod: 10 * 15 * 12 * 18 = 32400
    assert_eq!(results.get("cumprod_result", &q4).unwrap(), 32400.0);

    // cummin: min of all = 10
    assert_eq!(results.get("cummin_result", &q4).unwrap(), 10.0);

    // cummax: max of all = 18
    assert_eq!(results.get("cummax_result", &q4).unwrap(), 18.0);
}

// Forecast Evaluation Error Paths

#[test]
fn test_forecast_error_no_actual_periods() {
    // All periods are forecast - should error
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder
            .periods("2025Q1..2025Q2", None) // No actuals_until
            .unwrap();
        model_builder.value(
            "revenue",
            &[], // No values provided
        );
        model_builder.forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::GrowthPct,
                params: indexmap! { "rate".into() => serde_json::json!(0.05) },
            },
        );
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    // Should error due to missing base value
    assert!(result.is_err());
}

#[test]
fn test_forecast_error_all_actual_periods() {
    // No forecast periods - should error when forecast is defined
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder
            .periods("2025Q1..2025Q2", Some("2025Q2"))
            .unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
            ],
        );
        model_builder.forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::GrowthPct,
                params: indexmap! { "rate".into() => serde_json::json!(0.05) },
            },
        );
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();

    // Should succeed but not use forecast (all periods are actuals)
    let results = evaluator.evaluate(&model).unwrap();

    // Values should be from explicit values, not forecast
    assert_eq!(
        results.get(
            "revenue",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(100.0)
    );
    assert_eq!(
        results.get(
            "revenue",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(110.0)
    );
}

// Division by Zero and NaN Handling

#[test]
fn test_division_by_zero_produces_nan() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        model_builder.value(
            "numerator",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder.value(
            "denominator",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(0.0),
            )],
        );
        model_builder
            .compute("result", "numerator / denominator")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let result = results
        .get(
            "result",
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
        )
        .unwrap();
    assert!(result.is_nan());
}

#[test]
fn test_pct_change_division_by_zero() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q2", None).unwrap();
        model_builder.value(
            "value",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
            ],
        );
        model_builder
            .compute("change", "pct_change(value)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // pct_change from 0 should be NaN (division by zero)
    assert!(results
        .get(
            "change",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        )
        .unwrap()
        .is_nan());
}

#[test]
fn test_rolling_functions_edge_cases() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q4", None).unwrap();
        model_builder.value(
            "base_data",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(5.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(10.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(15.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(20.0),
                ),
            ],
        );
        model_builder
            .compute("rolling_median_result", "rolling_median(base_data, 3)")
            .unwrap();
        model_builder
            .compute("rolling_min_result", "rolling_min(base_data, 3)")
            .unwrap();
        model_builder
            .compute("rolling_max_result", "rolling_max(base_data, 3)")
            .unwrap();
        model_builder
            .compute("rolling_count_result", "rolling_count(base_data, 3)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q4 = PeriodId::quarter(2025, 4).expect("valid period fixture");

    // Rolling median of [10, 15, 20] = 15
    assert_eq!(results.get("rolling_median_result", &q4).unwrap(), 15.0);

    // Rolling min of [10, 15, 20] = 10
    assert_eq!(results.get("rolling_min_result", &q4).unwrap(), 10.0);

    // Rolling max of [10, 15, 20] = 20
    assert_eq!(results.get("rolling_max_result", &q4).unwrap(), 20.0);

    // Rolling count = 3
    assert_eq!(results.get("rolling_count_result", &q4).unwrap(), 3.0);
}

// Error Handling Tests

#[test]
fn test_lag_negative_periods_error() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder
            .compute("bad_lag", "lag(revenue, -1)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let eval_result = evaluator.evaluate(&model);

    assert!(eval_result.is_err());
}

#[test]
fn test_diff_zero_periods_returns_zero() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
            ],
        );
        model_builder
            .compute("zero_diff", "diff(revenue, 0)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        result.get(
            "zero_diff",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(0.0)
    );
    assert_eq!(
        result.get(
            "zero_diff",
            &PeriodId::quarter(2025, 2).expect("valid period fixture")
        ),
        Some(0.0)
    );
}

#[test]
fn test_rolling_window_zero_size_error() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        model_builder.value(
            "data",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder
            .compute("bad_rolling", "rolling_mean(data, 0)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
}

#[test]
fn test_quantile_out_of_range_error() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        model_builder.value(
            "data",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder
            .compute("bad_quantile", "quantile(data, 1.5)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
}

#[test]
fn test_ewm_mean_alpha_out_of_range() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q2", None).unwrap();
        model_builder.value(
            "data",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
            ],
        );
        model_builder
            .compute("bad_ewm", "ewm_mean(data, 1.5)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
}

#[test]
fn test_ewm_var_insufficient_data() {
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..2025Q1", None).unwrap();
        model_builder.value(
            "data",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            )],
        );
        model_builder
            .compute("ewm_variance", "ewm_var(data, 0.3)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // With only 1 value, should return NaN
    assert!(results
        .get(
            "ewm_variance",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        )
        .unwrap()
        .is_nan());
}

// Regression tests

/// Nested aggregates under lagged evaluation must not look ahead: the
/// rolling window evaluated at the lagged period may only see data up to
/// that period.
#[test]
fn test_lagged_rolling_mean_has_no_look_ahead() {
    let model = {
        let mut model_builder = ModelBuilder::new("lagged-rolling");
        model_builder.periods("2025Q1..Q4", None).unwrap();
        model_builder.value(
            "x",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(2.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(4.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(8.0),
                ),
            ],
        );
        model_builder
            .compute("y", "lag(rolling_mean(x, 2), 2)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // At Q4 the lag(.., 2) evaluates rolling_mean(x, 2) as of Q2:
    // mean(x@Q1, x@Q2) = 1.5. The look-ahead bug returned mean(x@Q2, x@Q3) = 3.
    let y_q4 = results
        .get(
            "y",
            &PeriodId::quarter(2025, 4).expect("valid period fixture"),
        )
        .unwrap();
    assert!(
        (y_q4 - 1.5).abs() < 1e-12,
        "lagged rolling mean must only see data up to the lagged period, got {y_q4}"
    );
}

/// The same look-ahead guard must apply when the rolling expression is not a
/// bare column; expression collectors have their own period enumeration path.
#[test]
fn test_lagged_expression_rolling_mean_has_no_look_ahead() {
    let model = {
        let mut model_builder = ModelBuilder::new("lagged-expression-rolling");
        model_builder.periods("2025Q1..Q4", None).unwrap();
        model_builder.value(
            "x",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(2.0),
                ),
                (
                    PeriodId::quarter(2025, 3).expect("valid period fixture"),
                    AmountOrScalar::scalar(4.0),
                ),
                (
                    PeriodId::quarter(2025, 4).expect("valid period fixture"),
                    AmountOrScalar::scalar(8.0),
                ),
            ],
        );
        model_builder
            .compute("y", "lag(rolling_mean(x * 1, 2), 2)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let y_q4 = results
        .get(
            "y",
            &PeriodId::quarter(2025, 4).expect("valid period fixture"),
        )
        .unwrap();
    assert!(
        (y_q4 - 1.5).abs() < 1e-12,
        "lagged expression rolling mean must only see data up to the lagged period, got {y_q4}"
    );
}

/// `lag`/`diff`/`pct_change` with expression (non-column) arguments must
/// return NaN at period boundaries, matching the column path, instead of
/// erroring against an empty historical context.
#[test]
fn test_timeseries_expression_args_return_nan_at_boundary() {
    let model = {
        let mut model_builder = ModelBuilder::new("ts-boundary");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "x",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110.0),
                ),
            ],
        );
        model_builder.compute("lagged", "lag(x * 1, 1)").unwrap();
        model_builder.compute("diffed", "diff(x * 1, 1)").unwrap();
        model_builder
            .compute("pct", "pct_change(x * 1, 1)")
            .unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("boundary lags must not abort evaluation");

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
    assert!(results.get("lagged", &q1).unwrap().is_nan());
    assert!(results.get("diffed", &q1).unwrap().is_nan());
    assert!(results.get("pct", &q1).unwrap().is_nan());
    // Periods with available history behave normally.
    assert!((results.get("lagged", &q2).unwrap() - 100.0).abs() < 1e-12);
    assert!((results.get("diffed", &q2).unwrap() - 10.0).abs() < 1e-12);
    assert!((results.get("pct", &q2).unwrap() - 0.1).abs() < 1e-12);
}

/// CAGR is undefined for non-positive bases: a negative starting value flips
/// the sign of the computed rate (improving losses would show negative
/// growth), so `growth_rate` returns NaN.
#[test]
fn test_growth_rate_negative_base_returns_nan() {
    let model = {
        let mut model_builder = ModelBuilder::new("growth-negative");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "x",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(-100.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(-50.0),
                ),
            ],
        );
        model_builder.compute("g", "growth_rate(x, 1)").unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert!(
        results
            .get(
                "g",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            )
            .unwrap()
            .is_nan(),
        "growth_rate over a negative base must be NaN, not a sign-flipped rate"
    );
}

/// `pct_change(x, 0)` must propagate NaN from the inner expression instead
/// of masking missing data with 0.0 (mirrors the diff(x, 0) guard).
#[test]
fn test_pct_change_zero_periods_propagates_nan() {
    let model = {
        let mut model_builder = ModelBuilder::new("pct-zero");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.compute("x", "0 / 0").unwrap();
        model_builder.compute("y", "pct_change(x, 0)").unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert!(results
        .get(
            "y",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        )
        .unwrap()
        .is_nan());
}

/// `evaluate_prepared` must use the compiled formulas captured at prepare
/// time, not whatever a later `evaluate` of a different model (sharing node
/// ids) left in the evaluator's cache.
#[test]
fn test_evaluate_prepared_is_isolated_from_later_evaluations() {
    let model_a = {
        let mut model_builder = ModelBuilder::new("model-a");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.compute("x", "1 + 1").unwrap();
        model_builder.build().unwrap()
    };
    let model_b = {
        let mut model_builder = ModelBuilder::new("model-b");
        model_builder.periods("2025Q1..Q1", None).unwrap();
        model_builder.compute("x", "5 * 5").unwrap();
        model_builder.build().unwrap()
    };

    let mut evaluator = Evaluator::new();
    let prepared = evaluator.prepare(&model_a).unwrap();

    // Evaluating model_b overwrites the evaluator's mutable compiled cache.
    let results_b = evaluator.evaluate(&model_b).unwrap();
    assert_eq!(
        results_b.get(
            "x",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(25.0)
    );

    // The prepared evaluation must still produce model_a's formula result.
    let results_a = evaluator.evaluate_prepared(&model_a, &prepared).unwrap();
    assert_eq!(
        results_a.get(
            "x",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(2.0),
        "prepared evaluation must use the formulas compiled at prepare time"
    );
}

// Windowed formulas on monetary nodes
//
// A debt corkscrew carries the prior period's closing balance forward with
// `lag(debt_balance, 1)`. The window/offset argument is a dimensionless
// period count, so it must not participate in dimension unification: folding
// it in rejected every windowed formula over a monetary node with
// "Dimensional mismatch in lag: cannot combine scalar and USD".

/// Build a two-period model holding a USD `debt_balance` node and a
/// dimensionless `ratio` node, plus one formula node applying `formula`.
fn window_model(
    node_id: &str,
    formula: &str,
) -> finstack_quant_statements::Result<FinancialModelSpec> {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

    {
        let mut model_builder = ModelBuilder::new("window-dimensions");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value_money(
            "debt_balance",
            &[
                (q1, Money::from((1_000_i64, Currency::USD))),
                (q2, Money::from((1_400_i64, Currency::USD))),
            ],
        );
        model_builder.value_scalar("ratio", &[(q1, 0.25), (q2, 0.5)]);
        model_builder.compute(node_id, formula).unwrap();
        model_builder.build()
    }
}

/// Assert that `formula` builds, that its node carries `expected_type`, and
/// that it evaluates to `expected_q2` in 2025Q2.
fn assert_window_formula(
    formula: &str,
    expected_type: finstack_quant_statements::types::NodeValueType,
    expected_q2: f64,
) {
    let model = window_model("windowed", formula)
        .unwrap_or_else(|error| panic!("{formula} must build: {error}"));

    assert_eq!(
        model.get_node("windowed").expect("node exists").value_type,
        Some(expected_type),
        "{formula} must carry its series argument's dimension"
    );

    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .unwrap_or_else(|error| panic!("{formula} must evaluate: {error}"));
    let actual = results
        .get(
            "windowed",
            &PeriodId::quarter(2025, 2).expect("valid period fixture"),
        )
        .unwrap_or_else(|| panic!("{formula} produced no 2025Q2 value"));
    assert!(
        (actual - expected_q2).abs() < 1e-9,
        "{formula} expected {expected_q2} at 2025Q2, got {actual}"
    );
}

#[test]
fn test_windowed_functions_on_monetary_node() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_statements::types::NodeValueType;

    let usd = NodeValueType::Monetary {
        currency: Currency::USD,
    };

    // A debt corkscrew's opening balance: Q2 opens at Q1's closing balance.
    assert_window_formula("lag(debt_balance, 1)", usd, 1_000.0);
    assert_window_formula("shift(debt_balance, 1)", usd, 1_000.0);
    assert_window_formula("rolling_sum(debt_balance, 2)", usd, 2_400.0);
}

#[test]
fn test_windowed_functions_on_dimensionless_node() {
    use finstack_quant_statements::types::NodeValueType;

    assert_window_formula("lag(ratio, 1)", NodeValueType::Scalar, 0.25);
    assert_window_formula("shift(ratio, 1)", NodeValueType::Scalar, 0.25);
    assert_window_formula("rolling_sum(ratio, 2)", NodeValueType::Scalar, 0.75);
}

/// The dimension rule must not widen into a hole: `clamp` and `coalesce`
/// carry genuine quantities in every argument position, so mixing currencies
/// through them is still rejected at build time.
#[test]
fn test_value_variadic_functions_reject_currency_mismatch() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");

    for formula in [
        "clamp(usd_amount, eur_amount, usd_amount)",
        "coalesce(lag(usd_amount, 1), eur_amount)",
    ] {
        let result = {
            let mut model_builder = ModelBuilder::new("mixed-currency");
            model_builder.periods("2025Q1..Q1", None).unwrap();
            model_builder.value_money("usd_amount", &[(q1, Money::from((100_i64, Currency::USD)))]);
            model_builder.value_money("eur_amount", &[(q1, Money::from((40_i64, Currency::EUR)))]);
            model_builder.compute("mixed", formula).unwrap();
            model_builder.build()
        };

        let error = result
            .err()
            .unwrap_or_else(|| panic!("{formula} mixes currencies and must be rejected"))
            .to_string();
        assert!(
            error.contains("Dimensional mismatch"),
            "{formula} must fail with a dimension error, got: {error}"
        );
    }
}
