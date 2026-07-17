//! Time-series forecasting methods with trend detection and seasonal decomposition.
//!
//! # Quarterly credit-model usage
//!
//! When building quarterly financial-statement models (the most common cadence
//! for credit analysis), keep the following in mind:
//!
//! * **`season_length`** — set to `4` for quarterly seasonality. The
//!   decomposition needs at least 2 full seasonal cycles (8 quarters) of
//!   historical data; 12–16 quarters is better for stable estimates.
//!
//! * **`actuals_until` interaction** — the evaluator only invokes forecasts for
//!   periods *after* the model's last `is_actual` period. If you update
//!   `actuals_until` to include a new quarter, that quarter's value is taken
//!   from the explicit node values and the seasonal forecast simply starts one
//!   period later. This means the historical window used by the decomposition
//!   grows automatically as you roll forward.
//!
//! * **Additive vs. multiplicative** — use `additive` when seasonal swings are
//!   roughly constant in absolute terms (e.g., a retailer's Q4 EBITDA uplift
//!   is always ~$5 M). Use `multiplicative` when the swing scales with the
//!   level (e.g., Q4 revenue is always ~20 % above trend). Multiplicative mode
//!   divides by the trend component; where a trend value is near zero the
//!   corresponding detrended ratio falls back to the identity (`1.0`) rather
//!   than dividing by ~0, and index normalization is skipped when the mean
//!   index itself is near zero. This keeps a series that crosses zero (e.g.
//!   EBITDA turning from loss to profit) from producing infinities, at the cost
//!   of a slightly biased seasonal index near the crossing — prefer `additive`
//!   for series that cross zero. For most credit metrics that are expected to
//!   stay positive, multiplicative is the safer default.
//!
//! Unsupported parameters such as `season_start` are rejected explicitly so
//! schema typos and unsupported calendar shifts do not become silent no-ops.

use crate::error::{Error, Result};
use crate::types::SeasonalMode;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::ZERO_TOLERANCE;
use indexmap::IndexMap;

fn parse_historical_series(historical: &[serde_json::Value], context: &str) -> Result<Vec<f64>> {
    let mut parsed = Vec::with_capacity(historical.len());
    for (idx, value) in historical.iter().enumerate() {
        let number = value.as_f64().ok_or_else(|| {
            Error::forecast(format!(
                "{} historical value at index {} must be numeric, got {}",
                context, idx, value
            ))
        })?;
        if !number.is_finite() {
            return Err(Error::forecast(format!(
                "{} historical value at index {} must be finite, got {}",
                context, idx, number
            )));
        }
        parsed.push(number);
    }
    Ok(parsed)
}

