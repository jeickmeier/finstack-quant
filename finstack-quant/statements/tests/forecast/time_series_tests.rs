//! Tests for time-series functions.

use finstack_quant_statements::prelude::*;

#[test]
fn test_lag_function() {
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
        .compute("lagged_revenue", "lag(revenue, 1)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: lag(100, 1) should be NaN (no prior period)
    assert!(results
        .get("lagged_revenue", &PeriodId::quarter(2025, 1))
        .unwrap()
        .is_nan());

    // Q2: lag(110, 1) should be 100 (Q1 value)
    assert_eq!(
        results
            .get("lagged_revenue", &PeriodId::quarter(2025, 2))
            .unwrap(),
        100.0
    );

    // Q3: lag(120, 1) should be 110 (Q2 value)
    assert_eq!(
        results
            .get("lagged_revenue", &PeriodId::quarter(2025, 3))
            .unwrap(),
        110.0
    );

    // Q4: lag(130, 1) should be 120 (Q3 value)
    assert_eq!(
        results
            .get("lagged_revenue", &PeriodId::quarter(2025, 4))
            .unwrap(),
        120.0
    );
}

#[test]
fn test_lag_multiple_periods() {
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
        .compute("lagged_revenue_2", "lag(revenue, 2)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1 and Q2: lag(_, 2) should be NaN (no prior periods)
    assert!(results
        .get("lagged_revenue_2", &PeriodId::quarter(2025, 1))
        .unwrap()
        .is_nan());
    assert!(results
        .get("lagged_revenue_2", &PeriodId::quarter(2025, 2))
        .unwrap()
        .is_nan());

    // Q3: lag(120, 2) should be 100 (Q1 value)
    assert_eq!(
        results
            .get("lagged_revenue_2", &PeriodId::quarter(2025, 3))
            .unwrap(),
        100.0
    );

    // Q4: lag(130, 2) should be 110 (Q2 value)
    assert_eq!(
        results
            .get("lagged_revenue_2", &PeriodId::quarter(2025, 4))
            .unwrap(),
        110.0
    );
}

#[test]
fn test_diff_function() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(125.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(135.0)),
            ],
        )
        .compute("revenue_change", "diff(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: diff(100) = NaN (no prior period)
    assert!(results
        .get("revenue_change", &PeriodId::quarter(2025, 1))
        .unwrap()
        .is_nan());

    // Q2: diff(110) = 110 - 100 = 10
    assert_eq!(
        results
            .get("revenue_change", &PeriodId::quarter(2025, 2))
            .unwrap(),
        10.0
    );

    // Q3: diff(125) = 125 - 110 = 15
    assert_eq!(
        results
            .get("revenue_change", &PeriodId::quarter(2025, 3))
            .unwrap(),
        15.0
    );

    // Q4: diff(135) = 135 - 125 = 10
    assert_eq!(
        results
            .get("revenue_change", &PeriodId::quarter(2025, 4))
            .unwrap(),
        10.0
    );
}

#[test]
fn test_pct_change_function() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(121.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(133.1)),
            ],
        )
        .compute("revenue_growth", "pct_change(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: pct_change(100) = NaN (no prior period)
    assert!(results
        .get("revenue_growth", &PeriodId::quarter(2025, 1))
        .unwrap()
        .is_nan());

    // Q2: pct_change(110) = (110 - 100) / 100 = 0.10
    assert_eq!(
        results
            .get("revenue_growth", &PeriodId::quarter(2025, 2))
            .unwrap(),
        0.10
    );

    // Q3: pct_change(121) = (121 - 110) / 110 = 0.10
    assert_eq!(
        results
            .get("revenue_growth", &PeriodId::quarter(2025, 3))
            .unwrap(),
        0.10
    );

    // Q4: pct_change(133.1) = (133.1 - 121) / 121 ≈ 0.10
    let q4_growth = results
        .get("revenue_growth", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert!((q4_growth - 0.10).abs() < 0.001);
}

#[test]
fn test_yoy_growth() {
    let model = ModelBuilder::new("test")
        .periods("2024Q1..2025Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                // 2024
                (PeriodId::quarter(2024, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2024, 3), AmountOrScalar::scalar(120.0)),
                (PeriodId::quarter(2024, 4), AmountOrScalar::scalar(130.0)),
                // 2025
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(105.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(121.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(138.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(156.0)),
            ],
        )
        .compute("yoy_growth", "pct_change(revenue, 4)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // 2025Q1: (105 - 100) / 100 = 0.05
    assert_eq!(
        results
            .get("yoy_growth", &PeriodId::quarter(2025, 1))
            .unwrap(),
        0.05
    );

    // 2025Q2: (121 - 110) / 110 = 0.10
    assert_eq!(
        results
            .get("yoy_growth", &PeriodId::quarter(2025, 2))
            .unwrap(),
        0.10
    );

    // 2025Q3: (138 - 120) / 120 = 0.15
    assert_eq!(
        results
            .get("yoy_growth", &PeriodId::quarter(2025, 3))
            .unwrap(),
        0.15
    );

    // 2025Q4: (156 - 130) / 130 = 0.20
    assert_eq!(
        results
            .get("yoy_growth", &PeriodId::quarter(2025, 4))
            .unwrap(),
        0.20
    );
}

// Note: lead() function is intentionally not supported in financial modeling
// to prevent forward-looking bias in time series analysis

#[test]
fn test_complex_time_series() {
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
        .compute("qoq_growth", "pct_change(revenue, 1)")
        .unwrap()
        .compute("revenue_acceleration", "diff(qoq_growth)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check QoQ growth calculations
    // Q1: NaN since there's no prior period
    let q1_qoq = results.get("qoq_growth", &PeriodId::quarter(2025, 1));
    assert!(q1_qoq.is_some() && q1_qoq.unwrap().is_nan());

    // Q2: (110-100)/100 = 0.10
    assert_eq!(
        results
            .get("qoq_growth", &PeriodId::quarter(2025, 2))
            .unwrap(),
        0.10
    );

    // Q3 growth: (120-110)/110 ≈ 0.0909
    let q3_growth = results
        .get("qoq_growth", &PeriodId::quarter(2025, 3))
        .unwrap();
    assert!((q3_growth - 0.0909).abs() < 0.001);

    // Check revenue acceleration (second derivative)
    // Q1: NaN since qoq_growth is NaN
    let q1_accel = results.get("revenue_acceleration", &PeriodId::quarter(2025, 1));
    assert!(q1_accel.is_some() && q1_accel.unwrap().is_nan());

    // Q2: NaN since there's no prior QoQ growth to diff against
    let q2_accel = results.get("revenue_acceleration", &PeriodId::quarter(2025, 2));
    assert!(q2_accel.is_some() && q2_accel.unwrap().is_nan());

    // Q3 acceleration: 0.0909 - 0.10 ≈ -0.0091
    let q3_accel = results
        .get("revenue_acceleration", &PeriodId::quarter(2025, 3))
        .unwrap();
    assert!((q3_accel - (-0.0091)).abs() < 0.001);
}
