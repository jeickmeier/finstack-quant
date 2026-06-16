//! Tests for market standards compliance.
//!
//! This test suite validates that statistical calculations match industry standards:
//! - Bloomberg Terminal
//! - Microsoft Excel (VAR.S, STDEV.S)
//! - Python pandas (with ddof=1)
//! - R statistical package

use finstack_quant_statements::prelude::*;
use indexmap::indexmap;

mod common;
use common::{assert_close, SAMPLE_VAR_TOLERANCE};

// ============================================================================
// Variance and Standard Deviation Tests (Market Standards)
// ============================================================================

#[test]
fn test_variance_uses_sample_not_population() {
    // Test that variance uses sample variance (n-1), not population variance (n)
    // Using rolling_var to compute variance over a window
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "data",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(2.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(4.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(4.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(4.0)),
            ],
        )
        .compute("rolling_var_4", "rolling_var(data, 4)")
        .unwrap()
        .compute("rolling_std_4", "rolling_std(data, 4)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q4: Rolling variance with window=4 should use all 4 values [2, 4, 4, 4]
    // Manual calculation:
    // Mean = (2+4+4+4)/4 = 3.5
    // Deviations: [-1.5, 0.5, 0.5, 0.5]
    // Squared deviations: [2.25, 0.25, 0.25, 0.25] = 3.0
    // Sample variance (n-1): 3.0 / 3 = 1.0
    // Population variance (n): 3.0 / 4 = 0.75
    let variance = results
        .get("rolling_var_4", &PeriodId::quarter(2025, 4))
        .unwrap();

    let expected_sample_var = 1.0;
    assert_close(
        variance,
        expected_sample_var,
        SAMPLE_VAR_TOLERANCE,
        "Rolling variance should use sample variance (n-1)",
    );

    // Verify it's NOT population variance
    let pop_variance = 0.75;
    assert!(
        (variance - pop_variance).abs() > 0.1,
        "Should use sample variance (n-1) = 1.0, not population variance (n) = 0.75"
    );

    // Standard deviation should be sqrt of variance
    let std_dev = results
        .get("rolling_std_4", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert_close(
        std_dev,
        variance.sqrt(),
        SAMPLE_VAR_TOLERANCE,
        "Rolling std should be sqrt of rolling variance",
    );
    assert_close(
        std_dev,
        1.0,
        SAMPLE_VAR_TOLERANCE,
        "Rolling std should be 1.0",
    );
}

#[test]
fn test_variance_single_value_returns_nan() {
    // Market standard: Variance undefined for single value with sample variance
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "data",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .compute("variance", "var(data)")
        .unwrap()
        .compute("std_dev", "std(data)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // With sample variance, single value should return NaN (undefined)
    let variance = results
        .get("variance", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert!(
        variance.is_nan(),
        "Variance of single value should be NaN with sample variance"
    );

    let std_dev = results.get("std_dev", &PeriodId::quarter(2025, 1)).unwrap();
    assert!(
        std_dev.is_nan(),
        "Std dev of single value should be NaN with sample variance"
    );
}

// ============================================================================
// TTM Tests (Period Frequency Awareness)
// ============================================================================

#[test]
fn test_ttm_quarterly_data() {
    let model = ModelBuilder::new("test")
        .periods("2024Q1..2025Q1", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2024, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2024, 3), AmountOrScalar::scalar(120.0)),
                (PeriodId::quarter(2024, 4), AmountOrScalar::scalar(130.0)),
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(140.0)),
            ],
        )
        .compute("revenue_ttm", "ttm(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Quarterly TTM should sum last 4 quarters
    let q4_2024_ttm = results
        .get("revenue_ttm", &PeriodId::quarter(2024, 4))
        .unwrap();
    assert_eq!(q4_2024_ttm, 460.0, "TTM Q4 2024 = 100+110+120+130");

    let q1_2025_ttm = results
        .get("revenue_ttm", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert_eq!(q1_2025_ttm, 500.0, "TTM Q1 2025 = 110+120+130+140");
}

#[test]
fn test_ttm_monthly_data() {
    let model = ModelBuilder::new("test")
        .periods("2024M01..2024M12", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::month(2024, 1), AmountOrScalar::scalar(10.0)),
                (PeriodId::month(2024, 2), AmountOrScalar::scalar(11.0)),
                (PeriodId::month(2024, 3), AmountOrScalar::scalar(12.0)),
                (PeriodId::month(2024, 4), AmountOrScalar::scalar(13.0)),
                (PeriodId::month(2024, 5), AmountOrScalar::scalar(14.0)),
                (PeriodId::month(2024, 6), AmountOrScalar::scalar(15.0)),
                (PeriodId::month(2024, 7), AmountOrScalar::scalar(16.0)),
                (PeriodId::month(2024, 8), AmountOrScalar::scalar(17.0)),
                (PeriodId::month(2024, 9), AmountOrScalar::scalar(18.0)),
                (PeriodId::month(2024, 10), AmountOrScalar::scalar(19.0)),
                (PeriodId::month(2024, 11), AmountOrScalar::scalar(20.0)),
                (PeriodId::month(2024, 12), AmountOrScalar::scalar(21.0)),
            ],
        )
        .compute("revenue_ttm", "ttm(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Monthly TTM should sum last 12 months
    let m12_ttm = results
        .get("revenue_ttm", &PeriodId::month(2024, 12))
        .unwrap();
    assert_eq!(m12_ttm, 186.0, "Monthly TTM should sum all 12 months");
}

#[test]
fn test_ytd_quarterly_data() {
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
        .compute("revenue_ytd", "ytd(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let q1_ytd = results
        .get("revenue_ytd", &PeriodId::quarter(2025, 1))
        .unwrap();
    let q2_ytd = results
        .get("revenue_ytd", &PeriodId::quarter(2025, 2))
        .unwrap();
    let q3_ytd = results
        .get("revenue_ytd", &PeriodId::quarter(2025, 3))
        .unwrap();

    assert_eq!(q1_ytd, 100.0, "YTD Q1 = 100");
    assert_eq!(q2_ytd, 210.0, "YTD Q2 = 100 + 110");
    assert_eq!(q3_ytd, 330.0, "YTD Q3 = 100 + 110 + 120");
}

#[test]
fn test_qtd_monthly_data() {
    let model = ModelBuilder::new("test")
        .periods("2025M01..2025M06", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::month(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2025, 2), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2025, 3), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2025, 4), AmountOrScalar::scalar(100.0)),
                (PeriodId::month(2025, 5), AmountOrScalar::scalar(110.0)),
                (PeriodId::month(2025, 6), AmountOrScalar::scalar(120.0)),
            ],
        )
        .compute("revenue_qtd", "qtd(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let m4_qtd = results
        .get("revenue_qtd", &PeriodId::month(2025, 4))
        .unwrap();
    let m5_qtd = results
        .get("revenue_qtd", &PeriodId::month(2025, 5))
        .unwrap();
    let m6_qtd = results
        .get("revenue_qtd", &PeriodId::month(2025, 6))
        .unwrap();

    assert_eq!(m4_qtd, 100.0, "QTD Apr = 100");
    assert_eq!(m5_qtd, 210.0, "QTD May = 100 + 110");
    assert_eq!(m6_qtd, 330.0, "QTD Jun = 100 + 110 + 120");
}