/// Apply time-series forecasting to generate future values.
///
/// Supports multiple methods:
/// - Linear trend detection
/// - Exponential smoothing
/// - Moving average
///
/// # Parameters
///
/// * `historical` - Array of historical values for trend detection (required)
/// * `method` - "linear", "exponential", "moving_average" (default: "linear")
/// * `alpha` - Smoothing factor for exponential method (0-1, required for exponential)
/// * `beta` - Trend smoothing factor for exponential method (0-1, required for exponential)
/// * `window` - Window size for moving average (required for moving_average)
pub(super) fn timeseries_forecast(
    _base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    // Validated here as well as at dispatch: these entry points are reachable
    // directly, and the vocabulary itself lives in one place.
    crate::forecast::validate_params(crate::types::ForecastMethod::TimeSeries, params)?;

    // Get historical data
    let historical = params
        .get("historical")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::forecast("'historical' must be an array"))?;

    if historical.len() < 2 {
        return Err(Error::forecast(format!(
            "Need at least 2 historical periods for trend detection, got {}. \
             Provide more historical data in the 'historical' parameter.",
            historical.len()
        )));
    }

    // Convert to f64 array
    let hist_data = parse_historical_series(historical, "Time-series forecast")?;

    // Get method (default to "linear")
    let method = params
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("linear");

    let mut result = IndexMap::new();

    match method {
        "linear" => {
            // Linear trend using least squares
            let (slope, intercept) = calculate_linear_trend(&hist_data);
            let n_hist = hist_data.len() as f64;

            for (i, period_id) in forecast_periods.iter().enumerate() {
                let t = n_hist + i as f64 + 1.0;
                let value = slope * t + intercept;
                if !value.is_finite() {
                    return Err(Error::forecast(format!(
                        "Linear forecast produced a non-finite value at period {:?}",
                        period_id
                    )));
                }
                result.insert(*period_id, value);
            }
        }

        "exponential" => {
            // Double exponential smoothing (Holt's method)
            // Both parameters are required per market standards (no universal defaults)
            let alpha = params
                .get("alpha")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| {
                    Error::forecast(
                        "'alpha' parameter required for exponential smoothing. \
                     Typical range: 0.05 (slow/stable) to 0.3 (fast/responsive). \
                     Industry standard: alpha = 2/(n+1) where n is smoothing window. \
                     Example: alpha = 0.2 for moderate smoothing.",
                    )
                })?;

            let beta = params.get("beta").and_then(|v| v.as_f64()).ok_or_else(|| {
                Error::forecast(
                    "'beta' parameter required for exponential smoothing trend. \
                     Typical range: 0.05 (slow trend) to 0.2 (fast trend). \
                     Should typically be less than alpha. \
                     Example: beta = 0.1 for moderate trend responsiveness.",
                )
            })?;

            // Validate parameter ranges — use open interval (0, 1).
            // alpha=0 means the level never updates (forecast ignores data).
            // beta=0 freezes the trend at the initial value.
            // alpha=1 or beta=1 degenerates to naïve methods.
            // See Hyndman & Athanasopoulos, "Forecasting: Principles and Practice", §8.1.
            if alpha <= 0.0 || alpha >= 1.0 {
                return Err(Error::forecast(format!(
                    "alpha must be in the open interval (0, 1), got {}. \
                     alpha=0 means the level never updates; alpha=1 uses only the latest observation. \
                     Typical values: 0.05 to 0.3",
                    alpha
                )));
            }
            if beta <= 0.0 || beta >= 1.0 {
                return Err(Error::forecast(format!(
                    "beta must be in the open interval (0, 1), got {}. \
                     beta=0 freezes the trend at its initial value; beta=1 uses only the latest trend. \
                     Typical values: 0.05 to 0.2",
                    beta
                )));
            }

            let (level, trend) = double_exponential_smoothing(&hist_data, alpha, beta);

            for (i, period_id) in forecast_periods.iter().enumerate() {
                let value = level + trend * (i + 1) as f64;
                if !value.is_finite() {
                    return Err(Error::forecast(format!(
                        "Exponential smoothing produced a non-finite value at period {:?}",
                        period_id
                    )));
                }
                result.insert(*period_id, value);
            }
        }

        "moving_average" => {
            // Simple moving average with trend extrapolation
            let window = params
                .get("window")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    Error::forecast(
                        "'window' parameter required for moving average forecast. \
                     Common values: 3 (short-term), 5-10 (medium-term), 20+ (long-term). \
                     Must be less than the number of historical periods. \
                     Example: window = 3 for 3-period moving average.",
                    )
                })? as usize;

            if window == 0 {
                return Err(Error::forecast("window must be greater than 0"));
            }
            if window > hist_data.len() {
                return Err(Error::forecast(format!(
                    "window ({}) cannot exceed historical data length ({})",
                    window,
                    hist_data.len()
                )));
            }

            let window = window.min(hist_data.len());

            // Calculate moving average of last 'window' periods
            let ma: f64 = hist_data.iter().rev().take(window).sum::<f64>() / window as f64;

            // Calculate trend from moving averages
            if hist_data.len() > window {
                let prev_ma: f64 = hist_data[..hist_data.len() - 1]
                    .iter()
                    .rev()
                    .take(window)
                    .sum::<f64>()
                    / window as f64;
                let trend = ma - prev_ma;

                // A trailing moving average is centered (window-1)/2 periods
                // behind the last observation, so extrapolating from `ma`
                // alone carries a half-window lag bias of trend*(window-1)/2
                // at every horizon. Anchor the projection at the last
                // observation's trend level instead.
                let half_window_lag = (window as f64 - 1.0) / 2.0;

                for (i, period_id) in forecast_periods.iter().enumerate() {
                    let value = ma + trend * (half_window_lag + (i + 1) as f64);
                    if !value.is_finite() {
                        return Err(Error::forecast(format!(
                            "Moving average forecast produced a non-finite value at period {:?}",
                            period_id
                        )));
                    }
                    result.insert(*period_id, value);
                }
            } else {
                // No trend, use constant MA
                for period_id in forecast_periods {
                    result.insert(*period_id, ma);
                }
            }
        }

        _ => {
            return Err(Error::forecast(format!(
                "Unknown time series method: {}",
                method
            )));
        }
    }

    Ok(result)
}

