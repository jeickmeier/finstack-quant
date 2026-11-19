//! Goal seek integration tests.
#![allow(clippy::expect_used, clippy::panic)]

use finstack_quant_core::{dates::PeriodId, market_data::context::MarketContext};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec};
use finstack_quant_statements_analytics::analysis::{
    GoalSeekCell, GoalSeekSearch, GoalSeekSpec, goal_seek,
};
use time::macros::date;

#[test]
fn test_goal_seek_simple_linear() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.source_snapshot("goal-seek-fixture");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid period");
        model_builder.value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))]);
        model_builder
            .compute("profit_margin", "0.15")
            .expect("valid formula");
        model_builder
            .compute("net_income", "revenue * profit_margin")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    };

    let solved = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "net_income".into(),
                period,
            },
            target_value: AmountOrScalar::Scalar(20_000.0),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "revenue".into(),
                period,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 200_000.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: false,
        },
        &MarketContext::new(),
    )
    .expect("goal seek should succeed");

    assert!((solved.solved_value.value() - 133_333.33).abs() < 1.0);
    assert!(
        solved.model.is_none(),
        "update_model=false returns no model"
    );
}

#[test]
fn test_goal_seek_with_update() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.source_snapshot("goal-seek-fixture");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid period");
        model_builder.value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))]);
        model_builder
            .compute("cogs", "revenue * 0.6")
            .expect("valid formula");
        model_builder
            .compute("gross_profit", "revenue - cogs")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    };

    let solved = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "gross_profit".into(),
                period,
            },
            target_value: AmountOrScalar::Scalar(50_000.0),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "revenue".into(),
                period,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 200_000.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: true,
        },
        &MarketContext::new(),
    )
    .expect("goal seek should succeed");

    assert!((solved.solved_value.value() - 125_000.0).abs() < 1.0);

    // The input model is untouched; the solved copy carries the new driver.
    let original = model
        .get_node("revenue")
        .and_then(|node| node.values.as_ref())
        .and_then(|values| values.get(&period))
        .expect("original value");
    assert!(matches!(original, AmountOrScalar::Scalar(s) if (*s - 100_000.0).abs() < 1e-9));
    let model = solved.model.expect("update_model=true returns the model");
    let node = model.get_node("revenue").expect("node should exist");
    let value = node
        .values
        .as_ref()
        .and_then(|v| v.get(&period))
        .expect("value should exist");

    match value {
        AmountOrScalar::Scalar(s) => {
            assert!((*s - 125_000.0).abs() < 1.0);
        }
        AmountOrScalar::Amount(_) => panic!("Expected scalar value"),
    }
}

#[test]
fn test_goal_seek_interest_coverage() {
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let q4 = PeriodId::quarter(2025, 4).expect("valid period fixture");

    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.source_snapshot("goal-seek-fixture");
        model_builder
            .periods("2025Q1..Q4", None)
            .expect("valid period range");
        model_builder.value(
            "revenue",
            &[
                (q1, AmountOrScalar::scalar(100_000.0)),
                (q4, AmountOrScalar::scalar(115_762.5)),
            ],
        );
        model_builder.forecast("revenue", ForecastSpec::growth(0.05));
        model_builder
            .compute("interest_expense", "10000.0")
            .expect("valid formula");
        model_builder
            .compute("ebitda", "revenue * 0.3")
            .expect("valid formula");
        model_builder
            .compute("interest_coverage", "ebitda / interest_expense")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    };

    let solved = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "interest_coverage".into(),
                period: q4,
            },
            target_value: AmountOrScalar::Scalar(2.0),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "revenue".into(),
                period: q4,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 200_000.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: true,
        },
        &MarketContext::new(),
    )
    .expect("goal seek should succeed");

    assert!((solved.solved_value.value() - 66_666.67).abs() < 1.0);

    let model = solved.model.expect("update_model=true returns the model");
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");
    let coverage = results
        .get("interest_coverage", &q4)
        .expect("should have value");
    assert!((coverage - 2.0).abs() < 0.01);
}

#[test]
fn test_goal_seek_invalid_target_node() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.source_snapshot("goal-seek-fixture");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid period");
        model_builder.value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))]);
        model_builder.build().expect("valid model")
    };

    let result = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "nonexistent".into(),
                period,
            },
            target_value: AmountOrScalar::Scalar(1000.0),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "revenue".into(),
                period,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 200_000.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: false,
        },
        &MarketContext::new(),
    );

    assert!(result.is_err());
}

#[test]
fn test_goal_seek_invalid_driver_node() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.source_snapshot("goal-seek-fixture");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid period");
        model_builder.value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))]);
        model_builder.build().expect("valid model")
    };

    let result = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "revenue".into(),
                period,
            },
            target_value: AmountOrScalar::Scalar(1000.0),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "nonexistent".into(),
                period,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 200_000.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: false,
        },
        &MarketContext::new(),
    );

    assert!(result.is_err());
}

