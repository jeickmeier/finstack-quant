//! Evaluator tests for Phase 3

use finstack_statements::prelude::*;
use indexmap::indexmap;

// ============================================================================
// Evaluation Context Tests
// ============================================================================

#[test]
fn test_context_set_and_get_value() {
    use finstack_statements::evaluator::EvaluationContext;
    use finstack_statements::types::NodeId;
    use indexmap::IndexMap;

    let mut node_to_column = IndexMap::new();
    node_to_column.insert(NodeId::new("revenue"), 0);
    node_to_column.insert(NodeId::new("cogs"), 1);

    let mut ctx = EvaluationContext::new(
        PeriodId::quarter(2025, 1),
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
    use finstack_statements::evaluator::EvaluationContext;
    use indexmap::IndexMap;

    let ctx = EvaluationContext::new(
        PeriodId::quarter(2025, 1),
        std::sync::Arc::new(IndexMap::new()),
        std::sync::Arc::new(IndexMap::new()),
    );

    let result = ctx.get_value("unknown");
    assert!(result.is_err());
}

// ============================================================================
// Basic Evaluator Tests
// ============================================================================

#[test]
fn test_evaluate_value_nodes() {
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
            "opex",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(20_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(21_000.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100_000.0)
    );
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 2)),
        Some(110_000.0)
    );
    assert_eq!(
        results.get("opex", &PeriodId::quarter(2025, 1)),
        Some(20_000.0)
    );
    assert_eq!(
        results.get("opex", &PeriodId::quarter(2025, 2)),
        Some(21_000.0)
    );
}

#[test]
fn test_evaluate_calculated_nodes() {
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
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check COGS (60% of revenue)
    assert!((results.get("cogs", &PeriodId::quarter(2025, 1)).unwrap() - 60_000.0).abs() < 0.01);
    assert!((results.get("cogs", &PeriodId::quarter(2025, 2)).unwrap() - 66_000.0).abs() < 0.01);

    // Check gross profit
    assert!(
        (results
            .get("gross_profit", &PeriodId::quarter(2025, 1))
            .unwrap()
            - 40_000.0)
            .abs()
            < 0.01
    );
    assert!(
        (results
            .get("gross_profit", &PeriodId::quarter(2025, 2))
            .unwrap()
            - 44_000.0)
            .abs()
            < 0.01
    );
}

#[test]
fn test_evaluate_arithmetic_operations() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "a",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0))],
        )
        .value(
            "b",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(3.0))],
        )
        .compute("add", "a + b")
        .unwrap()
        .compute("sub", "a - b")
        .unwrap()
        .compute("mul", "a * b")
        .unwrap()
        .compute("div", "a / b")
        .unwrap()
        .compute("mod", "a % b")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    assert_eq!(results.get("add", &q1), Some(13.0));
    assert_eq!(results.get("sub", &q1), Some(7.0));
    assert_eq!(results.get("mul", &q1), Some(30.0));
    assert!((results.get("div", &q1).unwrap() - 3.333333).abs() < 0.001);
    assert_eq!(results.get("mod", &q1), Some(1.0));
}

#[test]
fn test_evaluate_comparison_operations() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            )],
        )
        .compute("gt", "revenue > 50000")
        .unwrap()
        .compute("lt", "revenue < 200000")
        .unwrap()
        .compute("eq", "revenue == 100000")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    assert_eq!(results.get("gt", &q1), Some(1.0)); // true
    assert_eq!(results.get("lt", &q1), Some(1.0)); // true
    assert_eq!(results.get("eq", &q1), Some(1.0)); // true
}

#[test]
fn test_evaluate_conditional() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(80_000.0)),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(120_000.0),
                ),
            ],
        )
        .compute("bonus", "if(revenue > 100000, revenue * 0.1, 0)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: revenue < 100k, no bonus
    assert_eq!(results.get("bonus", &PeriodId::quarter(2025, 1)), Some(0.0));

    // Q2: revenue > 100k, get bonus
    assert!((results.get("bonus", &PeriodId::quarter(2025, 2)).unwrap() - 12_000.0).abs() < 0.01);
}