/// Calculate linear trend using numerically stable least squares regression.
///
/// # Numerical Stability
///
/// Uses the mean-centered formulation to avoid catastrophic cancellation:
///
/// ```text
/// slope = Σ((x_i - x̄)(y_i - ȳ)) / Σ((x_i - x̄)²)
/// intercept = ȳ - slope × x̄
/// ```
///
/// This is equivalent to the standard formula but avoids subtracting two large
/// nearly-equal numbers, which causes precision loss for large `n` or correlated
/// data. See Welford's algorithm / Knuth TAOCP Vol. 2, Section 4.2.2.
///
/// # Returns
///
/// A tuple `(slope, intercept)` where `y = slope * x + intercept`.
/// For degenerate cases (constant x or insufficient data), returns `(0.0, mean_y)`.
fn calculate_linear_trend(data: &[f64]) -> (f64, f64) {
    let n = data.len() as f64;
    if n == 0.0 {
        return (0.0, 0.0);
    }

    // Mean of x values (1-indexed: 1, 2, ..., n)
    let x_bar = (n + 1.0) / 2.0;
    let y_bar = data.iter().sum::<f64>() / n;

    // Compute slope using mean-centered formula (numerically stable)
    let mut num = 0.0; // Σ (x_i - x̄)(y_i - ȳ)
    let mut den = 0.0; // Σ (x_i - x̄)²

    for (i, &y) in data.iter().enumerate() {
        let dx = (i + 1) as f64 - x_bar;
        let dy = y - y_bar;
        num += dx * dy;
        den += dx * dx;
    }

    // Guard against degenerate cases (near-zero denominator)
    if den.abs() < ZERO_TOLERANCE {
        return (0.0, y_bar);
    }

    let slope = num / den;
    let intercept = y_bar - slope * x_bar;

    (slope, intercept)
}

/// Double exponential smoothing (Holt's method)
fn double_exponential_smoothing(data: &[f64], alpha: f64, beta: f64) -> (f64, f64) {
    if data.is_empty() {
        return (0.0, 0.0);
    }

    let mut level = data[0];
    let init_window = data.len().min(10);
    let mut trend = if init_window >= 2 {
        (data[init_window - 1] - data[0]) / (init_window - 1) as f64
    } else {
        0.0
    };

    for &value in data.iter().skip(1) {
        let prev_level = level;
        level = alpha * value + (1.0 - alpha) * (level + trend);
        trend = beta * (level - prev_level) + (1.0 - beta) * trend;
    }

    (level, trend)
}

