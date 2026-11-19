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
//!   from the explicit node values and the seasonal forecast starts one
//!   period later. Update the external `historical` array at the same time:
//!   the evaluator does not append model actuals, and stale history retains
//!   its old level and seasonal phase.
//!
//! * **Additive vs. multiplicative** — use `additive` when seasonal swings are
//!   roughly constant in absolute terms (e.g., a retailer's Q4 EBITDA uplift
//!   is always ~$5 M). Use `multiplicative` when the swing scales with the
//!   level (e.g., Q4 revenue is always ~20 % above trend). Multiplicative mode
//!   divides by the trend component, so it is rejected with an error when the
//!   trend crosses or touches zero (e.g. EBITDA turning from loss to profit) —
//!   use `additive` for such series. For most credit metrics that are expected
//!   to stay positive, multiplicative is the safer default.
//!
//! Unsupported parameters such as `season_start` are rejected explicitly so
//! schema typos and unsupported calendar shifts do not become silent no-ops.

use crate::error::{Error, Result};
use crate::types::{ForecastTrendMethod, SeasonalMode, SeasonalParams, TimeSeriesParams};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::summation::neumaier_sum;
use indexmap::IndexMap;

fn finite_history(historical: &[f64], context: &str) -> Result<()> {
    match historical.iter().position(|value| !value.is_finite()) {
        Some(idx) => Err(Error::forecast(format!(
            "{context} historical value at index {idx} must be finite, got {}",
            historical[idx]
        ))),
        None => Ok(()),
    }
}