#[test]
fn test_goal_seek_with_explicit_bounds() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut model_builder = ModelBuilder::new("bounds");
        model_builder.source_snapshot("goal-seek-fixture");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid period");
        model_builder.value("driver", &[(period, AmountOrScalar::scalar(0.0))]);
        model_builder
            .compute("target", "driver")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    };

    let solution = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "target".into(),
                period,
            },
            target_value: AmountOrScalar::Scalar(0.75),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "driver".into(),
                period,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.5,
                upper: 1.5,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: false,
        },
        &MarketContext::new(),
    )
    .expect("goal seek should succeed");

    assert!((solution.solved_value.value() - 0.75).abs() < 1e-9);
    // Bounded solves report diagnostics too: two endpoint probes at least.
    assert!(solution.residual.value().abs() <= solution.target_tolerance.value());
    assert!(solution.evaluations >= 3);
}

/// The diagnostics reproduce the convergence check: the residual is the
/// target node's value at the solved driver minus the target, it is within
/// the reported tolerance, and the evaluation count is recorded.
#[test]
fn test_goal_seek_reports_residual_and_evaluations() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut model_builder = ModelBuilder::new("diagnostics");
        model_builder.source_snapshot("goal-seek-fixture");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid period");
        model_builder.value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))]);
        model_builder
            .compute("cogs", "revenue * 0.6")
            .expect("valid formula");
        model_builder
            .compute("gross_profit", "revenue - cogs")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    };
    let target = 50_000.0;

    let solved = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "gross_profit".into(),
                period,
            },
            target_value: AmountOrScalar::Scalar(target),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "revenue".into(),
                period,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 200_000.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: true,
        },
        &MarketContext::new(),
    )
    .expect("goal seek should succeed");

    assert_eq!(solved.target_tolerance.value(), 1e-7);
    assert!(solved.residual.value().abs() <= solved.target_tolerance.value());
    // At least a bracket, one iterate and the final residual check.
    assert!(solved.evaluations >= 3, "{}", solved.evaluations);

    // The residual is reproducible from the returned model.
    let solved_model = solved.model.as_ref().expect("updated model");
    let results = Evaluator::new()
        .evaluate(solved_model)
        .expect("solved model evaluates");
    let actual = results.get("gross_profit", &period).expect("target value");
    assert_eq!(actual - target, solved.residual.value());

    let json = serde_json::to_value(&solved).expect("serialize");
    assert_eq!(json["target_tolerance"], 1e-7);
    assert_eq!(
        json["evaluations"].as_u64(),
        Some(solved.evaluations as u64)
    );
}

/// The result's wire form always carries `model` (`null` when the model was
/// not requested) and round-trips through serde, which both hosts rely on.
#[test]
fn test_goal_seek_result_wire_form() {
    use finstack_quant_statements_analytics::analysis::GoalSeekResult;

    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let model = {
        let mut model_builder = ModelBuilder::new("wire");
        model_builder.source_snapshot("goal-seek-fixture");
        model_builder
            .periods("2025Q1..Q1", None)
            .expect("valid period");
        model_builder.value("revenue", &[(period, AmountOrScalar::scalar(100.0))]);
        model_builder
            .compute("profit", "revenue * 0.5")
            .expect("valid formula");
        model_builder.build().expect("valid model")
    };

    let bare = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "profit".into(),
                period,
            },
            target_value: AmountOrScalar::Scalar(60.0),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "revenue".into(),
                period,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 200_000.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: false,
        },
        &MarketContext::new(),
    )
    .expect("goal seek should succeed");
    let json = serde_json::to_value(&bare).expect("serialize");
    assert_eq!(json["model"], serde_json::Value::Null);
    assert!((json["solved_value"].as_f64().expect("number") - 120.0).abs() < 1e-6);

    let updated = goal_seek(
        &model,
        &GoalSeekSpec {
            target: GoalSeekCell {
                node: "profit".into(),
                period,
            },
            target_value: AmountOrScalar::Scalar(60.0),
            target_tolerance: AmountOrScalar::Scalar(1e-7),
            driver: GoalSeekCell {
                node: "revenue".into(),
                period,
            },
            driver_tolerance: 1e-10,
            max_iterations: 100,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 200_000.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: true,
        },
        &MarketContext::new(),
    )
    .expect("goal seek should succeed");
    let text = serde_json::to_string(&updated).expect("serialize");
    let back: GoalSeekResult = serde_json::from_str(&text).expect("round trip");
    assert!(back.model.is_some());
    assert!(
        serde_json::from_str::<GoalSeekResult>(r#"{"solved_value":1.0,"model":null,"x":1}"#)
            .is_err()
    );
}