/// Apply a seasonal forecast with decomposition.
///
/// Uses seasonal decomposition of historical data to extract trend, seasonal,
/// and residual components, then projects forward.
///
/// # Parameters
///
/// * `historical` - Array of historical values (need 2+ seasons, required)
/// * `season_length` - Length of seasonal cycle (required)
/// * `growth` - Total compound growth rate applied to the trend component
///   (default: 0.0). When `growth == 0.0` the trend is held flat at its last
///   historical value. This is a **total** rate, not an additional rate on
///   top of the historical trend slope—the decomposition already captures the
///   historical trajectory in the trend component.
/// * `mode` - SeasonalMode enum: "additive" or "multiplicative" (required)
///
/// Unsupported calendar-shift parameters such as `season_start` are rejected;
/// the decomposition keys seasonal factors by data position.
///
/// Note: `base_value` is provided for API parity with other forecast methods but
/// is not used in the seasonal calculation—the historical series establishes
/// the level for projections.
pub(super) fn seasonal_forecast(
    base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    // Validated here as well as at dispatch: this entry point is reachable
    // directly, and the vocabulary itself lives in one place.
    crate::forecast::validate_params(crate::types::ForecastMethod::Seasonal, params)?;
    // `base_value` is currently ignored for seasonal forecasts—historical data provides the level.
    seasonal_forecast_with_decomposition(base_value, forecast_periods, params)
}

/// Apply seasonal forecasting with decomposition.
///
/// This is the main implementation of seasonal forecasting, requiring historical
/// data and explicit parameters for decomposition.
fn seasonal_forecast_with_decomposition(
    _base_value: f64,
    forecast_periods: &[PeriodId],
    params: &IndexMap<String, serde_json::Value>,
) -> Result<IndexMap<PeriodId, f64>> {
    // Get historical data
    let historical = params
        .get("historical")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::forecast("'historical' must be an array"))?;

    // Convert to f64 array
    let hist_data = parse_historical_series(historical, "Seasonal forecast")?;

    // Get season length (required parameter - no default per market standards)
    let season_length = params
        .get("season_length")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            Error::forecast(
                "'season_length' parameter required for seasonal decomposition. \
             Common values: 4 (quarterly), 12 (monthly). \
             Must match the cyclical pattern in your data.",
            )
        })? as usize;

    if season_length == 0 {
        return Err(Error::forecast("season_length must be greater than 0"));
    }

    if hist_data.len() < season_length * 2 {
        return Err(Error::forecast(format!(
            "Need at least 2 full seasons of historical data for seasonal decomposition. \
             Season length: {}, need {} periods, got {}. \
             Provide more historical data or reduce season_length.",
            season_length,
            season_length * 2,
            hist_data.len()
        )));
    }

    // Get mode (type-safe enum)
    let mode = params.get("mode").ok_or_else(|| {
        Error::forecast(
            "'mode' parameter required for seasonal forecast. \
             Must be either 'additive' or 'multiplicative'.",
        )
    })?;
    let mode: SeasonalMode = serde_json::from_value(mode.clone()).map_err(|_| {
        Error::forecast("Invalid 'mode' parameter. Must be 'additive' or 'multiplicative'.")
    })?;

    // Decompose the series using the requested seasonal semantics.
    let (trend, seasonal, _residual) = decompose_series_with_mode(&hist_data, season_length, mode);

    // Get growth rate for trend projection
    let growth = params.get("growth").and_then(|v| v.as_f64()).unwrap_or(0.0);

    // Project forward
    let mut results = IndexMap::new();
    let last_trend = trend
        .last()
        .copied()
        .unwrap_or_else(|| hist_data.last().copied().unwrap_or(0.0));

    for (i, period_id) in forecast_periods.iter().enumerate() {
        // Calculate trend component with growth
        let trend_value = if growth == 0.0 {
            last_trend
        } else {
            last_trend * (1.0 + growth).powi(i as i32 + 1)
        };

        // Get seasonal component. The decomposition keys factors by *data
        // position* (index 0 of `seasonal` corresponds to the first
        // historical observation), so the forecast simply continues the
        // positional cycle. Adding `season_start` here would double-shift
        // and rotate the pattern (e.g. apply the Q4 uplift to Q1).
        let season_idx = (hist_data.len() + i) % season_length;
        let seasonal_value = seasonal.get(season_idx).copied().unwrap_or(0.0);

        // Combine based on mode (type-safe match)
        let value = match mode {
            SeasonalMode::Additive => trend_value + seasonal_value,
            SeasonalMode::Multiplicative => trend_value * seasonal_value,
        };

        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "Seasonal forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }

        // Allow negative values for financial metrics (losses, declines, etc.)
        results.insert(*period_id, value);
    }

    Ok(results)
}