/// Apply time-series forecasting to generate future values.
///
/// Supports multiple methods:
/// - Linear trend detection
/// - Exponential smoothing
/// - Moving average
///
/// # Arguments
///
/// * `forecast_periods` - Periods that require projected values
/// * `params` - At least 2 finite `historical` observations (oldest first) and
///   the trend `method`: linear, exponential (Holt; requires `alpha` and `beta`
///   in (0, 1), optional trend damping `phi` in (0, 1], default 1.0 = classic
///   undamped Holt, legal only with this method) or moving average (requires a
///   positive `window` no longer than the history)
pub(super) fn timeseries_forecast(
    forecast_periods: &[PeriodId],
    params: &TimeSeriesParams,
) -> Result<IndexMap<PeriodId, f64>> {
    let hist_data = params.historical.as_slice();
    if hist_data.len() < 2 {
        return Err(Error::forecast(format!(
            "Need at least 2 historical periods for trend detection, got {}. \
             Provide more historical data in the 'historical' parameter.",
            hist_data.len()
        )));
    }
    finite_history(hist_data, "Time-series forecast")?;

    let method = params.method;

    // `phi` (trend damping) only has meaning for exponential smoothing; a
    // damping factor silently ignored on another method would be a no-op typo.
    if params.phi.is_some() && method != ForecastTrendMethod::Exponential {
        return Err(Error::forecast(format!(
            "'phi' is only valid with method = \"exponential\", but method is {method:?}. \
             Remove 'phi' or switch the method."
        )));
    }

    let mut result = IndexMap::new();

    match method {
        ForecastTrendMethod::Linear => {
            let fit = LinearFit::of(hist_data)?;
            for (i, period_id) in forecast_periods.iter().enumerate() {
                let value = fit.at_step(i + 1);
                if !value.is_finite() {
                    return Err(Error::forecast(format!(
                        "Linear forecast produced a non-finite value at period {:?}",
                        period_id
                    )));
                }
                result.insert(*period_id, value);
            }
        }

        ForecastTrendMethod::Exponential => {
            // Both Holt parameters are required; there is no universal default.
            let alpha = params.alpha.ok_or_else(|| {
                Error::forecast(
                    "'alpha' parameter required for exponential smoothing. \
                     Typical range: 0.05 (slow/stable) to 0.3 (fast/responsive). \
                     Industry standard: alpha = 2/(n+1) where n is smoothing window. \
                     Example: alpha = 0.2 for moderate smoothing.",
                )
            })?;

            let beta = params.beta.ok_or_else(|| {
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
            if !(alpha > 0.0 && alpha < 1.0) {
                return Err(Error::forecast(format!(
                    "alpha must be in the open interval (0, 1), got {}. \
                     alpha=0 means the level never updates; alpha=1 uses only the latest observation. \
                     Typical values: 0.05 to 0.3",
                    alpha
                )));
            }
            if !(beta > 0.0 && beta < 1.0) {
                return Err(Error::forecast(format!(
                    "beta must be in the open interval (0, 1), got {}. \
                     beta=0 freezes the trend at its initial value; beta=1 uses only the latest trend. \
                     Typical values: 0.05 to 0.2",
                    beta
                )));
            }

            // Optional trend damping (damped Holt, Hyndman & Athanasopoulos §8.2).
            // phi = 1.0 (default) is classic Holt; phi < 1 damps the trend so
            // long-horizon forecasts converge to level + phi/(1-phi) * trend
            // instead of extrapolating linearly forever.
            let phi = params.phi.unwrap_or(1.0);
            if !phi.is_finite() || phi <= 0.0 || phi > 1.0 {
                return Err(Error::forecast(format!(
                    "phi must be in the half-open interval (0, 1], got {phi}. \
                     phi=1 is classic (undamped) Holt; smaller values damp the trend \
                     harder. Typical values: 0.8 to 0.98."
                )));
            }

            let (level, trend) = double_exponential_smoothing(hist_data, alpha, beta, phi);
            if !level.is_finite() || !trend.is_finite() {
                return Err(Error::forecast(
                    "Exponential smoothing produced a non-finite level or trend",
                ));
            }

            // Damped forecast function: ŷ_{t+h} = ℓ + (φ + φ² + … + φ^h)·b.
            // With φ = 1 the damped sum accumulates exactly 1.0 per step, so
            // it equals h and reproduces classic Holt bit-for-bit.
            let mut damped_sum = 0.0;
            let mut phi_pow = 1.0;
            for period_id in forecast_periods {
                phi_pow *= phi;
                damped_sum += phi_pow;
                let value = level + trend * damped_sum;
                if !value.is_finite() {
                    return Err(Error::forecast(format!(
                        "Exponential smoothing produced a non-finite value at period {:?}",
                        period_id
                    )));
                }
                result.insert(*period_id, value);
            }
        }

        ForecastTrendMethod::MovingAverage => {
            let window = params.window.ok_or_else(|| {
                Error::forecast(
                    "'window' parameter required for moving average forecast. \
                     Common values: 3 (short-term), 5-10 (medium-term), 20+ (long-term). \
                     Must be less than the number of historical periods. \
                     Example: window = 3 for 3-period moving average.",
                )
            })?;
            let window = usize::try_from(window)
                .map_err(|_| Error::forecast("window exceeds the supported platform size"))?;

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

            let n = hist_data.len();
            let ma = scaled_mean(&hist_data[n - window..])?;

            if n > window {
                // The preceding same-length window, shifted back one slot.
                let prev_ma = scaled_mean(&hist_data[n - window - 1..n - 1])?;
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
    }

    Ok(result)
}

/// The largest magnitude of a finite series, or 1 when it is all zeros:
/// dividing by it keeps every sum of the series representable.
fn magnitude_scale(data: &[f64]) -> f64 {
    let largest = data.iter().fold(0.0_f64, |largest, y| largest.max(y.abs()));
    if largest > 0.0 { largest } else { 1.0 }
}

/// Mean of a non-empty finite window, accumulated on scaled values with
/// compensated summation so a finite window cannot overflow its partial sums.
fn scaled_mean(window: &[f64]) -> Result<f64> {
    let scale = magnitude_scale(window);
    let total = neumaier_sum(window.iter().map(|y| y / scale))?;
    Ok(scale * (total / window.len() as f64))
}

/// Equal-weight ordinary least squares over reporting-slot indices.
///
/// The slot indices `1..=n` are centered on their mean and scaled to
/// `[-1, 1]`, and the observations are divided by their largest magnitude,
/// so the normal sums are well conditioned and cannot overflow at large
/// levels. Sums use compensated accumulation.
struct LinearFit {
    /// Largest observation magnitude; fitted values are restored by it.
    scale: f64,
    /// Mean of the scaled observations: the fitted value at the centre.
    mean: f64,
    /// Slope per unit of the scaled coordinate.
    slope: f64,
    /// Number of observations.
    n: usize,
}

impl LinearFit {
    /// Fit at least two finite observations in slot order.
    fn of(data: &[f64]) -> Result<Self> {
        debug_assert!(data.len() >= 2);
        let n = data.len();
        let scale = magnitude_scale(data);
        let mean = neumaier_sum(data.iter().map(|y| y / scale))? / n as f64;
        let cross = neumaier_sum(
            data.iter()
                .enumerate()
                .map(|(i, y)| Self::coordinate(n, i as f64) * (y / scale - mean)),
        )?;
        let spread = neumaier_sum((0..n).map(|i| Self::coordinate(n, i as f64).powi(2)))?;
        Ok(Self {
            scale,
            mean,
            slope: cross / spread,
            n,
        })
    }

    /// Centered and scaled coordinate of zero-based slot `index`: the first
    /// observation is -1 and the last is 1.
    fn coordinate(n: usize, index: f64) -> f64 {
        let half = (n as f64 - 1.0) / 2.0;
        (index - half) / half
    }

    /// The fitted line at forecast step `step` (1 is the slot after the
    /// last observation).
    fn at_step(&self, step: usize) -> f64 {
        let coordinate = Self::coordinate(self.n, (self.n - 1 + step) as f64);
        self.scale * (self.mean + self.slope * coordinate)
    }
}

/// Double exponential smoothing (Holt's method), optionally damped.
///
/// With `phi = 1.0` this is classic Holt; with `phi < 1.0` the trend is
/// damped in both the smoothing recurrences and the forecast function
/// (Hyndman & Athanasopoulos, "Forecasting: Principles and Practice", §8.2):
/// `ℓ_t = α·y_t + (1−α)(ℓ_{t−1} + φ·b_{t−1})`,
/// `b_t = β(ℓ_t − ℓ_{t−1}) + (1−β)·φ·b_{t−1}`.
fn double_exponential_smoothing(data: &[f64], alpha: f64, beta: f64, phi: f64) -> (f64, f64) {
    // The public forecast entry requires at least two observations.
    debug_assert!(data.len() >= 2);

    let mut level = data[0];
    let init_window = data.len().min(10);
    let mut trend = (data[init_window - 1] - data[0]) / (init_window - 1) as f64;

    for &value in data.iter().skip(1) {
        let prev_level = level;
        level = alpha * value + (1.0 - alpha) * (level + phi * trend);
        trend = beta * (level - prev_level) + (1.0 - beta) * phi * trend;
    }

    (level, trend)
}

/// Apply a seasonal forecast with decomposition.
///
/// The supplied history is decomposed into a centered moving-average trend
/// and one effect per phase; the forecast grows the final trend and applies
/// the effect of each slot's phase. Phase is keyed by position in the
/// history and continues across every forecast slot.
///
/// # Arguments
///
/// * `forecast_periods` - Periods that require projected values
/// * `params` - Finite `historical` observations (oldest first, at least two
///   full seasons of consecutive slots ending at the forecast opening),
///   positive `season_length`, decomposition `mode`, and optional
///   **per-slot** compound trend `growth` (default 0.0): forecast step `h`
///   uses `last_trend * (1 + growth)^h`. Multiplicative mode requires
///   `growth > -1`.
pub(super) fn seasonal_forecast(
    forecast_periods: &[PeriodId],
    params: &SeasonalParams,
) -> Result<IndexMap<PeriodId, f64>> {
    let hist_data = params.historical.as_slice();
    finite_history(hist_data, "Seasonal forecast")?;

    let season_length = usize::try_from(params.season_length)
        .map_err(|_| Error::forecast("season_length exceeds the supported platform size"))?;
    if season_length == 0 {
        return Err(Error::forecast("season_length must be greater than 0"));
    }
    let minimum_history = season_length.checked_mul(2).ok_or_else(|| {
        Error::forecast("season_length is too large to represent two full seasons")
    })?;
    if hist_data.len() < minimum_history {
        return Err(Error::forecast(format!(
            "Need at least 2 full seasons of historical data for seasonal decomposition. \
             Season length: {}, need {} periods, got {}. \
             Provide more historical data or reduce season_length.",
            season_length,
            minimum_history,
            hist_data.len()
        )));
    }

    let mode = params.mode;
    let growth = params.growth.unwrap_or(0.0);
    if !growth.is_finite() || (mode == SeasonalMode::Multiplicative && growth <= -1.0) {
        return Err(Error::forecast(format!(
            "Seasonal 'growth' must be finite, and above -1 in multiplicative mode; got {growth}"
        )));
    }

    let trend = centered_trend(hist_data, season_length)?;
    let effects = match mode {
        SeasonalMode::Additive => additive_effects(hist_data, season_length, &trend)?,
        SeasonalMode::Multiplicative => multiplicative_factors(hist_data, season_length, &trend)?,
    };
    let last_trend = trend[trend.len() - 1];

    let mut results = IndexMap::new();
    let mut growth_factor = 1.0;
    for (i, period_id) in forecast_periods.iter().enumerate() {
        growth_factor *= 1.0 + growth;
        let trend_value = last_trend * growth_factor;
        // Phase continues from the history, one step per slot.
        let effect = effects[(hist_data.len() + i) % season_length];
        let value = match mode {
            SeasonalMode::Additive => trend_value + effect,
            SeasonalMode::Multiplicative => trend_value * effect,
        };
        if !value.is_finite() {
            return Err(Error::forecast(format!(
                "Seasonal forecast produced a non-finite value at period {:?}",
                period_id
            )));
        }
        results.insert(*period_id, value);
    }

    Ok(results)
}

/// Centered moving-average trend of at least two complete seasons.
///
/// For an odd season length `m` the `m` observations around a slot have
/// equal weight. For an even `m` the `m + 1` observations around it are
/// used with half weight at each endpoint, and the weighted sum is divided
/// by `m`. Each edge is extended linearly with the slope between its two
/// nearest centered points; the slope is used as computed.
fn centered_trend(data: &[f64], season_length: usize) -> Result<Vec<f64>> {
    debug_assert!(season_length > 0 && data.len() >= 2 * season_length);
    let n = data.len();
    let half = season_length / 2;
    let scale = magnitude_scale(data);
    let mut trend = vec![0.0; n];

    for (center, value) in trend.iter_mut().enumerate().take(n - half).skip(half) {
        let window = &data[center - half..=center + half];
        let weighted = if season_length.is_multiple_of(2) {
            // m + 1 observations, half weight at both ends.
            let last = window.len() - 1;
            neumaier_sum(window.iter().enumerate().map(|(k, y)| {
                let weight = if k == 0 || k == last { 0.5 } else { 1.0 };
                weight * (y / scale)
            }))?
        } else {
            neumaier_sum(window.iter().map(|y| y / scale))?
        };
        *value = scale * (weighted / season_length as f64);
    }

    // Two complete seasons give at least two centered points, except for a
    // season length of one, where every slot is its own centered point.
    if half > 0 {
        let (first, last) = (half, n - half - 1);
        let slope_front = trend[first + 1] - trend[first];
        let slope_back = trend[last] - trend[last - 1];
        for i in 0..half {
            trend[i] = trend[first] - slope_front * (first - i) as f64;
            trend[last + 1 + i] = trend[last] + slope_back * (i + 1) as f64;
        }
    }
    if trend.iter().any(|value| !value.is_finite()) {
        return Err(Error::forecast(
            "Seasonal decomposition produced a non-finite trend",
        ));
    }
    Ok(trend)
}

/// Mean of `values` at the slots of each phase: `values[phase]`,
/// `values[phase + m]`, and so on.
fn phase_means(values: &[f64], season_length: usize) -> Result<Vec<f64>> {
    (0..season_length)
        .map(|phase| {
            let slots = values.iter().skip(phase).step_by(season_length);
            let count = slots.clone().count();
            Ok(neumaier_sum(slots.copied())? / count as f64)
        })
        .collect()
}

/// Additive phase effects: the mean observation-minus-trend residual of
/// each phase, centered so the effects average zero across phases.
fn additive_effects(data: &[f64], season_length: usize, trend: &[f64]) -> Result<Vec<f64>> {
    let residuals: Vec<f64> = data.iter().zip(trend).map(|(y, t)| y - t).collect();
    if residuals.iter().any(|residual| !residual.is_finite()) {
        return Err(Error::forecast(
            "Seasonal decomposition produced a non-finite residual",
        ));
    }
    let mut effects = phase_means(&residuals, season_length)?;
    let mean = neumaier_sum(effects.iter().copied())? / season_length as f64;
    for effect in &mut effects {
        *effect -= mean;
    }
    Ok(effects)
}

/// Multiplicative phase factors: the mean observation-to-trend ratio of
/// each phase, normalized so the factors average one across phases.
///
/// The trend must be non-zero with one sign, and every observation must
/// share that sign or be exactly zero, so every ratio is non-negative.
fn multiplicative_factors(data: &[f64], season_length: usize, trend: &[f64]) -> Result<Vec<f64>> {
    let negative = trend[0] < 0.0;
    if trend.iter().any(|t| *t == 0.0 || (*t < 0.0) != negative) {
        return Err(Error::forecast(
            "Multiplicative seasonal decomposition is undefined for series whose trend \
             crosses or touches zero (the seasonal ratio divides by the trend). \
             Use mode 'additive' for series that change sign or hover near zero.",
        ));
    }
    if data.iter().any(|y| *y != 0.0 && (*y < 0.0) != negative) {
        return Err(Error::forecast(
            "Multiplicative seasonal decomposition requires every observation to share the \
             sign of the trend or be exactly zero. Use mode 'additive' for series that \
             change sign.",
        ));
    }
    let ratios: Vec<f64> = data.iter().zip(trend).map(|(y, t)| y / t).collect();
    let mut factors = phase_means(&ratios, season_length)?;
    let mean = neumaier_sum(factors.iter().copied())? / season_length as f64;
    if !(mean.is_finite() && mean > 0.0) || ratios.iter().any(|ratio| !ratio.is_finite()) {
        return Err(Error::forecast(
            "Multiplicative seasonal factors cannot be normalized: their average is not a \
             positive finite number",
        ));
    }
    for factor in &mut factors {
        *factor /= mean;
    }
    Ok(factors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trend(historical: Vec<f64>, method: ForecastTrendMethod) -> TimeSeriesParams {
        TimeSeriesParams {
            historical,
            method,
            alpha: None,
            beta: None,
            phi: None,
            window: None,
            min: None,
            max: None,
        }
    }

    fn seasonal(historical: Vec<f64>, season_length: u64, mode: SeasonalMode) -> SeasonalParams {
        SeasonalParams {
            historical,
            season_length,
            mode,
            growth: None,
            min: None,
            max: None,
        }
    }

    #[test]
    fn test_timeseries_forecast() {
        let params = trend(
            vec![100.0, 110.0, 120.0, 130.0],
            ForecastTrendMethod::Linear,
        );

        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];

        let result =
            timeseries_forecast(&periods, &params).expect("timeseries_forecast should succeed");

        // Should continue linear trend
        assert!(result[&PeriodId::quarter(2025, 1).expect("valid period fixture")] > 130.0);
        assert!(
            result[&PeriodId::quarter(2025, 2).expect("valid period fixture")]
                > result[&PeriodId::quarter(2025, 1).expect("valid period fixture")]
        );
    }

    #[test]
    fn test_seasonal_forecast_with_historical() {
        // Create historical data with seasonal pattern (2 years of quarterly data)
        let historical = vec![
            100.0, 90.0, 110.0, 85.0, // Year 1
            105.0, 95.0, 115.0, 90.0, // Year 2
        ];

        let params = SeasonalParams {
            growth: Some(0.02),
            ..seasonal(historical, 4, SeasonalMode::Multiplicative)
        };

        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            PeriodId::quarter(2025, 3).expect("valid period fixture"),
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        ];

        let result =
            seasonal_forecast(&periods, &params).expect("seasonal_forecast should succeed");

        assert!(result.len() == 4);

        let q1 = result[&PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let q2 = result[&PeriodId::quarter(2025, 2).expect("valid period fixture")];
        assert!(q1 > 0.0, "Q1 should be positive");
        assert!(q2 > 0.0, "Q2 should be positive");
    }

    #[test]
    fn test_linear_trend() {
        // A perfect line y = 10 t is extended exactly.
        let fit = LinearFit::of(&[10.0, 20.0, 30.0, 40.0]).unwrap();
        assert!((fit.at_step(1) - 50.0).abs() < 1e-12);
        assert!((fit.at_step(3) - 70.0).abs() < 1e-12);
    }

    /// At large levels the fit keeps the small trend: 1e15 + 3 t fitted on
    /// centered and scaled coordinates predicts 1e15 + 3 (n + h) to the
    /// spacing of doubles at 1e15 (0.125). A fit through raw sums of t y
    /// (about 1e16) would lose it.
    #[test]
    fn linear_fit_keeps_a_small_trend_at_a_large_level() {
        let data: Vec<f64> = (1..=8).map(|t| 1e15 + 3.0 * f64::from(t)).collect();
        let fit = LinearFit::of(&data).unwrap();
        for step in 1..=4_usize {
            let expected = 1e15 + 3.0 * (8 + step) as f64;
            assert!((fit.at_step(step) - expected).abs() <= 0.5, "{step}");
        }
        // Finite observations near the top of the range do not overflow the
        // normal sums: the mean of [1.5e308, 1.7e308] is 1.6e308.
        let extreme = LinearFit::of(&[1.5e308, 1.7e308, 1.5e308, 1.7e308]).unwrap();
        assert!(extreme.at_step(1).is_finite());
        assert!((scaled_mean(&[1.5e308, 1.7e308]).unwrap() / 1.6e308 - 1.0).abs() < 1e-12);
    }

    /// An independent fit with noisy data: the closed-form slope and
    /// intercept for t = 1..5, y = [2, 4, 5, 4, 5] are 0.6 and 2.2
    /// (Sxy = 6, Sxx = 10, means 3 and 4), so step 1 is 5.8 and step 2 is 6.4.
    #[test]
    fn linear_fit_matches_an_independent_closed_form() {
        let fit = LinearFit::of(&[2.0, 4.0, 5.0, 4.0, 5.0]).unwrap();
        assert!((fit.at_step(1) - 5.8).abs() < 1e-12);
        assert!((fit.at_step(2) - 6.4).abs() < 1e-12);
    }

    #[test]
    fn test_exponential_smoothing() {
        let data = vec![100.0, 110.0, 120.0, 130.0];
        let (level, trend) = double_exponential_smoothing(&data, 0.5, 0.5, 1.0);

        // Should detect upward trend
        assert!(level > 120.0);
        assert!(trend > 0.0);
    }

    fn holt_params(phi: Option<f64>) -> TimeSeriesParams {
        TimeSeriesParams {
            alpha: Some(0.3),
            beta: Some(0.1),
            phi,
            ..trend(
                vec![100.0, 110.0, 120.0, 130.0],
                ForecastTrendMethod::Exponential,
            )
        }
    }

    /// `phi = 1.0` must reproduce classic (undamped) Holt exactly — both the
    /// smoothing recurrences and the forecast function reduce to the
    /// pre-damping behavior.
    #[test]
    fn damped_holt_with_phi_one_matches_classic_holt() {
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            PeriodId::quarter(2025, 3).expect("valid period fixture"),
        ];
        let undamped = timeseries_forecast(&periods, &holt_params(None)).expect("classic Holt");
        let phi_one =
            timeseries_forecast(&periods, &holt_params(Some(1.0))).expect("phi = 1.0 Holt");
        for pid in &periods {
            assert!(
                (undamped[pid] - phi_one[pid]).abs() < 1e-12,
                "phi = 1.0 must be byte-identical to classic Holt at {pid:?}"
            );
        }
    }

    /// With `phi < 1` the h-step forecast is `ℓ + (φ + … + φ^h)·b`, which is
    /// bounded above by the asymptote `ℓ + φ/(1−φ)·b` — the trend levels off
    /// instead of extrapolating linearly.
    #[test]
    fn damped_holt_levels_off_at_asymptote() {
        let phi = 0.8;
        let periods: Vec<PeriodId> = (0..20)
            .map(|i| {
                PeriodId::quarter(2025 + i / 4, (i % 4 + 1) as u8).expect("valid period fixture")
            })
            .collect();
        let damped = timeseries_forecast(&periods, &holt_params(Some(phi))).expect("damped Holt");
        let undamped = timeseries_forecast(&periods, &holt_params(None)).expect("classic Holt");

        // Recompute the asymptote from the damped smoothing state.
        let (level, trend) =
            double_exponential_smoothing(&[100.0, 110.0, 120.0, 130.0], 0.3, 0.1, phi);
        let asymptote = level + trend * phi / (1.0 - phi);

        let last = periods[periods.len() - 1];
        assert!(
            damped[&last] < undamped[&last],
            "damping must pull long-horizon forecasts below the linear extrapolation"
        );
        assert!(
            damped[&last] <= asymptote + 1e-9,
            "damped forecast {:.6} must stay below its asymptote {asymptote:.6}",
            damped[&last]
        );
        // The 20-step forecast should be close to (but not beyond) the asymptote.
        assert!(
            (asymptote - damped[&last]) / asymptote.abs() < 0.05,
            "20-step damped forecast should approach the asymptote"
        );
    }

    #[test]
    fn damped_holt_rejects_out_of_range_phi() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        for phi in [0.0, -0.5, 1.5, f64::INFINITY] {
            let err = timeseries_forecast(&periods, &holt_params(Some(phi)))
                .expect_err("out-of-range phi must be rejected");
            assert!(err.to_string().contains("phi"), "{err}");
        }
    }

    #[test]
    fn phi_rejected_for_non_exponential_methods() {
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let params = TimeSeriesParams {
            phi: Some(0.9),
            ..trend(vec![100.0, 110.0, 120.0], ForecastTrendMethod::Linear)
        };
        let err = timeseries_forecast(&periods, &params)
            .expect_err("phi on a non-exponential method must be rejected");
        assert!(err.to_string().contains("only valid with method"), "{err}");
    }

    #[test]
    fn test_seasonal_decomposition() {
        let data = vec![
            100.0, 90.0, 110.0, 85.0, // Year 1
            105.0, 95.0, 115.0, 90.0, // Year 2
            110.0, 100.0, 120.0, 95.0, // Year 3
        ];

        let trend = centered_trend(&data, 4).expect("trend");
        let seasonal = additive_effects(&data, 4, &trend).expect("additive");

        assert_eq!(
            trend.len(),
            data.len(),
            "Trend should have same length as data"
        );
        assert_eq!(seasonal.len(), 4, "Seasonal should have 4 components");

        let seasonal_sum: f64 = seasonal.iter().sum();
        assert!(
            seasonal_sum.abs() < 1.0,
            "Seasonal components should sum to near zero"
        );

        let seasonal_max = seasonal.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let seasonal_min = seasonal.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        assert!(
            seasonal_max > seasonal_min,
            "Seasonal should have variation"
        );
    }

    #[test]
    fn test_seasonal_forecast_rejects_unsupported_season_start() {
        let err = serde_json::from_value::<crate::types::ForecastSpec>(serde_json::json!({
            "method": "seasonal",
            "params": {"historical": [0.0, 10.0, 0.0, 10.0], "season_length": 2, "mode": "additive", "season_start": 1},
        }))
            .expect_err("unknown parameter key must be rejected");
        assert!(err.to_string().contains("season_start"), "{err}");
    }

    #[test]
    fn test_moving_average_forecast_has_no_half_window_lag() {
        // Perfectly linear data y = 10x with window=2: ma = 35, trend = 10.
        // The trailing MA is centered (window-1)/2 = 0.5 periods behind the
        // last observation, so the anchored projection must continue the
        // line exactly: 50, 60 (the unanchored form lagged by trend/2 = 5).
        let params = TimeSeriesParams {
            window: Some(2),
            ..trend(
                vec![10.0, 20.0, 30.0, 40.0],
                ForecastTrendMethod::MovingAverage,
            )
        };
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
        ];

        let result =
            timeseries_forecast(&periods, &params).expect("timeseries_forecast should succeed");

        assert!(
            (result[&PeriodId::quarter(2025, 1).expect("valid period fixture")] - 50.0).abs()
                < 1e-9
        );
        assert!(
            (result[&PeriodId::quarter(2025, 2).expect("valid period fixture")] - 60.0).abs()
                < 1e-9
        );
    }

    #[test]
    fn test_timeseries_forecast_rejects_unknown_param_keys() {
        let err = serde_json::from_value::<crate::types::ForecastSpec>(serde_json::json!({
            "method": "time_series",
            "params": {"historical": [100.0, 110.0, 120.0], "method": "moving_average", "windoww": 2},
        }))
            .expect_err("unknown parameter key must be rejected");
        assert!(err.to_string().contains("windoww"), "{err}");
    }

    #[test]
    fn test_seasonal_forecast_rejects_unknown_param_keys() {
        let err = serde_json::from_value::<crate::types::ForecastSpec>(serde_json::json!({
            "method": "seasonal",
            "params": {"historical": [100.0, 90.0, 110.0, 85.0], "season_length": 2, "mode": "additive", "seasonstart": 1},
        }))
            .expect_err("unknown parameter key must be rejected");
        assert!(err.to_string().contains("seasonstart"), "{err}");
    }

    #[test]
    fn test_seasonal_forecast_rejects_zero_season_length() {
        let params = seasonal(vec![100.0, 90.0, 110.0, 85.0], 0, SeasonalMode::Additive);
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let result = seasonal_forecast(&periods, &params);
        assert!(result.is_err(), "zero season length must be rejected");
    }

    #[test]
    fn test_timeseries_forecast_rejects_non_finite_history_entries() {
        let params = trend(vec![100.0, f64::NAN, 110.0], ForecastTrendMethod::Linear);
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let result = timeseries_forecast(&periods, &params);
        assert!(
            result.is_err(),
            "malformed historical series should fail instead of being compacted"
        );
    }

    #[test]
    fn test_multiplicative_seasonal_forecast_preserves_relative_factors() {
        // Stable-level data with clear seasonal pattern (3 years).
        // Seasonal ratios: Q1=1.0, Q2=0.8, Q3=1.2, Q4=0.9 relative to Q1.
        let params = SeasonalParams {
            growth: Some(0.0),
            ..seasonal(
                vec![
                    100.0, 80.0, 120.0, 90.0, //
                    102.0, 82.0, 122.0, 92.0, //
                    104.0, 84.0, 124.0, 94.0,
                ],
                4,
                SeasonalMode::Multiplicative,
            )
        };
        let periods = vec![
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            PeriodId::quarter(2025, 3).expect("valid period fixture"),
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        ];

        let result =
            seasonal_forecast(&periods, &params).expect("seasonal_forecast should succeed");

        let q1 = result[&PeriodId::quarter(2025, 1).expect("valid period fixture")];
        let q2 = result[&PeriodId::quarter(2025, 2).expect("valid period fixture")];
        let q3 = result[&PeriodId::quarter(2025, 3).expect("valid period fixture")];
        let q4 = result[&PeriodId::quarter(2025, 4).expect("valid period fixture")];

        assert!((q2 / q1 - 0.8).abs() < 0.05, "q2/q1={}", q2 / q1);
        assert!((q3 / q1 - 1.2).abs() < 0.05, "q3/q1={}", q3 / q1);
        assert!((q4 / q1 - 0.9).abs() < 0.05, "q4/q1={}", q4 / q1);
    }

    #[test]
    fn multiplicative_mode_rejects_zero_crossing_trend() {
        // A sign-alternating series centred on zero has a zero trend; the
        // multiplicative ratio is undefined there and must fail loudly
        // instead of silently substituting identity factors.
        let params = seasonal(
            vec![-10.0, 10.0, -10.0, 10.0, -10.0, 10.0, -10.0, 10.0],
            4,
            SeasonalMode::Multiplicative,
        );
        let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];

        let err = seasonal_forecast(&periods, &params)
            .expect_err("zero-crossing trend must be rejected in multiplicative mode");
        assert!(
            err.to_string().contains("additive"),
            "error should point the user at additive mode: {err}"
        );
    }

    fn quarters(count: u8) -> Vec<PeriodId> {
        (1..=count)
            .map(|q| PeriodId::quarter(2026, q).expect("valid period fixture"))
            .collect()
    }

    /// Two exact cycles of a linear trend plus a zero-sum pattern decompose
    /// exactly: the centered average of `2 t + s` is `2 t` in the interior,
    /// the edges extend that line, and the effects are `s`. The forecast is
    /// the final trend (16 at t = 8) plus the effect of each phase.
    #[test]
    fn two_exact_even_cycles_recover_trend_and_effects() {
        let pattern = [3.0, -1.0, -4.0, 2.0];
        let data: Vec<f64> = (0..8)
            .map(|i| 2.0 * (i + 1) as f64 + pattern[i % 4])
            .collect();
        let trend = centered_trend(&data, 4).expect("trend");
        for (i, value) in trend.iter().enumerate() {
            assert!((value - 2.0 * (i + 1) as f64).abs() < 1e-12, "{i}: {value}");
        }
        let effects = additive_effects(&data, 4, &trend).expect("effects");
        for (effect, expected) in effects.iter().zip(pattern) {
            assert!((effect - expected).abs() < 1e-12);
        }
        let periods = quarters(4);
        let forecast =
            seasonal_forecast(&periods, &seasonal(data, 4, SeasonalMode::Additive)).unwrap();
        for (period, expected) in periods.iter().zip(pattern) {
            assert!((forecast[period] - (16.0 + expected)).abs() < 1e-12);
        }
    }

    /// An odd season length weights its `m` observations equally:
    /// `3 t + s` with `s = [-1, 0, 1]` over two cycles has trend `3 t`.
    #[test]
    fn two_exact_odd_cycles_recover_trend_and_effects() {
        let pattern = [-1.0, 0.0, 1.0];
        let data: Vec<f64> = (0..6)
            .map(|i| 3.0 * (i + 1) as f64 + pattern[i % 3])
            .collect();
        let trend = centered_trend(&data, 3).expect("trend");
        for (i, value) in trend.iter().enumerate() {
            assert!((value - 3.0 * (i + 1) as f64).abs() < 1e-12, "{i}: {value}");
        }
        let periods = quarters(4);
        let forecast =
            seasonal_forecast(&periods, &seasonal(data, 3, SeasonalMode::Additive)).unwrap();
        // Phase continues from the history: steps 1..4 are phases 0, 1, 2, 0.
        for (period, expected) in periods.iter().zip([17.0, 18.0, 19.0, 17.0]) {
            assert!((forecast[period] - expected).abs() < 1e-12);
        }
    }

    /// A tiny edge slope is kept as computed, not snapped to zero.
    #[test]
    fn edge_slopes_are_not_snapped_to_zero() {
        let data: Vec<f64> = (0..8).map(|i| 1e-12 * (i + 1) as f64).collect();
        let trend = centered_trend(&data, 4).expect("trend");
        assert!(trend[7] > trend[6] && trend[0] < trend[1], "{trend:?}");
        assert!((trend[7] - 8e-12).abs() < 1e-24);
    }

    /// Multiplicative factors are non-negative ratios normalized to average
    /// one. A zero observation is valid: the pattern [0, 20, 30, 50] around
    /// a level of 25 has factors [0, 0.8, 1.2, 2.0].
    #[test]
    fn multiplicative_mode_accepts_a_zero_observation() {
        let data = vec![0.0, 20.0, 30.0, 50.0, 0.0, 20.0, 30.0, 50.0];
        let trend = centered_trend(&data, 4).expect("trend");
        let factors = multiplicative_factors(&data, 4, &trend).expect("factors");
        for (factor, expected) in factors.iter().zip([0.0, 0.8, 1.2, 2.0]) {
            assert!((factor - expected).abs() < 1e-12);
        }
        let periods = quarters(4);
        let grown = SeasonalParams {
            growth: Some(0.1),
            ..seasonal(data, 4, SeasonalMode::Multiplicative)
        };
        let forecast = seasonal_forecast(&periods, &grown).unwrap();
        // The trend of 25 grows 10% per slot before the factor applies.
        for (h, (period, factor)) in periods.iter().zip([0.0, 0.8, 1.2, 2.0]).enumerate() {
            let expected = 25.0 * 1.1_f64.powi(h as i32 + 1) * factor;
            assert!((forecast[period] - expected).abs() < 1e-9);
        }
    }

    /// A negative trend with observations of that sign is a valid
    /// multiplicative series; an observation of the other sign is not, and
    /// neither is growth at or below -100%.
    #[test]
    fn multiplicative_mode_states_its_sign_domain() {
        let periods = quarters(4);
        let negative = vec![-10.0, -20.0, -30.0, -40.0, -10.0, -20.0, -30.0, -40.0];
        let forecast = seasonal_forecast(
            &periods,
            &seasonal(negative, 4, SeasonalMode::Multiplicative),
        )
        .unwrap();
        for (period, expected) in periods.iter().zip([-10.0, -20.0, -30.0, -40.0]) {
            assert!((forecast[period] - expected).abs() < 1e-9);
        }
        let mixed = vec![-1.0, 20.0, 30.0, 51.0, -1.0, 20.0, 30.0, 51.0];
        let error = seasonal_forecast(&periods, &seasonal(mixed, 4, SeasonalMode::Multiplicative))
            .expect_err("an observation against the trend's sign");
        assert!(error.to_string().contains("sign of the trend"), "{error}");
        let collapsing = SeasonalParams {
            growth: Some(-1.0),
            ..seasonal(vec![1.0; 8], 4, SeasonalMode::Multiplicative)
        };
        assert!(seasonal_forecast(&periods, &collapsing).is_err());
    }
}
