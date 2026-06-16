//! Tests to verify that all previously incomplete features are now fully functional.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::prelude::*;
use indexmap::indexmap;

#[test]
fn test_advanced_statistical_functions_work() {
    // Build a model with historical data
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
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
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(105_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(115_000.0),
                ),
            ],
        )
        // Test Rank function
        .compute("revenue_rank", "rank(revenue)")
        .unwrap()
        // Test Quantile function (median)
        .compute("revenue_median", "quantile(revenue, 0.5)")
        .unwrap()
        // Test EWM functions
        .compute("revenue_ewm", "ewm_mean(revenue, 0.3)")
        .unwrap()
        .compute("revenue_ewm_std", "ewm_std(revenue, 0.3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check rank function works (should not be 0)
    let q4_rank = results
        .get("revenue_rank", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert!(q4_rank > 0.0, "Rank function should return non-zero value");
    assert!(q4_rank <= 4.0, "Rank should be within range");

    // Check quantile function works
    let q4_median = results
        .get("revenue_median", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert!(
        q4_median > 0.0,
        "Quantile function should return non-zero value"
    );
    assert!(
        (100_000.0..=115_000.0).contains(&q4_median),
        "Median should be in range"
    );

    // Check EWM functions work
    let q4_ewm = results
        .get("revenue_ewm", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert!(q4_ewm > 0.0, "EWM mean should return non-zero value");

    let q4_ewm_std = results
        .get("revenue_ewm_std", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert!(q4_ewm_std >= 0.0, "EWM std should be non-negative");
}

#[test]
fn test_timeseries_forecast_with_trend_detection() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value("sales", &[
            (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100_000.0)),
        ])
        .forecast("sales", ForecastSpec {
            method: ForecastMethod::TimeSeries,
            params: indexmap! {
                "historical".into() => serde_json::json!([80_000, 85_000, 90_000, 95_000, 100_000]),
                "method".into() => serde_json::json!("linear"),
            },
        })
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Check that trend was detected and applied
    let q2_sales = results.get("sales", &PeriodId::quarter(2025, 2)).unwrap();
    let q3_sales = results.get("sales", &PeriodId::quarter(2025, 3)).unwrap();
    let q4_sales = results.get("sales", &PeriodId::quarter(2025, 4)).unwrap();

    // Linear trend should continue
    assert!(q2_sales > 100_000.0, "Q2 should show growth");
    assert!(q3_sales > q2_sales, "Q3 should be higher than Q2");
    assert!(q4_sales > q3_sales, "Q4 should be higher than Q3");

    // Check that the trend is approximately linear
    let q2_growth = q2_sales - 100_000.0;
    let q3_growth = q3_sales - q2_sales;
    let q4_growth = q4_sales - q3_sales;
    assert!(
        (q2_growth - q3_growth).abs() < 100.0,
        "Growth should be approximately linear"
    );
    assert!(
        (q3_growth - q4_growth).abs() < 100.0,
        "Growth should be approximately linear"
    );
}

#[test]
fn test_seasonal_forecast_with_decomposition() {
    // Create historical data with clear seasonal pattern
    let historical = vec![
        100.0, 90.0, 110.0, 85.0, // Year 1: Q1=100, Q2=90, Q3=110, Q4=85
        105.0, 95.0, 115.0, 90.0, // Year 2: slight growth
        110.0, 100.0, 120.0, 95.0, // Year 3: continued growth
    ];

    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "seasonal_sales",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(115.0))],
        )
        .forecast(
            "seasonal_sales",
            ForecastSpec {
                method: ForecastMethod::Seasonal,
                params: indexmap! {
                    "historical".into() => serde_json::json!(historical),
                    "season_length".into() => serde_json::json!(4),
                    "growth".into() => serde_json::json!(0.02),  // 2% growth
                    "mode".into() => serde_json::json!("additive"),
                },
            },
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q1 is actual, Q2-Q4 are forecast
    let q1 = results
        .get("seasonal_sales", &PeriodId::quarter(2025, 1))
        .unwrap();
    let q2 = results
        .get("seasonal_sales", &PeriodId::quarter(2025, 2))
        .unwrap();
    let q3 = results
        .get("seasonal_sales", &PeriodId::quarter(2025, 3))
        .unwrap();
    let q4 = results
        .get("seasonal_sales", &PeriodId::quarter(2025, 4))
        .unwrap();

    // Check that seasonal pattern produces variation
    assert!(
        q1 != q2 || q2 != q3 || q3 != q4,
        "Seasonal pattern should create variation"
    );

    // Check that at least some seasonal differences exist
    let values = [q1, q2, q3, q4];
    let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    assert!(max > min, "There should be seasonal variation");

    // All values should be positive
    assert!(
        q1 > 0.0 && q2 > 0.0 && q3 > 0.0 && q4 > 0.0,
        "All values should be positive"
    );
}