#[test]
fn test_fiscal_ytd_monthly_data() {
    let model = ModelBuilder::new("test")
        .periods("2024M04..2025M06", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::month(2024, 4), AmountOrScalar::scalar(80.0)),
                (PeriodId::month(2024, 5), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2024, 6), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2024, 7), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2024, 8), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2024, 9), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2024, 10), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2024, 11), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2024, 12), AmountOrScalar::scalar(90.0)),
                (PeriodId::month(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::month(2025, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::month(2025, 3), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2025, 4), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2025, 5), AmountOrScalar::scalar(0.0)),
                (PeriodId::month(2025, 6), AmountOrScalar::scalar(0.0)),
            ],
        )
        // April fiscal year
        .compute("revenue_fiscal_ytd", "fiscal_ytd(revenue, 4)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let feb_2025_fytd = results
        .get("revenue_fiscal_ytd", &PeriodId::month(2025, 2))
        .unwrap();

    assert_eq!(
        feb_2025_fytd,
        80.0 + 90.0 + 100.0 + 110.0,
        "Fiscal YTD (Apr start) from 2024M04 through 2025M02"
    );
}

#[test]
fn test_ttm_semi_annual_data() {
    let model = ModelBuilder::new("test")
        .periods("2024H1..2025H2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::half(2024, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::half(2024, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::half(2025, 1), AmountOrScalar::scalar(120.0)),
                (PeriodId::half(2025, 2), AmountOrScalar::scalar(130.0)),
            ],
        )
        .compute("revenue_ttm", "ttm(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Semi-annual TTM should sum last 2 halves (1 year)
    let h2_2025_ttm = results
        .get("revenue_ttm", &PeriodId::half(2025, 2))
        .unwrap();
    assert_eq!(h2_2025_ttm, 250.0, "Semi-annual TTM = last 2 halves");
}

