//! Tests for NaN handling in custom functions

use finstack_quant_statements::prelude::*;

#[test]
fn test_sum_with_nan() {
    // Create a model with NaN values using division by zero
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("value1", "10.0")
        .unwrap()
        .compute("value2", "0.0 / 0.0") // Produces NaN
        .unwrap()
        .compute("value3", "20.0")
        .unwrap()
        .compute("total", "sum(value1, value2, value3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // sum should skip NaN and return 10.0 + 20.0 = 30.0
    let total = results.get("total", &PeriodId::quarter(2025, 1)).unwrap();
    assert_eq!(total, 30.0, "sum() should skip NaN values");
}

#[test]
fn test_mean_with_nan() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("value1", "10.0")
        .unwrap()
        .compute("value2", "0.0 / 0.0") // Produces NaN
        .unwrap()
        .compute("value3", "20.0")
        .unwrap()
        .compute("average", "mean(value1, value2, value3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // mean should skip NaN and return (10.0 + 20.0) / 2 = 15.0
    let average = results.get("average", &PeriodId::quarter(2025, 1)).unwrap();
    assert_eq!(average, 15.0, "mean() should skip NaN values");
}

#[test]
fn test_all_nan_values() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("value1", "0.0 / 0.0") // NaN
        .unwrap()
        .compute("value2", "0.0 / 0.0") // NaN
        .unwrap()
        .compute("sum_result", "sum(value1, value2)")
        .unwrap()
        .compute("mean_result", "mean(value1, value2)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // If all values are NaN, sum and mean should return NaN
    let sum = results
        .get("sum_result", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert!(
        sum.is_nan(),
        "sum() should return NaN when all values are NaN"
    );

    let mean = results
        .get("mean_result", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert!(
        mean.is_nan(),
        "mean() should return NaN when all values are NaN"
    );
}

#[test]
fn test_coalesce_with_nan() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("value1", "0.0 / 0.0") // NaN
        .unwrap()
        .compute("value2", "0.0") // Zero
        .unwrap()
        .compute("value3", "10.0") // Valid value
        .unwrap()
        .compute("result", "coalesce(value1, value2, value3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // coalesce skips NaN values; returns first non-NaN which is 0.0 (value2)
    let result = results.get("result", &PeriodId::quarter(2025, 1)).unwrap();
    assert_eq!(
        result, 0.0,
        "coalesce() should skip NaN but return first non-NaN (including zero)"
    );
}

#[test]
fn test_annualize_with_nan() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("quarterly_value", "0.0 / 0.0") // NaN
        .unwrap()
        .compute("annual", "annualize(quarterly_value, 4)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // annualize with NaN input should return NaN
    let annual = results.get("annual", &PeriodId::quarter(2025, 1)).unwrap();
    assert!(
        annual.is_nan(),
        "annualize() should return NaN when input is NaN"
    );
}

#[test]
fn test_ttm_with_nan() {
    // Create revenue with one NaN value in the middle
    let model = ModelBuilder::new("test")
        .periods("2024Q1..2024Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2024, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(f64::NAN)),
                (PeriodId::quarter(2024, 3), AmountOrScalar::scalar(150.0)),
                (PeriodId::quarter(2024, 4), AmountOrScalar::scalar(200.0)),
            ],
        )
        .compute("revenue_ttm", "ttm(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // TTM propagates NaN strictly — any non-finite value
    // in the trailing window makes the TTM NaN, rather than silently
    // treating a gap as zero. This is the correct behavior for a
    // trailing-twelve-months metric: a missing quarter is a material
    // signal that the metric is not well-defined and should not be
    // consumed downstream.
    let ttm = results
        .get("revenue_ttm", &PeriodId::quarter(2024, 4))
        .unwrap();
    assert!(
        ttm.is_nan(),
        "ttm() should propagate NaN strictly under C18 guard, got {ttm}"
    );
}

#[test]
fn test_mixed_operations_with_nan() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("a", "10.0")
        .unwrap()
        .compute("b", "0.0 / 0.0") // NaN
        .unwrap()
        .compute("c", "20.0")
        .unwrap()
        .compute("d", "0.0")
        .unwrap()
        // Complex formula mixing custom functions
        .compute("result", "mean(sum(a, b), coalesce(d, c))")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // sum(a, b) = 10.0 (skips NaN)
    // coalesce(d, c) = 0.0 (d is 0.0 which is non-NaN; coalesce returns first non-NaN)
    // mean(10.0, 0.0) = 5.0
    let result = results.get("result", &PeriodId::quarter(2025, 1)).unwrap();
    assert_eq!(
        result, 5.0,
        "Complex operations should handle NaN correctly"
    );
}

#[test]
fn test_sum_empty_args() {
    // Test that sum() with no arguments produces an error at compile time
    let result = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("total", "sum()");

    assert!(
        result.is_err(),
        "sum() with no arguments should produce an error"
    );
}

#[test]
fn test_mean_empty_args() {
    // Test that mean() with no arguments produces an error at compile time
    let result = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("average", "mean()");

    assert!(
        result.is_err(),
        "mean() with no arguments should produce an error"
    );
}

#[test]
fn test_coalesce_single_arg() {
    // Test that coalesce() with one argument produces an error
    let result = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .compute("value", "10.0")
        .unwrap()
        .compute("result", "coalesce(value)");

    assert!(
        result.is_err(),
        "coalesce() with one argument should produce an error"
    );
}
