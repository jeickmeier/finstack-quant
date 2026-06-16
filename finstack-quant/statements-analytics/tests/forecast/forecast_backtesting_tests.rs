//! Tests for forecast backtesting and error metrics.

use finstack_quant_statements_analytics::analysis::{backtest_forecast, ForecastMetrics};

use crate::common;
use common::assert_close;

const METRICS_TOLERANCE: f64 = 1e-10;

// ============================================================================
// Basic Metrics Tests
// ============================================================================

#[test]
fn test_backtest_perfect_forecast() {
    let actual = vec![100.0, 110.0, 120.0, 130.0];
    let forecast = vec![100.0, 110.0, 120.0, 130.0];

    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    assert_eq!(metrics.mae, 0.0);
    assert_eq!(metrics.mape, 0.0);
    assert_eq!(metrics.rmse, 0.0);
    assert_eq!(metrics.n, 4);
}

#[test]
fn test_backtest_constant_bias() {
    let actual = vec![100.0, 110.0, 120.0];
    let forecast = vec![95.0, 105.0, 115.0]; // Consistently 5.0 low

    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    assert_close(metrics.mae, 5.0, METRICS_TOLERANCE, "MAE should be 5.0");
    assert_close(
        metrics.rmse,
        5.0,
        METRICS_TOLERANCE,
        "RMSE should equal MAE for constant error",
    );
}

#[test]
fn test_backtest_mae_vs_rmse() {
    let actual = vec![100.0, 100.0, 100.0, 100.0];
    let forecast = vec![99.0, 99.0, 99.0, 90.0]; // One large error

    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    // MAE: (1+1+1+10)/4 = 3.25
    assert_close(metrics.mae, 3.25, METRICS_TOLERANCE, "MAE calculation");

    // RMSE: sqrt((1+1+1+100)/4) = sqrt(25.75) ≈ 5.074
    assert!(
        metrics.rmse > metrics.mae,
        "RMSE should be > MAE with outliers"
    );
    assert_close(metrics.rmse, 5.074, 0.01, "RMSE calculation");
}

#[test]
fn test_backtest_mape_calculation() {
    let actual = vec![100.0, 200.0, 50.0];
    let forecast = vec![110.0, 180.0, 55.0];

    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    // MAPE: ((10/100 + 20/200 + 5/50) / 3) * 100 = ((0.1 + 0.1 + 0.1) / 3) * 100 = 10.0
    assert_close(metrics.mape, 10.0, METRICS_TOLERANCE, "MAPE calculation");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_backtest_single_datapoint() {
    let actual = vec![100.0];
    let forecast = vec![95.0];

    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    assert_eq!(metrics.mae, 5.0);
    assert_eq!(metrics.rmse, 5.0);
    assert_close(
        metrics.mape,
        5.0,
        METRICS_TOLERANCE,
        "MAPE for single point",
    );
    assert_eq!(metrics.n, 1);
}

#[test]
fn test_backtest_negative_values() {
    let actual = vec![-100.0, -110.0, -120.0];
    let forecast = vec![-95.0, -115.0, -118.0];

    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    // Metrics should work with negative values
    assert!(metrics.mae > 0.0);
    assert!(metrics.rmse > 0.0);
    assert!(metrics.mape > 0.0);
    assert!(!metrics.mae.is_nan());
    assert!(!metrics.rmse.is_nan());
}

#[test]
fn test_backtest_near_zero_actuals() {
    let actual = vec![0.001, 100.0, 200.0];
    let forecast = vec![10.0, 110.0, 210.0];

    // Should not panic or produce inf/nan despite near-zero actual
    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    assert!(!metrics.mape.is_infinite());
    assert!(!metrics.mape.is_nan());
}

// ============================================================================
// Error Cases
// ============================================================================

#[test]
fn test_backtest_length_mismatch_error() {
    let actual = vec![100.0, 110.0];
    let forecast = vec![100.0];

    let result = backtest_forecast(&actual, &forecast);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("same length"));
}

#[test]
fn test_backtest_empty_arrays_error() {
    let actual: Vec<f64> = vec![];
    let forecast: Vec<f64> = vec![];

    let result = backtest_forecast(&actual, &forecast);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

// ============================================================================
// Metrics Summary Tests
// ============================================================================

#[test]
fn test_forecast_metrics_summary() {
    let metrics = ForecastMetrics {
        mae: 2.5,
        mape: 3.7,
        mape_effective_n: 10,
        smape: 3.5,
        rmse: 3.2,
        n: 10,
    };

    let summary = metrics.summary();
    assert!(summary.contains("MAE"));
    assert!(summary.contains("MAPE"));
    assert!(summary.contains("RMSE"));
    assert!(summary.contains("2.50"));
    assert!(summary.contains("3.70%"));
    assert!(summary.contains("n=10"));
}

// ============================================================================
// Practical Forecasting Scenarios
// ============================================================================

#[test]
fn test_backtest_seasonal_forecast_scenario() {
    // Simulate a seasonal pattern where forecast captures trend but misses variance
    let actual = vec![100.0, 90.0, 110.0, 80.0, 105.0, 95.0];
    let forecast = vec![100.0, 95.0, 105.0, 85.0, 100.0, 95.0];

    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    // Verify metrics are computed
    assert!(metrics.mae > 0.0);
    assert!(metrics.mae < 10.0); // Should be reasonable
    assert!(metrics.rmse >= metrics.mae); // Mathematical property
    assert_eq!(metrics.n, 6);
}

#[test]
fn test_backtest_trending_series() {
    // Upward trend with forecast lag
    let actual = vec![100.0, 105.0, 110.0, 115.0, 120.0];
    let forecast = vec![100.0, 103.0, 108.0, 113.0, 118.0];

    let metrics = backtest_forecast(&actual, &forecast).unwrap();

    // Forecast is consistently slightly low
    assert!(metrics.mae > 0.0);
    assert!(metrics.mape > 0.0);
}