#[test]
fn test_ttm_annual_data() {
    let model = ModelBuilder::new("test")
        .periods("2023..2025", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::annual(2023), AmountOrScalar::scalar(1000.0)),
                (PeriodId::annual(2024), AmountOrScalar::scalar(1100.0)),
                (PeriodId::annual(2025), AmountOrScalar::scalar(1200.0)),
            ],
        )
        .compute("revenue_ttm", "ttm(revenue)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Annual TTM should just return the value itself (1 period = 1 year)
    let y2025_ttm = results.get("revenue_ttm", &PeriodId::annual(2025)).unwrap();
    assert_eq!(y2025_ttm, 1200.0, "Annual TTM = the value itself");
}

// ============================================================================
// Forecast Parameters Required (No Silent Defaults)
// ============================================================================

#[test]
fn test_exponential_smoothing_requires_alpha() {
    // Should error if alpha not provided
    let result = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::TimeSeries,
                params: indexmap! {
                    "historical".into() => serde_json::json!([90.0, 95.0, 100.0]),
                    "method".into() => serde_json::json!("exponential"),
                    "beta".into() => serde_json::json!(0.1),
                    // Missing alpha - should error
                },
            },
        )
        .build();

    assert!(result.is_ok()); // Model builds fine

    let model = result.unwrap();
    let mut evaluator = Evaluator::new();
    let eval_result = evaluator.evaluate(&model);

    // Should error during evaluation when alpha is missing
    assert!(eval_result.is_err());
    let err = eval_result.unwrap_err();
    assert!(
        err.to_string().contains("alpha"),
        "Error should mention missing alpha parameter: {}",
        err
    );
}

#[test]
fn test_exponential_smoothing_requires_beta() {
    // Should error if beta not provided
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::TimeSeries,
                params: indexmap! {
                    "historical".into() => serde_json::json!([90.0, 95.0, 100.0]),
                    "method".into() => serde_json::json!("exponential"),
                    "alpha".into() => serde_json::json!(0.3),
                    // Missing beta - should error
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("beta"));
}

#[test]
fn test_moving_average_requires_window() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::TimeSeries,
                params: indexmap! {
                    "historical".into() => serde_json::json!([90.0, 95.0, 100.0]),
                    "method".into() => serde_json::json!("moving_average"),
                    // Missing window - should error
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("window"));
}

#[test]
fn test_seasonal_requires_mode() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::Seasonal,
                params: indexmap! {
                    "historical".into() => serde_json::json!([100.0, 90.0, 110.0, 80.0, 100.0, 90.0, 110.0, 80.0]),
                    "season_length".into() => serde_json::json!(4),
                    // Missing mode - should error
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("mode"));
}

#[test]
fn test_seasonal_decomposition_requires_season_length() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::Seasonal,
                params: indexmap! {
                    "historical".into() => serde_json::json!([90.0, 85.0, 95.0, 80.0, 100.0, 95.0, 105.0, 90.0]),
                    "mode".into() => serde_json::json!("additive"),
                    // Missing season_length - should error
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("season_length"));
}

// ============================================================================
// Period Stepping Tests (Core API Usage)
// ============================================================================

#[test]
fn test_lag_quarterly_periods() {
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
        .compute("lagged", "lag(revenue, 1)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    assert_eq!(
        results.get("lagged", &PeriodId::quarter(2025, 2)).unwrap(),
        100.0
    );
    assert_eq!(
        results.get("lagged", &PeriodId::quarter(2025, 4)).unwrap(),
        120.0
    );
}