#[test]
fn test_evaluate_complex_expression() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            )],
        )
        .value(
            "cogs",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60_000.0))],
        )
        .compute("gross_margin", "(revenue - cogs) / revenue")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let margin = results
        .get("gross_margin", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert!((margin - 0.4).abs() < 0.001); // 40% margin
}

// ============================================================================
// DAG Construction Tests
// ============================================================================

#[test]
fn test_dag_simple_chain() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "a",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0))],
        )
        .compute("b", "a * 2")
        .unwrap()
        .compute("c", "b + 5")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    assert_eq!(results.get("a", &q1), Some(10.0));
    assert_eq!(results.get("b", &q1), Some(20.0));
    assert_eq!(results.get("c", &q1), Some(25.0));
}

#[test]
fn test_dag_multiple_dependencies() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .value(
            "opex",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(20.0))],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("operating_income", "gross_profit - opex")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1 = PeriodId::quarter(2025, 1);
    assert_eq!(results.get("revenue", &q1), Some(100.0));
    assert_eq!(results.get("cogs", &q1), Some(60.0));
    assert_eq!(results.get("gross_profit", &q1), Some(40.0));
    assert_eq!(results.get("opex", &q1), Some(20.0));
    assert_eq!(results.get("operating_income", &q1), Some(20.0));
}

#[test]
fn test_circular_dependency_detection() {
    // Cycles are now caught at build time by ModelBuilder::build()
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .compute("a", "b + 1")
        .unwrap()
        .compute("b", "c + 1")
        .unwrap()
        .compute("c", "a + 1")
        .unwrap()
        .build();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::CircularDependency { .. }));
}

#[test]
fn test_self_reference_cycle() {
    // Self-referencing cycles are now caught at build time
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .compute("a", "a + 1")
        .unwrap()
        .build();

    assert!(result.is_err());
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_complete_pl_model() {
    let model = ModelBuilder::new("P&L Model")
        .periods("2025Q1..2025Q2", None)
        .unwrap() // Just Q1-Q2 for Phase 3 (no forecasts yet)
        // Revenue
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(10_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(11_000_000.0),
                ),
            ],
        )
        // COGS as 60% of revenue
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        // Operating expenses
        .value(
            "opex",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(2_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(2_100_000.0),
                ),
            ],
        )
        // Derived metrics
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("operating_income", "gross_profit - opex")
        .unwrap()
        .compute("gross_margin", "gross_profit / revenue")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check Q1 results
    let q1 = PeriodId::quarter(2025, 1);
    assert_eq!(results.get("revenue", &q1), Some(10_000_000.0));
    assert_eq!(results.get("cogs", &q1), Some(6_000_000.0));
    assert_eq!(results.get("gross_profit", &q1), Some(4_000_000.0));
    assert_eq!(results.get("opex", &q1), Some(2_000_000.0));
    assert_eq!(results.get("operating_income", &q1), Some(2_000_000.0));
    assert!((results.get("gross_margin", &q1).unwrap() - 0.4).abs() < 0.001);

    // Check Q2 results
    let q2 = PeriodId::quarter(2025, 2);
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
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "base",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(121.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(133.1)),
            ],
        )
        .compute("doubled", "base * 2")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get("doubled", &PeriodId::quarter(2025, 1)),
        Some(200.0)
    );
    assert_eq!(
        results.get("doubled", &PeriodId::quarter(2025, 2)),
        Some(220.0)
    );
    assert_eq!(
        results.get("doubled", &PeriodId::quarter(2025, 3)),
        Some(242.0)
    );
    assert!((results.get("doubled", &PeriodId::quarter(2025, 4)).unwrap() - 266.2).abs() < 0.01);
}

#[test]
fn test_nested_parentheses() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .value(
            "cogs",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60.0))],
        )
        .value(
            "opex",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(20.0))],
        )
        .compute("result", "((revenue - cogs) - opex) / revenue")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let expected = ((100.0 - 60.0) - 20.0) / 100.0; // 0.2
    let actual = results.get("result", &PeriodId::quarter(2025, 1)).unwrap();
    assert!((actual - expected).abs() < 0.001);
}

