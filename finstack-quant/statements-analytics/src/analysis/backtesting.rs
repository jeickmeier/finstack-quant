//! Forecast backtesting and error metrics.
//!
//! This module provides tools to evaluate forecast accuracy by comparing
//! predicted values against actual outcomes using standard error metrics.

use finstack_quant_core::math::ZERO_TOLERANCE;
use finstack_quant_statements::error::{Error, Result};

/// Forecast accuracy metrics.
///
/// Standard error metrics used to evaluate forecast quality by comparing
/// predictions against actual outcomes.
///
/// # Example
///
/// ```rust
/// # use finstack_quant_statements_analytics::analysis::backtest_forecast;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let actual = vec![100.0, 110.0, 105.0, 115.0];
/// let forecast = vec![98.0, 112.0, 104.0, 116.0];
///
/// let metrics = backtest_forecast(&actual, &forecast)?;
/// assert!(metrics.mae > 0.0);
/// assert!(metrics.rmse >= metrics.mae); // RMSE >= MAE always
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastMetrics {
    /// Mean Absolute Error: average of |actual - forecast|
    ///
    /// Interpretation: Average magnitude of errors in the same units as the data.
    /// Lower is better. Not sensitive to outliers.
    pub mae: f64,

    /// Mean Absolute Percentage Error: average of |actual - forecast| / |actual| × 100
    ///
    /// Interpretation: Average error as a percentage. Scale-independent.
    /// Be cautious when actual values are near zero (can produce extreme values).
    /// The implementation treats `|actual| < ZERO_TOLERANCE` as a zero-weight
    /// term and excludes that sample from both the numerator and the count;
    /// [`ForecastMetrics::mape_effective_n`] records how many samples were
    /// used so callers can tell a low MAPE from a MAPE that was computed on
    /// very few points. Very small non-zero actuals can still dominate the
    /// metric. If `mape_effective_n == 0`, `mape` is `NaN`; prefer
    /// [`ForecastMetrics::smape`] in that case.
    /// Lower is better.
    pub mape: f64,

    /// Number of samples that actually contributed to MAPE (i.e. had
    /// `|actual| >= ZERO_TOLERANCE`). Always `<= n`.
    pub mape_effective_n: usize,

    /// Symmetric Mean Absolute Percentage Error:
    /// `mean( |a - f| / ((|a| + |f|) / 2) ) × 100`.
    ///
    /// Always well-defined when at least one of `|a|` or `|f|` is positive,
    /// and always bounded in `[0, 200]`. Preferred over MAPE when the actual
    /// series contains zeros or near-zeros. Terms where both `a` and `f`
    /// are within `ZERO_TOLERANCE` of zero are skipped.
    pub smape: f64,

    /// Root Mean Squared Error: sqrt(average((actual - forecast)²))
    ///
    /// Interpretation: Penalizes larger errors more heavily than MAE.
    /// Same units as the data. Always >= MAE.
    /// Lower is better.
    pub rmse: f64,

    /// Number of data points used in the calculation
    pub n: usize,
}

impl ForecastMetrics {
    /// Format metrics as a human-readable string.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements_analytics::analysis::ForecastMetrics;
    /// let metrics = ForecastMetrics {
    ///     mae: 2.5,
    ///     mape: 3.7,
    ///     mape_effective_n: 10,
    ///     smape: 3.5,
    ///     rmse: 3.2,
    ///     n: 10,
    /// };
    /// println!("{}", metrics.summary());
    /// ```
    pub fn summary(&self) -> String {
        let mape_str = if self.mape.is_nan() {
            "n/a".to_string()
        } else {
            format!("{:.2}%", self.mape)
        };
        format!(
            "MAE: {:.2}, MAPE: {} (eff_n={}), sMAPE: {:.2}%, RMSE: {:.2} (n={})",
            self.mae, mape_str, self.mape_effective_n, self.smape, self.rmse, self.n
        )
    }
}