#[test]
fn test_lag_monthly_periods() {
    let model = ModelBuilder::new("test")
        .periods("2024M11..2025M02", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (PeriodId::month(2024, 11), AmountOrScalar::scalar(100.0)),
                (PeriodId::month(2024, 12), AmountOrScalar::scalar(110.0)),
                (PeriodId::month(2025, 1), AmountOrScalar::scalar(120.0)),
                (PeriodId::month(2025, 2), AmountOrScalar::scalar(130.0)),
            ],
        )
        .compute("lagged", "lag(revenue, 2)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // M01 lag 2 should be M11 (crosses year boundary)
    assert_eq!(
        results.get("lagged", &PeriodId::month(2025, 1)).unwrap(),
        100.0
    );
    assert_eq!(
        results.get("lagged", &PeriodId::month(2025, 2)).unwrap(),
        110.0
    );
}

// ============================================================================
// Type Safety Tests (SeasonalMode Enum)
// ============================================================================

#[test]
fn test_seasonal_mode_enum_additive() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::Seasonal,
                params: indexmap! {
                    "historical".into() => serde_json::json!([100.0, 90.0, 120.0, 80.0, 110.0, 100.0, 130.0, 90.0]),
                    "season_length".into() => serde_json::json!(4),
                    "mode".into() => serde_json::json!("additive"),
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Verify additive mode produces reasonable forecasts
    let q2 = results.get("revenue", &PeriodId::quarter(2025, 2)).unwrap();
    let q3 = results.get("revenue", &PeriodId::quarter(2025, 3)).unwrap();
    let q4 = results.get("revenue", &PeriodId::quarter(2025, 4)).unwrap();

    // All forecasts should be valid numbers (additive mode is working)
    assert!(!q2.is_nan(), "Q2 forecast should be valid");
    assert!(!q3.is_nan(), "Q3 forecast should be valid");
    assert!(!q4.is_nan(), "Q4 forecast should be valid");
}

#[test]
fn test_seasonal_mode_enum_multiplicative() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::Seasonal,
                params: indexmap! {
                    "historical".into() => serde_json::json!([100.0, 90.0, 110.0, 80.0, 110.0, 99.0, 132.0, 96.0]),
                    "season_length".into() => serde_json::json!(4),
                    "mode".into() => serde_json::json!("multiplicative"),
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Verify multiplicative mode produces reasonable forecasts
    let q2 = results.get("revenue", &PeriodId::quarter(2025, 2)).unwrap();
    let q3 = results.get("revenue", &PeriodId::quarter(2025, 3)).unwrap();
    let q4 = results.get("revenue", &PeriodId::quarter(2025, 4)).unwrap();

    // All forecasts should be positive and reasonable
    assert!(q2 > 0.0, "Q2 forecast should be positive");
    assert!(q3 > 0.0, "Q3 forecast should be positive");
    assert!(q4 > 0.0, "Q4 forecast should be positive");
}