#[test]
fn test_results_metadata() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "a",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(2.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(results.meta.num_nodes, 1);
    assert_eq!(results.meta.num_periods, 2);
    assert!(results.meta.eval_time_ms.is_some());
    assert!(results.meta.eval_time_ms.unwrap() < 1000); // Should be fast
}

// ============================================================================
// evaluate_with_market Tests
// ============================================================================

#[test]
fn test_evaluate_with_market_basic() {
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use time::Month;

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
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        )
        .build()
        .unwrap();

    let market_ctx = MarketContext::new();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let mut evaluator = finstack_statements::evaluator::Evaluator::new();
    let results = evaluator
        .evaluate_with_market(&model, &market_ctx, as_of)
        .unwrap();

    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100_000.0)
    );
}

#[test]
fn test_evaluate_with_market_no_capital_structure() {
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use time::Month;

    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .build()
        .unwrap();

    let market_ctx = MarketContext::new();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let mut evaluator = finstack_statements::evaluator::Evaluator::new();
    let results = evaluator
        .evaluate_with_market(&model, &market_ctx, as_of)
        .unwrap();

    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100.0)
    );
}

// ============================================================================
// Formula Edge Cases
// ============================================================================

#[test]
fn test_shift_function() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(120.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(130.0)),
            ],
        )
        .compute("shifted", "shift(revenue, 2)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // shift(revenue, 2) should return revenue from 2 periods ago
    assert!(results
        .get("shifted", &PeriodId::quarter(2025, 1))
        .unwrap()
        .is_nan());
    assert!(results
        .get("shifted", &PeriodId::quarter(2025, 2))
        .unwrap()
        .is_nan());
    assert_eq!(
        results.get("shifted", &PeriodId::quarter(2025, 3)).unwrap(),
        100.0
    );
    assert_eq!(
        results.get("shifted", &PeriodId::quarter(2025, 4)).unwrap(),
        110.0
    );
}

#[test]
fn test_ttm_function_supports_expression_arguments() {
    let model = ModelBuilder::new("test")
        .periods("2024Q1..2025Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2024, 1), AmountOrScalar::scalar(1000.0)),
                (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(1100.0)),
                (PeriodId::quarter(2024, 3), AmountOrScalar::scalar(1200.0)),
                (PeriodId::quarter(2024, 4), AmountOrScalar::scalar(1300.0)),
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1400.0)),
            ],
        )
        .value(
            "cogs",
            &[
                (PeriodId::quarter(2024, 1), AmountOrScalar::scalar(600.0)),
                (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(650.0)),
                (PeriodId::quarter(2024, 3), AmountOrScalar::scalar(700.0)),
                (PeriodId::quarter(2024, 4), AmountOrScalar::scalar(750.0)),
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(800.0)),
            ],
        )
        .compute("ttm_gross_profit", "ttm(revenue - cogs)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // TTM requires a full 4-quarter trailing window. Partial
    // windows (Q1-Q3 2024) return NaN rather than silently summing what's
    // available. Only Q4 2024 (Q1-Q4 2024) and Q1 2025 (Q2 2024-Q1 2025)
    // have the full 4-period history needed.
    for partial in [
        PeriodId::quarter(2024, 1),
        PeriodId::quarter(2024, 2),
        PeriodId::quarter(2024, 3),
    ] {
        let value = results.get("ttm_gross_profit", &partial).unwrap();
        assert!(
            value.is_nan(),
            "partial TTM window at {partial} should be NaN under C18 guard, got {value}"
        );
    }
    assert_eq!(
        results
            .get("ttm_gross_profit", &PeriodId::quarter(2024, 4))
            .unwrap(),
        1900.0
    );
    assert_eq!(
        results
            .get("ttm_gross_profit", &PeriodId::quarter(2025, 1))
            .unwrap(),
        2100.0
    );
}

