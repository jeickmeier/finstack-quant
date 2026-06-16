//! Tests for min() and max() functions

use finstack_quant_statements::prelude::*;

#[test]
fn test_min_two_args() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "a",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(50.0)),
            ],
        )
        .value(
            "b",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(80.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(120.0)),
            ],
        )
        .compute("minimum", "min(a, b)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: min(100, 80) = 80
    assert_eq!(
        results.get("minimum", &PeriodId::quarter(2025, 1)).unwrap(),
        80.0
    );

    // Q2: min(50, 120) = 50
    assert_eq!(
        results.get("minimum", &PeriodId::quarter(2025, 2)).unwrap(),
        50.0
    );
}

#[test]
fn test_max_two_args() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "a",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(50.0)),
            ],
        )
        .value(
            "b",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(80.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(120.0)),
            ],
        )
        .compute("maximum", "max(a, b)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: max(100, 80) = 100
    assert_eq!(
        results.get("maximum", &PeriodId::quarter(2025, 1)).unwrap(),
        100.0
    );

    // Q2: max(50, 120) = 120
    assert_eq!(
        results.get("maximum", &PeriodId::quarter(2025, 2)).unwrap(),
        120.0
    );
}

#[test]
fn test_min_three_args() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "a",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .value(
            "b",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(80.0))],
        )
        .value(
            "c",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(120.0))],
        )
        .compute("minimum", "min(a, b, c)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // min(100, 80, 120) = 80
    assert_eq!(
        results.get("minimum", &PeriodId::quarter(2025, 1)).unwrap(),
        80.0
    );
}

#[test]
fn test_max_three_args() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "a",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .value(
            "b",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(80.0))],
        )
        .value(
            "c",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(120.0))],
        )
        .compute("maximum", "max(a, b, c)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // max(100, 80, 120) = 120
    assert_eq!(
        results.get("maximum", &PeriodId::quarter(2025, 1)).unwrap(),
        120.0
    );
}

#[test]
fn test_min_with_literal() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(1500000.0),
                ),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(800000.0)),
            ],
        )
        .compute("capped_revenue", "min(revenue, 1000000)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: min(1500000, 1000000) = 1000000 (capped)
    assert_eq!(
        results
            .get("capped_revenue", &PeriodId::quarter(2025, 1))
            .unwrap(),
        1000000.0
    );

    // Q2: min(800000, 1000000) = 800000 (below cap)
    assert_eq!(
        results
            .get("capped_revenue", &PeriodId::quarter(2025, 2))
            .unwrap(),
        800000.0
    );
}

#[test]
fn test_max_with_literal() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(80000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(150000.0)),
            ],
        )
        .compute("min_revenue", "max(revenue, 100000)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: max(80000, 100000) = 100000 (floor)
    assert_eq!(
        results
            .get("min_revenue", &PeriodId::quarter(2025, 1))
            .unwrap(),
        100000.0
    );

    // Q2: max(150000, 100000) = 150000 (above floor)
    assert_eq!(
        results
            .get("min_revenue", &PeriodId::quarter(2025, 2))
            .unwrap(),
        150000.0
    );
}

#[test]
fn test_min_single_arg() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "a",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(42.0))],
        )
        .compute("result", "min(a)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // min(42) = 42
    assert_eq!(
        results.get("result", &PeriodId::quarter(2025, 1)).unwrap(),
        42.0
    );
}

#[test]
fn test_min_max_nested() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "a",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(50.0))],
        )
        .value(
            "b",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .value(
            "c",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(75.0))],
        )
        .compute("result", "max(min(a, b), c)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // max(min(50, 100), 75) = max(50, 75) = 75
    assert_eq!(
        results.get("result", &PeriodId::quarter(2025, 1)).unwrap(),
        75.0
    );
}