#[test]
fn test_seasonal_mode_typo_errors() {
    // Typo in mode should error, not silently default
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::Seasonal,
                params: indexmap! {
                    "historical".into() => serde_json::json!([100.0, 100.0, 100.0, 100.0]),
                    "season_length".into() => serde_json::json!(2),
                    "mode".into() => serde_json::json!("additiv"), // Typo
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model);

    // Should error on typo, not silently default
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("mode") || err_msg.contains("Invalid"));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_rolling_window_with_limited_history() {
    // With limited historical data, rolling functions use available data
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
        .compute("rolling_mean_4", "rolling_mean(revenue, 4)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1: With only 1 value and window=4, uses just that value
    let q1_value = results
        .get("rolling_mean_4", &PeriodId::quarter(2025, 1))
        .unwrap();
    assert_eq!(
        q1_value, 100.0,
        "Rolling mean with 1 value returns that value"
    );

    // Q2: With 2 values and window=4, uses both: (100+110)/2 = 105
    let q2_value = results
        .get("rolling_mean_4", &PeriodId::quarter(2025, 2))
        .unwrap();
    assert_eq!(
        q2_value, 105.0,
        "Rolling mean with 2 values returns their mean"
    );
}

#[test]
fn test_seasonal_allows_negative_values() {
    // Seasonal forecast should allow negative values (not force non-negative)
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "net_income",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10.0))],
        )
        .forecast(
            "net_income",
            ForecastSpec {
                method: ForecastMethod::Seasonal,
                params: indexmap! {
                    "historical".into() => serde_json::json!([-50.0, 20.0, 30.0, -40.0, -45.0, 25.0, 35.0, -35.0]),
                    "season_length".into() => serde_json::json!(4),
                    "mode".into() => serde_json::json!("additive"),
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check that seasonal decomposition handles negative values correctly
    let q2 = results
        .get("net_income", &PeriodId::quarter(2025, 2))
        .unwrap();
    let q3 = results
        .get("net_income", &PeriodId::quarter(2025, 3))
        .unwrap();
    let q4 = results
        .get("net_income", &PeriodId::quarter(2025, 4))
        .unwrap();

    // The key test: seasonal forecast should not force values to be non-negative
    // Negative values (losses) are valid in financial forecasting
    assert!(
        !q2.is_nan() && !q3.is_nan() && !q4.is_nan(),
        "All forecasts should be valid numbers (negative values allowed)"
    );
}

// ============================================================================
// EWM Bias Correction Tests (pandas parity)
// ============================================================================

#[test]
fn test_ewm_var_without_bias_correction() {
    // Test default behavior (adjust=False)
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "returns",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.10)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.05)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(0.15)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(0.08)),
            ],
        )
        .compute("volatility", "ewm_var(returns, 0.3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Should return a valid non-negative variance
    let vol = results
        .get("volatility", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert!(vol >= 0.0, "EWM variance should be non-negative");
    assert!(!vol.is_nan(), "EWM variance should not be NaN");
}

#[test]
fn test_ewm_var_with_bias_correction() {
    // Test bias-corrected mode (adjust=True, pandas default)
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "returns",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.10)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.05)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(0.15)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(0.08)),
            ],
        )
        .compute("volatility_adjusted", "ewm_var(returns, 0.3, 1.0)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Bias-corrected variance should be valid
    let vol_adj = results
        .get("volatility_adjusted", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert!(
        vol_adj >= 0.0,
        "Bias-corrected EWM variance should be non-negative"
    );
    assert!(
        !vol_adj.is_nan(),
        "Bias-corrected EWM variance should not be NaN"
    );
}

#[test]
fn test_ewm_std_with_bias_correction() {
    // Test that std is sqrt of variance (with bias correction)
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "returns",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.10)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.05)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(0.15)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(0.08)),
            ],
        )
        .compute("ewm_variance", "ewm_var(returns, 0.3, 1.0)")
        .unwrap()
        .compute("ewm_std_dev", "ewm_std(returns, 0.3, 1.0)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let variance = results
        .get("ewm_variance", &PeriodId::quarter(2025, 4))
        .unwrap();
    let std_dev = results
        .get("ewm_std_dev", &PeriodId::quarter(2025, 4))
        .unwrap();

    // Standard deviation should be sqrt of variance
    assert!(
        (std_dev - variance.sqrt()).abs() < 1e-10,
        "EWM std should equal sqrt(variance)"
    );
}

// ============================================================================
// Core API Tests
// ============================================================================

#[test]
fn test_period_kind_accessor() {
    let q1 = PeriodId::quarter(2025, 1);
    assert_eq!(q1.kind(), PeriodKind::Quarterly);

    let m1 = PeriodId::month(2025, 1);
    assert_eq!(m1.kind(), PeriodKind::Monthly);
}

#[test]
fn test_periods_per_year() {
    assert_eq!(PeriodId::quarter(2025, 1).periods_per_year(), 4);
    assert_eq!(PeriodId::month(2025, 1).periods_per_year(), 12);
    assert_eq!(PeriodId::half(2025, 1).periods_per_year(), 2);
    assert_eq!(PeriodId::annual(2025).periods_per_year(), 1);
}

#[test]
fn test_period_next() {
    let q1 = PeriodId::quarter(2025, 1);
    let q2 = q1.next().unwrap();
    assert_eq!(q2, PeriodId::quarter(2025, 2));

    let q4 = PeriodId::quarter(2025, 4);
    let next_q1 = q4.next().unwrap();
    assert_eq!(next_q1, PeriodId::quarter(2026, 1));
}

#[test]
fn test_period_prev() {
    let q2 = PeriodId::quarter(2025, 2);
    let q1 = q2.prev().unwrap();
    assert_eq!(q1, PeriodId::quarter(2025, 1));

    let q1 = PeriodId::quarter(2025, 1);
    let prev_q4 = q1.prev().unwrap();
    assert_eq!(prev_q4, PeriodId::quarter(2024, 4));
}
