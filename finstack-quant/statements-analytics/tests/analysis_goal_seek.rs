//! Goal seek integration tests.
#![allow(clippy::expect_used, clippy::panic)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec};
use finstack_quant_statements_analytics::analysis::goal_seek::goal_seek;

#[test]
fn test_goal_seek_simple_linear() {
    let period = PeriodId::quarter(2025, 1);
    let mut model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .expect("valid period")
        .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
        .compute("profit_margin", "0.15")
        .expect("valid formula")
        .compute("net_income", "revenue * profit_margin")
        .expect("valid formula")
        .build()
        .expect("valid model");

    let solved = goal_seek(
        &mut model,
        "net_income",
        period,
        20_000.0,
        "revenue",
        period,
        false,
        None,
    )
    .expect("goal seek should succeed");

    assert!((solved - 133_333.33).abs() < 1.0);
}

#[test]
fn test_goal_seek_with_update() {
    let period = PeriodId::quarter(2025, 1);
    let mut model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .expect("valid period")
        .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
        .compute("cogs", "revenue * 0.6")
        .expect("valid formula")
        .compute("gross_profit", "revenue - cogs")
        .expect("valid formula")
        .build()
        .expect("valid model");

    let solved = goal_seek(
        &mut model,
        "gross_profit",
        period,
        50_000.0,
        "revenue",
        period,
        true,
        None,
    )
    .expect("goal seek should succeed");

    assert!((solved - 125_000.0).abs() < 1.0);

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
    let q1 = PeriodId::quarter(2025, 1);
    let q4 = PeriodId::quarter(2025, 4);

    let mut model = ModelBuilder::new("test")
        .periods("2025Q1..Q4", None)
        .expect("valid period range")
        .value("revenue", &[(q1, AmountOrScalar::scalar(100_000.0))])
        .forecast("revenue", ForecastSpec::growth(0.05))
        .compute("interest_expense", "10000.0")
        .expect("valid formula")
        .compute("ebitda", "revenue * 0.3")
        .expect("valid formula")
        .compute("interest_coverage", "ebitda / interest_expense")
        .expect("valid formula")
        .build()
        .expect("valid model");

    let solved = goal_seek(
        &mut model,
        "interest_coverage",
        q4,
        2.0,
        "revenue",
        q4,
        true,
        None,
    )
    .expect("goal seek should succeed");

    assert!((solved - 66_666.67).abs() < 1.0);

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
    let period = PeriodId::quarter(2025, 1);
    let mut model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .expect("valid period")
        .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
        .build()
        .expect("valid model");

    let result = goal_seek(
        &mut model,
        "nonexistent",
        period,
        1000.0,
        "revenue",
        period,
        false,
        None,
    );

    assert!(result.is_err());
}

#[test]
fn test_goal_seek_invalid_driver_node() {
    let period = PeriodId::quarter(2025, 1);
    let mut model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .expect("valid period")
        .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
        .build()
        .expect("valid model");

    let result = goal_seek(
        &mut model,
        "revenue",
        period,
        1000.0,
        "nonexistent",
        period,
        false,
        None,
    );

    assert!(result.is_err());
}

#[test]
fn test_goal_seek_with_explicit_bounds() {
    let period = PeriodId::quarter(2025, 1);
    let mut model = ModelBuilder::new("bounds")
        .periods("2025Q1..Q1", None)
        .expect("valid period")
        .value("driver", &[(period, AmountOrScalar::scalar(0.0))])
        .compute("target", "driver")
        .expect("valid formula")
        .build()
        .expect("valid model");

    let solution = goal_seek(
        &mut model,
        "target",
        period,
        0.75,
        "driver",
        period,
        false,
        Some((0.5, 1.5)),
    )
    .expect("goal seek should succeed");

    assert!((solution - 0.75).abs() < 1e-9);
}
