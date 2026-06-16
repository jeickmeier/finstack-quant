//! Tests for custom functions.

use finstack_quant_statements::prelude::*;

#[test]
fn test_sum_function() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "a",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(15.0)),
            ],
        )
        .value(
            "b",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(20.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(25.0)),
            ],
        )
        .value(
            "c",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(30.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(35.0)),
            ],
        )
        .compute("total", "sum(a, b, c)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: sum(10, 20, 30) = 60
    assert_eq!(
        results.get("total", &PeriodId::quarter(2025, 1)).unwrap(),
        60.0
    );

    // Q2: sum(15, 25, 35) = 75
    assert_eq!(
        results.get("total", &PeriodId::quarter(2025, 2)).unwrap(),
        75.0
    );
}

#[test]
fn test_mean_function() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "a",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(20.0)),
            ],
        )
        .value(
            "b",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(20.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(30.0)),
            ],
        )
        .value(
            "c",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(30.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(40.0)),
            ],
        )
        .compute("average", "mean(a, b, c)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: mean(10, 20, 30) = 20
    assert_eq!(
        results.get("average", &PeriodId::quarter(2025, 1)).unwrap(),
        20.0
    );

    // Q2: mean(20, 30, 40) = 30
    assert_eq!(
        results.get("average", &PeriodId::quarter(2025, 2)).unwrap(),
        30.0
    );
}

#[test]
fn test_annualize_function() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "quarterly_revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(1100.0)),
            ],
        )
        .compute("annual_revenue", "annualize(quarterly_revenue, 4)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: annualize(1000, 4) = 4000
    assert_eq!(
        results
            .get("annual_revenue", &PeriodId::quarter(2025, 1))
            .unwrap(),
        4000.0
    );

    // Q2: annualize(1100, 4) = 4400
    assert_eq!(
        results
            .get("annual_revenue", &PeriodId::quarter(2025, 2))
            .unwrap(),
        4400.0
    );
}

#[test]
fn test_coalesce_function() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q3", None)
        .unwrap()
        .value(
            "value1",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)), // Zero value
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

    // Q1: coalesce(100, 50) = 100 (value1 is non-NaN, returns first non-NaN)
    assert_eq!(
        results.get("result", &PeriodId::quarter(2025, 1)).unwrap(),
        100.0
    );

    // Q2: coalesce(0, 200) = 0 (value1 is zero but not NaN; coalesce returns first non-NaN)
    assert_eq!(
        results.get("result", &PeriodId::quarter(2025, 2)).unwrap(),
        0.0
    );

    // Q3: coalesce(300, 350) = 300 (value1 is non-NaN)
    assert_eq!(
        results.get("result", &PeriodId::quarter(2025, 3)).unwrap(),
        300.0
    );
}

#[test]
fn test_ttm_function() {
    let model = ModelBuilder::new("test")
        .periods("2024Q1..2025Q1", None)
        .unwrap()
        .value(
            "quarterly_revenue",
            &[
                (PeriodId::quarter(2024, 1), AmountOrScalar::scalar(1000.0)),
                (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(1100.0)),
                (PeriodId::quarter(2024, 3), AmountOrScalar::scalar(1200.0)),
                (PeriodId::quarter(2024, 4), AmountOrScalar::scalar(1300.0)),
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1400.0)),
            ],
        )
        .compute("ttm_revenue", "ttm(quarterly_revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // TTM requires a full 4-quarter trailing window. Partial
    // windows (Q1-Q3 2024) return NaN; full windows sum the 4 quarters.

    for partial in [
        PeriodId::quarter(2024, 1),
        PeriodId::quarter(2024, 2),
        PeriodId::quarter(2024, 3),
    ] {
        let value = results.get("ttm_revenue", &partial).unwrap();
        assert!(
            value.is_nan(),
            "partial TTM at {partial} should be NaN under C18 guard, got {value}"
        );
    }

    // 2024Q4: 4 quarters = 1000 + 1100 + 1200 + 1300 = 4600
    let q4_ttm = results
        .get("ttm_revenue", &PeriodId::quarter(2024, 4))
        .unwrap();
    assert_eq!(q4_ttm, 4600.0);

    // 2025Q1: 1100 + 1200 + 1300 + 1400 = 5000
    let q1_2025_ttm = results
        .get("ttm_revenue", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert_eq!(q1_2025_ttm, 5000.0);
}

#[test]
fn test_complex_custom_functions() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(1200.0)),
            ],
        )
        .value(
            "cogs",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(600.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(700.0)),
            ],
        )
        .value(
            "opex",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(200.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(250.0)),
            ],
        )
        // Complex calculation using custom functions
        .compute("total_costs", "sum(cogs, opex)")
        .unwrap()
        .compute("avg_unit_cost", "mean(cogs, opex)")
        .unwrap()
        .compute("profit", "revenue - total_costs")
        .unwrap()
        .compute("annualized_profit", "annualize(profit, 4)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1 calculations
    assert_eq!(
        results
            .get("total_costs", &PeriodId::quarter(2025, 1))
            .unwrap(),
        800.0 // 600 + 200
    );
    assert_eq!(
        results
            .get("avg_unit_cost", &PeriodId::quarter(2025, 1))
            .unwrap(),
        400.0 // (600 + 200) / 2
    );
    assert_eq!(
        results.get("profit", &PeriodId::quarter(2025, 1)).unwrap(),
        200.0 // 1000 - 800
    );
    assert_eq!(
        results
            .get("annualized_profit", &PeriodId::quarter(2025, 1))
            .unwrap(),
        800.0 // 200 * 4
    );

    // Q2 calculations
    assert_eq!(
        results
            .get("total_costs", &PeriodId::quarter(2025, 2))
            .unwrap(),
        950.0 // 700 + 250
    );
    assert_eq!(
        results
            .get("avg_unit_cost", &PeriodId::quarter(2025, 2))
            .unwrap(),
        475.0 // (700 + 250) / 2
    );
    assert_eq!(
        results.get("profit", &PeriodId::quarter(2025, 2)).unwrap(),
        250.0 // 1200 - 950
    );
    assert_eq!(
        results
            .get("annualized_profit", &PeriodId::quarter(2025, 2))
            .unwrap(),
        1000.0 // 250 * 4
    );
}