/// Decompose a time series into trend, seasonal, and residual components.
/// Uses a simple moving average approach.
#[cfg(test)]
fn decompose_series(data: &[f64], season_length: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    decompose_series_with_mode(data, season_length, SeasonalMode::Additive)
}

fn decompose_series_with_mode(
    data: &[f64],
    season_length: usize,
    mode: SeasonalMode,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let trend = calculate_trend_component(data, season_length);

    match mode {
        SeasonalMode::Additive => decompose_additive(data, season_length, trend),
        SeasonalMode::Multiplicative => decompose_multiplicative(data, season_length, trend),
    }
}

fn calculate_trend_component(data: &[f64], season_length: usize) -> Vec<f64> {
    // Internal helper. Public callers (`seasonal_forecast_with_decomposition`)
    // already validate `season_length > 0` and `data.len() >= season_length * 2`,
    // so the asserts catch any future caller that bypasses those guards before
    // they reach the modulo / division-by-zero paths below. In release builds
    // we degrade to a no-op return rather than panicking so a future refactor
    // that bypasses the public guards cannot crash production.
    debug_assert!(season_length > 0, "season_length must be > 0");
    debug_assert!(!data.is_empty(), "data must not be empty");
    if season_length == 0 || data.is_empty() {
        return Vec::new();
    }
    let n = data.len();
    let mut trend = vec![0.0; n];
    let half_season = season_length / 2;
    let is_even = season_length.is_multiple_of(2);

    // Use centered moving average for middle values.
    // For even-period seasonality, apply 2×MA weighting (half-weight endpoints).
    for (i, trend_val) in trend
        .iter_mut()
        .enumerate()
        .take(n.saturating_sub(half_season))
        .skip(half_season)
    {
        let window_start = i.saturating_sub(half_season);
        let window_end = (i + half_season + 1).min(n);

        if is_even && window_end - window_start == season_length + 1 {
            let sum: f64 = data[window_start + 1..window_end - 1].iter().sum();
            let endpoint_sum = 0.5 * (data[window_start] + data[window_end - 1]);
            *trend_val = (sum + endpoint_sum) / season_length as f64;
        } else {
            let sum: f64 = data[window_start..window_end].iter().sum();
            *trend_val = sum / (window_end - window_start) as f64;
        }
    }

    // Extrapolate trend to edges using linear extrapolation from the first /
    // last two centered-MA points. Guard indices and require finite, non-equal
    // values before computing the slope.
    if n > season_length {
        // Front extrapolation: slope from the first two centered trend points.
        let slope_front = if half_season + 1 < n
            && trend[half_season].is_finite()
            && trend[half_season + 1].is_finite()
        {
            let diff = trend[half_season + 1] - trend[half_season];
            if diff.abs() > ZERO_TOLERANCE {
                diff
            } else {
                0.0
            }
        } else {
            0.0
        };
        for i in 0..half_season {
            trend[i] = trend[half_season] - slope_front * (half_season - i) as f64;
        }

        // Back extrapolation: slope from the last two centered trend points.
        let last_valid = n.saturating_sub(half_season + 1);
        let slope_back =
            if last_valid > 0 && trend[last_valid].is_finite() && trend[last_valid - 1].is_finite()
            {
                let diff = trend[last_valid] - trend[last_valid - 1];
                if diff.abs() > ZERO_TOLERANCE {
                    diff
                } else {
                    0.0
                }
            } else {
                0.0
            };
        for i in (n.saturating_sub(half_season))..n {
            let steps = i - (n - half_season - 1);
            trend[i] = trend[n - half_season - 1] + slope_back * steps as f64;
        }
    } else {
        // For short series, use simple average as trend
        let avg = data.iter().sum::<f64>() / n as f64;
        for trend_val in trend.iter_mut().take(n) {
            *trend_val = avg;
        }
    }

    trend
}