#[test]
fn test_all_features_integrated() {
    // Build a comprehensive model using all new features
    let model = ModelBuilder::new("comprehensive")
        .periods("2025Q1..2025Q4", Some("2025Q2"))
        .unwrap()
        // Historical revenue with seasonality
        .value("revenue", &[
            (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100_000.0)),
            (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(90_000.0)),
        ])
        // Seasonal forecast with decomposition
        .forecast("revenue", ForecastSpec {
            method: ForecastMethod::Seasonal,
            params: indexmap! {
                "historical".into() => serde_json::json!([100_000, 90_000, 110_000, 85_000, 105_000, 95_000, 115_000, 90_000]),
                "season_length".into() => serde_json::json!(4),
                "mode".into() => serde_json::json!("multiplicative"),
                "growth".into() => serde_json::json!(0.02),
            },
        })
        // Cost with time-series forecast
        .value("costs", &[
            (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60_000.0)),
            (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(55_000.0)),
        ])
        .forecast("costs", ForecastSpec {
            method: ForecastMethod::TimeSeries,
            params: indexmap! {
                "historical".into() => serde_json::json!([50_000, 52_000, 55_000, 58_000, 60_000]),
                "method".into() => serde_json::json!("exponential"),
                "alpha".into() => serde_json::json!(0.3),
                "beta".into() => serde_json::json!(0.1),
            },
        })
        // Statistical calculations
        .compute("profit", "revenue - costs").unwrap()
        .compute("profit_rank", "rank(profit)").unwrap()
        .compute("profit_75th", "quantile(profit, 0.75)").unwrap()
        .compute("profit_ewm", "ewm_mean(profit, 0.2)").unwrap()
        // Margins
        .compute("margin", "profit / revenue").unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Verify all features produced meaningful results
    for quarter in 3..=4 {
        let period = PeriodId::quarter(2025, quarter as u8);

        // Seasonal forecast should work
        let revenue = results.get("revenue", &period).unwrap();
        assert!(
            revenue > 0.0,
            "Seasonal forecast should produce positive revenue"
        );

        // Time-series forecast should work
        let costs = results.get("costs", &period).unwrap();
        assert!(
            costs > 0.0,
            "Time-series forecast should produce positive costs"
        );

        // Statistical functions should work
        let rank = results.get("profit_rank", &period).unwrap();
        assert!(rank > 0.0, "Rank should be positive");

        let quantile = results.get("profit_75th", &period).unwrap();
        assert!(quantile >= 0.0, "Quantile should be non-negative");

        let ewm = results.get("profit_ewm", &period).unwrap();
        assert!(!ewm.is_nan(), "EWM should not be NaN");
    }

    // Verify seasonality is preserved in revenue
    let q2_revenue = results.get("revenue", &PeriodId::quarter(2025, 2)).unwrap();
    let q3_revenue = results.get("revenue", &PeriodId::quarter(2025, 3)).unwrap();
    let q4_revenue = results.get("revenue", &PeriodId::quarter(2025, 4)).unwrap();

    // Q3 should be peak, Q4 should be trough (based on historical pattern)
    assert!(q3_revenue > q2_revenue, "Q3 should be higher than Q2");
    assert!(q4_revenue < q3_revenue, "Q4 should be lower than Q3");
}