#[test]
fn test_coalesce_treats_zero_as_present_value() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q3", None)
        .unwrap()
        .value(
            "value1",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(300.0)),
            ],
        )
        .value(
            "value2",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(50.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(200.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(350.0)),
            ],
        )
        .compute("result", "coalesce(value1, value2)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get("result", &PeriodId::quarter(2025, 1)),
        Some(100.0)
    );
    assert_eq!(
        results.get("result", &PeriodId::quarter(2025, 2)),
        Some(0.0)
    );
    assert_eq!(
        results.get("result", &PeriodId::quarter(2025, 3)),
        Some(300.0)
    );
}

#[test]
fn test_quantile_does_not_double_count_current_period() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "series",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(20.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(30.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(40.0)),
            ],
        )
        .compute("median", "quantile(series, 0.5)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get("median", &PeriodId::quarter(2025, 4)),
        Some(25.0)
    );
}

#[test]
fn test_lag_rejects_non_integer_offsets() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "series",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(20.0)),
            ],
        )
        .compute("lagged", "lag(series, 1.5)")
        .unwrap()
        .build()
        .unwrap();

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
        .compute("shifted", "shift(revenue, 0)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // shift by 0 should return the value itself
    assert_eq!(
        results.get("shifted", &PeriodId::quarter(2025, 1)).unwrap(),
        100.0
    );
    assert_eq!(
        results.get("shifted", &PeriodId::quarter(2025, 2)).unwrap(),
        110.0
    );
}

#[test]
fn test_shift_negative_returns_nan() {
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
        .compute("future_shifted", "shift(revenue, -1)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Negative shift (forward-looking) should return NaN
    assert!(results
        .get("future_shifted", &PeriodId::quarter(2025, 1))
        .unwrap()
        .is_nan());
}

#[test]
fn test_rank_function() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(150.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(120.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(180.0)),
            ],
        )
        .compute("revenue_rank", "rank(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Rank is cumulative - Q1 only has itself (rank 1)
    assert_eq!(
        results
            .get("revenue_rank", &PeriodId::quarter(2025, 1))
            .unwrap(),
        1.0
    );

    // Q2 has seen [100, 150] - 150 is rank 2
    assert_eq!(
        results
            .get("revenue_rank", &PeriodId::quarter(2025, 2))
            .unwrap(),
        2.0
    );

    // Q3 has seen [100, 150, 120] sorted to [100, 120, 150] - 120 is rank 2
    assert_eq!(
        results
            .get("revenue_rank", &PeriodId::quarter(2025, 3))
            .unwrap(),
        2.0
    );

    // Q4 has seen [100, 150, 120, 180] sorted to [100, 120, 150, 180] - 180 is rank 4
    assert_eq!(
        results
            .get("revenue_rank", &PeriodId::quarter(2025, 4))
            .unwrap(),
        4.0
    );
}

#[test]
fn test_quantile_function_edge_values() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "data",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(20.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(30.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(40.0)),
            ],
        )
        .compute("q0", "quantile(data, 0.0)")
        .unwrap()
        .compute("q25", "quantile(data, 0.25)")
        .unwrap()
        .compute("q50", "quantile(data, 0.5)")
        .unwrap()
        .compute("q100", "quantile(data, 1.0)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q4 = PeriodId::quarter(2025, 4);

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
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "returns",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.05)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.08)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(0.06)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(0.07)),
            ],
        )
        .compute("ewm", "ewm_mean(returns, 0.3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let ewm = results.get("ewm", &PeriodId::quarter(2025, 4)).unwrap();
    assert!(ewm > 0.0);
    assert!(!ewm.is_nan());
}

#[test]
fn test_cumulative_functions() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "base_values",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(15.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(12.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(18.0)),
            ],
        )
        .compute("cumsum_result", "cumsum(base_values)")
        .unwrap()
        .compute("cumprod_result", "cumprod(base_values)")
        .unwrap()
        .compute("cummin_result", "cummin(base_values)")
        .unwrap()
        .compute("cummax_result", "cummax(base_values)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q4 = PeriodId::quarter(2025, 4);

    // cumsum: 10 + 15 + 12 + 18 = 55
    assert_eq!(results.get("cumsum_result", &q4).unwrap(), 55.0);

    // cumprod: 10 * 15 * 12 * 18 = 32400
    assert_eq!(results.get("cumprod_result", &q4).unwrap(), 32400.0);

    // cummin: min of all = 10
    assert_eq!(results.get("cummin_result", &q4).unwrap(), 10.0);

    // cummax: max of all = 18
    assert_eq!(results.get("cummax_result", &q4).unwrap(), 18.0);
}