fn decompose_additive(
    data: &[f64],
    season_length: usize,
    trend: Vec<f64>,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    // `i % season_length` and `sum / season_length` would panic / produce NaN
    // if season_length were zero. Public callers guard this; assert in case of
    // future refactors and degrade to empty in release builds rather than
    // panicking on `i % 0`.
    debug_assert!(season_length > 0, "season_length must be > 0");
    if season_length == 0 {
        return (trend, Vec::new(), vec![0.0; data.len()]);
    }
    let n = data.len();

    // Calculate detrended series
    let detrended: Vec<f64> = data.iter().zip(&trend).map(|(d, t)| d - t).collect();

    // Calculate seasonal component (average of same season across years)
    let mut seasonal = vec![0.0; season_length];
    for (season, seasonal_val) in seasonal.iter_mut().enumerate().take(season_length) {
        let mut sum = 0.0;
        let mut count = 0;

        let mut idx = season;
        while idx < n {
            sum += detrended[idx];
            count += 1;
            idx += season_length;
        }

        if count > 0 {
            *seasonal_val = sum / count as f64;
        }
    }

    // Normalize seasonal component (sum to zero for additive)
    let seasonal_mean: f64 = seasonal.iter().sum::<f64>() / season_length as f64;
    for s in &mut seasonal {
        *s -= seasonal_mean;
    }

    // Calculate residual
    let mut residual = vec![0.0; n];
    for i in 0..n {
        let season_idx = i % season_length;
        residual[i] = data[i] - trend[i] - seasonal[season_idx];
    }

    (trend, seasonal, residual)
}

