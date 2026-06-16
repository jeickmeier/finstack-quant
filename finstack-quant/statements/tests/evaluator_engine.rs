//! Evaluator engine integration tests.
#![allow(clippy::expect_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::EvalWarning;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeSpec, NodeType};
use indexmap::IndexMap;

#[test]
fn test_simple_evaluation() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .expect("test should succeed")
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
        .expect("test should succeed");

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).expect("test should succeed");

    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 1)),
        Some(100_000.0)
    );
    assert_eq!(
        results.get("revenue", &PeriodId::quarter(2025, 2)),
        Some(110_000.0)
    );
}

#[test]
fn test_formula_evaluation() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .expect("test should succeed")
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
        .expect("test should succeed")
        .build()
        .expect("test should succeed");

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).expect("test should succeed");

    assert_eq!(
        results.get("cogs", &PeriodId::quarter(2025, 1)),
        Some(60_000.0)
    );
    assert_eq!(
        results.get("cogs", &PeriodId::quarter(2025, 2)),
        Some(66_000.0)
    );
}

#[test]
fn test_circular_dependency_error() {
    // Cycles are now caught at build time by ModelBuilder::build()
    let result = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .expect("test should succeed")
        .compute("a", "b + 1")
        .expect("test should succeed")
        .compute("b", "a + 1")
        .expect("test should succeed")
        .build();

    assert!(result.is_err());
    assert!(result
        .expect_err("should fail with circular dependency")
        .to_string()
        .contains("Circular"));
}

#[test]
fn test_recompile_on_reuse() {
    let periods = ModelBuilder::new("cache")
        .periods("2025Q1..Q1", None)
        .expect("valid periods")
        .build()
        .expect("build should succeed")
        .periods;

    let mut model = FinancialModelSpec::new("cache", periods);
    let mut nodes = IndexMap::new();
    nodes.insert(
        "x".into(),
        NodeSpec::new("x", NodeType::Value).with_values(
            [(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0))]
                .into_iter()
                .collect(),
        ),
    );
    nodes.insert(
        "y".into(),
        NodeSpec::new("y", NodeType::Calculated).with_formula("x * 2"),
    );
    model.nodes = nodes;

    let mut evaluator = Evaluator::new();
    let first = evaluator.evaluate(&model).expect("first eval");
    assert_eq!(first.get("y", &PeriodId::quarter(2025, 1)), Some(20.0));

    if let Some(node) = model.get_node_mut("y") {
        node.formula_text = Some("x * 3".to_string());
    }

    let second = evaluator.evaluate(&model).expect("second eval");
    assert_eq!(second.get("y", &PeriodId::quarter(2025, 1)), Some(30.0));
}

#[test]
fn test_results_include_warnings() {
    let period = PeriodId::quarter(2025, 1);
    let model = ModelBuilder::new("warnings")
        .periods("2025Q1..Q1", None)
        .expect("valid period")
        .value("denominator", &[(period, AmountOrScalar::scalar(0.0))])
        .compute("ratio", "1.0 / denominator")
        .expect("valid formula")
        .build()
        .expect("valid model");

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).expect("evaluation succeeds");

    assert!(
        results.meta.warnings.iter().any(|warning| matches!(
            warning,
            EvalWarning::DivisionByZero { node_id, period: p } if node_id == "ratio" && *p == period
        )),
        "expected division-by-zero warning for ratio node"
    );
}