// ============================================================================
// Forecast Evaluation Error Paths
// ============================================================================

#[test]
fn test_forecast_error_no_actual_periods() {
    // All periods are forecast - should error
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None) // No actuals_until
        .unwrap()
        .value(
            "revenue",
            &[], // No values provided
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::GrowthPct,
                params: indexmap! { "rate".into() => serde_json::json!(0.05) },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    // Should error due to missing base value
    assert!(result.is_err());
}

#[test]
fn test_forecast_error_all_actual_periods() {
    // No forecast periods - should error when forecast is defined
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", Some("2025Q2"))
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
            ],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::GrowthPct,
                params: indexmap! { "rate".into() => serde_json::json!(0.05) },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();

    // Should succeed but not use forecast (all periods are actuals)
    let results = evaluator.evaluate(&model).unwrap();

    // Values should be from explicit values, not forecast
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100.0)
    );
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 2)),
        Some(110.0)
    );
}

// ============================================================================
// Division by Zero and NaN Handling
// ============================================================================

#[test]
fn test_division_by_zero_produces_nan() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "numerator",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .value(
            "denominator",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0))],
        )
        .compute("result", "numerator / denominator")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let result = results.get("result", &PeriodId::quarter(2025, 1)).unwrap();
    assert!(result.is_nan());
}

#[test]
fn test_pct_change_division_by_zero() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "value",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(100.0)),
            ],
        )
        .compute("change", "pct_change(value)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // pct_change from 0 should be NaN (division by zero)
    assert!(results
        .get("change", &PeriodId::quarter(2025, 2))
        .unwrap()
        .is_nan());
}

#[test]
fn test_rolling_functions_edge_cases() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "base_data",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(5.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(15.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(20.0)),
            ],
        )
        .compute("rolling_median_result", "rolling_median(base_data, 3)")
        .unwrap()
        .compute("rolling_min_result", "rolling_min(base_data, 3)")
        .unwrap()
        .compute("rolling_max_result", "rolling_max(base_data, 3)")
        .unwrap()
        .compute("rolling_count_result", "rolling_count(base_data, 3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q4 = PeriodId::quarter(2025, 4);

    // Rolling median of [10, 15, 20] = 15
    assert_eq!(results.get("rolling_median_result", &q4).unwrap(), 15.0);

    // Rolling min of [10, 15, 20] = 10
    assert_eq!(results.get("rolling_min_result", &q4).unwrap(), 10.0);

    // Rolling max of [10, 15, 20] = 20
    assert_eq!(results.get("rolling_max_result", &q4).unwrap(), 20.0);

    // Rolling count = 3
    assert_eq!(results.get("rolling_count_result", &q4).unwrap(), 3.0);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_lag_negative_periods_error() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .compute("bad_lag", "lag(revenue, -1)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let eval_result = evaluator.evaluate(&model);

    assert!(eval_result.is_err());
}

#[test]
fn test_diff_zero_periods_returns_zero() {
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
        .compute("zero_diff", "diff(revenue, 0)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        result.get("zero_diff", &PeriodId::quarter(2025, 1)),
        Some(0.0)
    );
    assert_eq!(
        result.get("zero_diff", &PeriodId::quarter(2025, 2)),
        Some(0.0)
    );
}

#[test]
fn test_rolling_window_zero_size_error() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "data",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .compute("bad_rolling", "rolling_mean(data, 0)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
}

#[test]
fn test_quantile_out_of_range_error() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "data",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .compute("bad_quantile", "quantile(data, 1.5)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
}

#[test]
fn test_ewm_mean_alpha_out_of_range() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "data",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
            ],
        )
        .compute("bad_ewm", "ewm_mean(data, 1.5)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
}

#[test]
fn test_ewm_var_insufficient_data() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "data",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .compute("ewm_variance", "ewm_var(data, 0.3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // With only 1 value, should return NaN
    assert!(results
        .get("ewm_variance", &PeriodId::quarter(2025, 1))
        .unwrap()
        .is_nan());
}