fn decompose_multiplicative(
    data: &[f64],
    season_length: usize,
    trend: Vec<f64>,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    // See `decompose_additive` — same modulo-zero / divide-by-zero risk if a
    // future caller bypasses the public-API guards. Multiplicative residuals
    // default to 1.0 (the identity) when there is nothing to decompose.
    debug_assert!(season_length > 0, "season_length must be > 0");
    if season_length == 0 {
        return (trend, Vec::new(), vec![1.0; data.len()]);
    }
    let n = data.len();

    let detrended: Vec<f64> = data
        .iter()
        .zip(&trend)
        .map(|(d, t)| if t.abs() < ZERO_TOLERANCE { 1.0 } else { d / t })
        .collect();

    let mut seasonal = vec![1.0; season_length];
    for (season, seasonal_val) in seasonal.iter_mut().enumerate().take(season_length) {
        let mut sum = 0.0;
        let mut count = 0;

        let mut idx = season;
        while idx < n {
            sum += detrended[idx];
            count += 1;
            idx += season_length;
        }

        if count > 0 {
            *seasonal_val = sum / count as f64;
        }
    }

    let seasonal_mean = seasonal.iter().sum::<f64>() / season_length as f64;
    if seasonal_mean.abs() >= ZERO_TOLERANCE {
        for s in &mut seasonal {
            *s /= seasonal_mean;
        }
    }

    let mut residual = vec![1.0; n];
    for i in 0..n {
        let season_idx = i % season_length;
        let denom = trend[i] * seasonal[season_idx];
        residual[i] = if denom.abs() < ZERO_TOLERANCE {
            1.0
        } else {
            data[i] / denom
        };
    }

    (trend, seasonal, residual)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::indexmap;

    #[test]
    fn test_timeseries_forecast() {
        let params = indexmap! {
            "historical".into() => serde_json::json!([100.0, 110.0, 120.0, 130.0]),
            "method".into() => serde_json::json!("linear"),
        };

        let periods = vec![PeriodId::quarter(2025, 1), PeriodId::quarter(2025, 2)];

        let result = timeseries_forecast(100.0, &periods, &params)
            .expect("timeseries_forecast should succeed");

        // Should continue linear trend
        assert!(result[&PeriodId::quarter(2025, 1)] > 130.0);
        assert!(result[&PeriodId::quarter(2025, 2)] > result[&PeriodId::quarter(2025, 1)]);
    }

    #[test]
    fn test_seasonal_forecast_with_historical() {
        // Create historical data with seasonal pattern (2 years of quarterly data)
        let historical = vec![
            100.0, 90.0, 110.0, 85.0, // Year 1
            105.0, 95.0, 115.0, 90.0, // Year 2
        ];

        let params = indexmap! {
            "historical".into() => serde_json::json!(historical),
            "season_length".into() => serde_json::json!(4),
            "mode".into() => serde_json::json!("multiplicative"),
            "growth".into() => serde_json::json!(0.02),
        };

        let periods = vec![
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
            PeriodId::quarter(2025, 4),
        ];

        let result =
            seasonal_forecast(100.0, &periods, &params).expect("seasonal_forecast should succeed");

        // Should produce forecasts for all 4 quarters with seasonal pattern
        assert!(result.len() == 4);

        // Check that growth is applied
        let q1 = result[&PeriodId::quarter(2025, 1)];
        let q2 = result[&PeriodId::quarter(2025, 2)];
        assert!(q1 > 0.0, "Q1 should be positive");
        assert!(q2 > 0.0, "Q2 should be positive");
    }

    #[test]
    fn test_linear_trend() {
        let data = vec![10.0, 20.0, 30.0, 40.0];
        let (slope, intercept) = calculate_linear_trend(&data);

        // Perfect linear trend: y = 10x + 0
        assert_eq!(slope, 10.0);
        assert_eq!(intercept, 0.0);
    }

    #[test]
    fn test_exponential_smoothing() {
        let data = vec![100.0, 110.0, 120.0, 130.0];
        let (level, trend) = double_exponential_smoothing(&data, 0.5, 0.5);

        // Should detect upward trend
        assert!(level > 120.0);
        assert!(trend > 0.0);
    }

    #[test]
    fn test_seasonal_decomposition() {
        // Create data with clear seasonal pattern
        let data = vec![
            100.0, 90.0, 110.0, 85.0, // Year 1
            105.0, 95.0, 115.0, 90.0, // Year 2
            110.0, 100.0, 120.0, 95.0, // Year 3
        ];

        let (trend, seasonal, _residual) = decompose_series(&data, 4);

        // Check that decomposition produces reasonable results
        assert_eq!(
            trend.len(),
            data.len(),
            "Trend should have same length as data"
        );
        assert_eq!(seasonal.len(), 4, "Seasonal should have 4 components");

        // Check that seasonal components sum to approximately zero (for additive)
        let seasonal_sum: f64 = seasonal.iter().sum();
        assert!(
            seasonal_sum.abs() < 1.0,
            "Seasonal components should sum to near zero"
        );

        // There should be some variation in seasonal components
        let seasonal_max = seasonal.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let seasonal_min = seasonal.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        assert!(
            seasonal_max > seasonal_min,
            "Seasonal should have variation"
        );
    }

    #[test]
    fn test_seasonal_forecast_rejects_unsupported_season_start() {
        let periods = vec![PeriodId::quarter(2025, 1), PeriodId::quarter(2025, 2)];
        let params = indexmap! {
            "historical".into() => serde_json::json!([0.0, 10.0, 0.0, 10.0, 0.0, 10.0]),
            "season_length".into() => serde_json::json!(2),
            "mode".into() => serde_json::json!("additive"),
            "season_start".into() => serde_json::json!(1),
        };

        let err = seasonal_forecast(0.0, &periods, &params)
            .expect_err("unsupported season_start must be rejected");
        assert!(err.to_string().contains("season_start"));
    }

    #[test]
    fn test_moving_average_forecast_has_no_half_window_lag() {
        // Perfectly linear data y = 10x with window=2: ma = 35, trend = 10.
        // The trailing MA is centered (window-1)/2 = 0.5 periods behind the
        // last observation, so the anchored projection must continue the
        // line exactly: 50, 60 (the unanchored form lagged by trend/2 = 5).
        let params = indexmap! {
            "historical".into() => serde_json::json!([10.0, 20.0, 30.0, 40.0]),
            "method".into() => serde_json::json!("moving_average"),
            "window".into() => serde_json::json!(2),
        };
        let periods = vec![PeriodId::quarter(2025, 1), PeriodId::quarter(2025, 2)];

        let result = timeseries_forecast(40.0, &periods, &params)
            .expect("timeseries_forecast should succeed");

        assert!((result[&PeriodId::quarter(2025, 1)] - 50.0).abs() < 1e-9);
        assert!((result[&PeriodId::quarter(2025, 2)] - 60.0).abs() < 1e-9);
    }

    #[test]
    fn test_timeseries_forecast_rejects_unknown_param_keys() {
        let params = indexmap! {
            "historical".into() => serde_json::json!([100.0, 110.0, 120.0]),
            "method".into() => serde_json::json!("moving_average"),
            "windoww".into() => serde_json::json!(2),
        };
        let periods = vec![PeriodId::quarter(2025, 1)];

        let err = timeseries_forecast(100.0, &periods, &params)
            .expect_err("typoed parameter key must be rejected");
        assert!(err.to_string().contains("windoww"));
    }

    #[test]
    fn test_seasonal_forecast_rejects_unknown_param_keys() {
        let params = indexmap! {
            "historical".into() => serde_json::json!([100.0, 90.0, 110.0, 85.0]),
            "season_length".into() => serde_json::json!(2),
            "mode".into() => serde_json::json!("additive"),
            "seasonstart".into() => serde_json::json!(1),
        };
        let periods = vec![PeriodId::quarter(2025, 1)];

        let err = seasonal_forecast(100.0, &periods, &params)
            .expect_err("typoed parameter key must be rejected");
        assert!(err.to_string().contains("seasonstart"));
    }

    #[test]
    fn test_seasonal_forecast_rejects_zero_season_length() {
        let params = indexmap! {
            "historical".into() => serde_json::json!([100.0, 90.0, 110.0, 85.0]),
            "season_length".into() => serde_json::json!(0),
            "mode".into() => serde_json::json!("additive"),
        };
        let periods = vec![PeriodId::quarter(2025, 1)];

        let result = seasonal_forecast(100.0, &periods, &params);
        assert!(result.is_err(), "zero season length must be rejected");
    }

    #[test]
    fn test_timeseries_forecast_rejects_non_numeric_history_entries() {
        let params = indexmap! {
            "historical".into() => serde_json::json!([100.0, "bad", 110.0]),
            "method".into() => serde_json::json!("linear"),
        };
        let periods = vec![PeriodId::quarter(2025, 1)];

        let result = timeseries_forecast(100.0, &periods, &params);
        assert!(
            result.is_err(),
            "malformed historical series should fail instead of being compacted"
        );
    }

    #[test]
    fn test_multiplicative_seasonal_forecast_preserves_relative_factors() {
        // Stable-level data with clear seasonal pattern (3 years).
        // Seasonal ratios: Q1=1.0, Q2=0.8, Q3=1.2, Q4=0.9 relative to Q1.
        let params = indexmap! {
            "historical".into() => serde_json::json!([
                100.0, 80.0, 120.0, 90.0,
                102.0, 82.0, 122.0, 92.0,
                104.0, 84.0, 124.0, 94.0
            ]),
            "season_length".into() => serde_json::json!(4),
            "mode".into() => serde_json::json!("multiplicative"),
            "growth".into() => serde_json::json!(0.0),
        };
        let periods = vec![
            PeriodId::quarter(2025, 1),
            PeriodId::quarter(2025, 2),
            PeriodId::quarter(2025, 3),
            PeriodId::quarter(2025, 4),
        ];

        let result =
            seasonal_forecast(100.0, &periods, &params).expect("seasonal_forecast should succeed");

        let q1 = result[&PeriodId::quarter(2025, 1)];
        let q2 = result[&PeriodId::quarter(2025, 2)];
        let q3 = result[&PeriodId::quarter(2025, 3)];
        let q4 = result[&PeriodId::quarter(2025, 4)];

        assert!((q2 / q1 - 0.8).abs() < 0.05, "q2/q1={}", q2 / q1);
        assert!((q3 / q1 - 1.2).abs() < 0.05, "q3/q1={}", q3 / q1);
        assert!((q4 / q1 - 0.9).abs() < 0.05, "q4/q1={}", q4 / q1);
    }
}