/// Compute forecast error metrics by comparing actual vs forecast values.
///
/// # Arguments
///
/// * `actual` - Actual observed values
/// * `forecast` - Forecasted/predicted values
///
/// # Returns
///
/// [`ForecastMetrics`] containing MAE, MAPE, and RMSE.
///
/// # Errors
///
/// Returns an error if:
/// - Arrays have different lengths
/// - Arrays are empty
/// - MAPE calculation encounters division by zero
///
/// # Example
///
/// ```rust
/// # use finstack_quant_statements_analytics::analysis::backtest_forecast;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let actual = vec![100.0, 110.0, 105.0, 115.0];
/// let forecast = vec![98.0, 112.0, 104.0, 116.0];
///
/// let metrics = backtest_forecast(&actual, &forecast)?;
///
/// println!("Forecast accuracy: {}", metrics.summary());
/// assert!(metrics.mae < 5.0); // MAE should be reasonable
/// # Ok(())
/// # }
/// ```
pub fn backtest_forecast(actual: &[f64], forecast: &[f64]) -> Result<ForecastMetrics> {
    if actual.len() != forecast.len() {
        return Err(Error::forecast(format!(
            "Actual and forecast arrays must have same length: {} vs {}",
            actual.len(),
            forecast.len()
        )));
    }

    if actual.is_empty() {
        return Err(Error::forecast("Cannot compute metrics on empty arrays"));
    }

    let n = actual.len();

    // Mean Absolute Error
    let mae = actual
        .iter()
        .zip(forecast.iter())
        .map(|(a, f)| (a - f).abs())
        .sum::<f64>()
        / n as f64;

    // Mean Absolute Percentage Error (excludes near-zero actuals).
    let (mape_sum, mape_effective_n) =
        actual
            .iter()
            .zip(forecast.iter())
            .fold((0.0_f64, 0_usize), |(sum, count), (a, f)| {
                if a.abs() < ZERO_TOLERANCE {
                    (sum, count)
                } else {
                    (sum + ((a - f).abs() / a.abs()) * 100.0, count + 1)
                }
            });
    let mape = if mape_effective_n > 0 {
        mape_sum / mape_effective_n as f64
    } else {
        f64::NAN
    };

    // Symmetric MAPE: well-defined even when some `a` are zero, so we
    // report it alongside MAPE as a second-line defence. Skip terms
    // where both `a` and `f` are effectively zero (no information).
    let (smape_sum, smape_count) =
        actual
            .iter()
            .zip(forecast.iter())
            .fold((0.0_f64, 0_usize), |(sum, count), (a, f)| {
                let denom = (a.abs() + f.abs()) * 0.5;
                if denom < ZERO_TOLERANCE {
                    (sum, count)
                } else {
                    (sum + ((a - f).abs() / denom) * 100.0, count + 1)
                }
            });
    let smape = if smape_count > 0 {
        smape_sum / smape_count as f64
    } else {
        f64::NAN
    };

    // Root Mean Squared Error
    let mse = actual
        .iter()
        .zip(forecast.iter())
        .map(|(a, f)| {
            let error = a - f;
            error * error
        })
        .sum::<f64>()
        / n as f64;

    let rmse = mse.sqrt();

    Ok(ForecastMetrics {
        mae,
        mape,
        mape_effective_n,
        smape,
        rmse,
        n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_forecast() {
        let actual = vec![100.0, 110.0, 120.0];
        let forecast = vec![100.0, 110.0, 120.0];

        let metrics = backtest_forecast(&actual, &forecast).expect("test should succeed");

        assert_eq!(metrics.mae, 0.0);
        assert_eq!(metrics.mape, 0.0);
        assert_eq!(metrics.rmse, 0.0);
        assert_eq!(metrics.n, 3);
    }

    #[test]
    fn test_constant_error() {
        let actual = vec![100.0, 110.0, 120.0];
        let forecast = vec![98.0, 108.0, 118.0]; // Consistently 2.0 low

        let metrics = backtest_forecast(&actual, &forecast).expect("test should succeed");

        assert!((metrics.mae - 2.0).abs() < 1e-10);
        assert!((metrics.rmse - 2.0).abs() < 1e-10); // Constant error: RMSE = MAE
    }

    #[test]
    fn test_rmse_greater_than_mae_with_outliers() {
        let actual = vec![100.0, 100.0, 100.0, 100.0];
        let forecast = vec![101.0, 101.0, 101.0, 110.0]; // One large error

        let metrics = backtest_forecast(&actual, &forecast).expect("test should succeed");

        // MAE: (1+1+1+10)/4 = 3.25
        assert!((metrics.mae - 3.25).abs() < 1e-10);

        // RMSE penalizes the large error more: sqrt((1+1+1+100)/4) = sqrt(25.75) ≈ 5.07
        assert!(metrics.rmse > metrics.mae);
        assert!((metrics.rmse - 5.074).abs() < 0.01);
    }

    #[test]
    fn test_mape_calculation() {
        let actual = vec![100.0, 200.0];
        let forecast = vec![90.0, 180.0]; // 10% and 10% errors

        let metrics = backtest_forecast(&actual, &forecast).expect("test should succeed");

        // MAPE: (10/100 + 20/200) * 100 / 2 = (0.1 + 0.1) * 100 / 2 = 10.0
        assert!((metrics.mape - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_mismatched_lengths_error() {
        let actual = vec![100.0, 110.0];
        let forecast = vec![100.0];

        let result = backtest_forecast(&actual, &forecast);
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .to_string()
            .contains("same length"));
    }

    #[test]
    fn test_empty_arrays_error() {
        let actual: Vec<f64> = vec![];
        let forecast: Vec<f64> = vec![];

        let result = backtest_forecast(&actual, &forecast);
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .to_string()
            .contains("empty"));
    }

    #[test]
    fn test_near_zero_actuals_handled() {
        let actual = vec![0.001, 100.0];
        let forecast = vec![1.0, 110.0]; // Large error on near-zero value

        // Should not panic or produce inf/nan
        let metrics = backtest_forecast(&actual, &forecast).expect("test should succeed");
        assert!(!metrics.mape.is_nan());
        assert!(!metrics.mape.is_infinite());
    }
}