// ============================================================================
// Quant-review regression tests (2026-06-09 statements review)
// ============================================================================

/// Nested aggregates under lagged evaluation must not look ahead: the
/// rolling window evaluated at the lagged period may only see data up to
/// that period.
#[test]
fn test_lagged_rolling_mean_has_no_look_ahead() {
    let model = ModelBuilder::new("lagged-rolling")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "x",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(2.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(4.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(8.0)),
            ],
        )
        .compute("y", "lag(rolling_mean(x, 2), 2)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // At Q4 the lag(.., 2) evaluates rolling_mean(x, 2) as of Q2:
    // mean(x@Q1, x@Q2) = 1.5. The look-ahead bug returned mean(x@Q2, x@Q3) = 3.
    let y_q4 = results.get("y", &PeriodId::quarter(2025, 4)).unwrap();
    assert!(
        (y_q4 - 1.5).abs() < 1e-12,
        "lagged rolling mean must only see data up to the lagged period, got {y_q4}"
    );
}

/// The same look-ahead guard must apply when the rolling expression is not a
/// bare column; expression collectors have their own period enumeration path.
#[test]
fn test_lagged_expression_rolling_mean_has_no_look_ahead() {
    let model = ModelBuilder::new("lagged-expression-rolling")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "x",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(2.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(4.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(8.0)),
            ],
        )
        .compute("y", "lag(rolling_mean(x * 1, 2), 2)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let y_q4 = results.get("y", &PeriodId::quarter(2025, 4)).unwrap();
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
    let model = ModelBuilder::new("ts-boundary")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "x",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
            ],
        )
        .compute("lagged", "lag(x * 1, 1)")
        .unwrap()
        .compute("diffed", "diff(x * 1, 1)")
        .unwrap()
        .compute("pct", "pct_change(x * 1, 1)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("boundary lags must not abort evaluation");

    let q1 = PeriodId::quarter(2025, 1);
    let q2 = PeriodId::quarter(2025, 2);
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
    let model = ModelBuilder::new("growth-negative")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "x",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(-100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(-50.0)),
            ],
        )
        .compute("g", "growth_rate(x, 1)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert!(
        results
            .get("g", &PeriodId::quarter(2025, 2))
            .unwrap()
            .is_nan(),
        "growth_rate over a negative base must be NaN, not a sign-flipped rate"
    );
}

/// `pct_change(x, 0)` must propagate NaN from the inner expression instead
/// of masking missing data with 0.0 (mirrors the diff(x, 0) guard).
#[test]
fn test_pct_change_zero_periods_propagates_nan() {
    let model = ModelBuilder::new("pct-zero")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .compute("x", "0 / 0")
        .unwrap()
        .compute("y", "pct_change(x, 0)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert!(results
        .get("y", &PeriodId::quarter(2025, 1))
        .unwrap()
        .is_nan());
}

/// `evaluate_prepared` must use the compiled formulas captured at prepare
/// time, not whatever a later `evaluate` of a different model (sharing node
/// ids) left in the evaluator's cache.
#[test]
fn test_evaluate_prepared_is_isolated_from_later_evaluations() {
    let model_a = ModelBuilder::new("model-a")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .compute("x", "1 + 1")
        .unwrap()
        .build()
        .unwrap();
    let model_b = ModelBuilder::new("model-b")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .compute("x", "5 * 5")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let prepared = evaluator.prepare(&model_a).unwrap();

    // Evaluating model_b overwrites the evaluator's mutable compiled cache.
    let results_b = evaluator.evaluate(&model_b).unwrap();
    assert_eq!(results_b.get("x", &PeriodId::quarter(2025, 1)), Some(25.0));

    // The prepared evaluation must still produce model_a's formula result.
    let results_a = evaluator.evaluate_prepared(&model_a, &prepared).unwrap();
    assert_eq!(
        results_a.get("x", &PeriodId::quarter(2025, 1)),
        Some(2.0),
        "prepared evaluation must use the formulas compiled at prepare time"
    );
}
