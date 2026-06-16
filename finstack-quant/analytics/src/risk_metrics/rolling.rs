//! Rolling risk metrics: Sharpe, Sortino, and volatility over a sliding window.
//!
//! Crate-internal except for [`DatedSeries`] (re-exported at the crate root).
//! `///` doc examples target crate developers and are marked `ignore`.
//!
//! All rolling functions share O(n) sliding-window kernels and produce either
//! a dated struct (aligned to window-end dates) or a NaN-padded `Vec<f64>`.

use crate::dates::Date;
use finstack_quant_core::math::neumaier_sum;

use super::return_based::{invalid_annualization_factor, sharpe};

/// Number of slide steps before the incremental rolling kernel fully
/// recomputes its running moments from the current window.
///
/// The rolling kernels update their moments incrementally on each slide,
/// which accumulates floating-point drift over time. Every
/// `ROLLING_KERNEL_RECOMPUTE_INTERVAL` steps we recompute those values
/// over the full window to restore precision.
const ROLLING_KERNEL_RECOMPUTE_INTERVAL: usize = 1024;

#[inline]
fn recompute_mean_m2(window: &[f64]) -> (f64, f64) {
    let mut mean = 0.0_f64;
    let mut m2 = 0.0_f64;
    let mut count = 0.0_f64;
    for &value in window {
        count += 1.0;
        let delta = value - mean;
        mean += delta / count;
        let delta2 = value - mean;
        m2 += delta * delta2;
    }
    (mean, m2)
}

#[inline]
fn recompute_sum_sum_ds(window: &[f64], mar: f64) -> (f64, f64) {
    (
        neumaier_sum(window.iter().copied()),
        neumaier_sum(window.iter().filter(|&&r| r < mar).map(|&r| {
            let d = mar - r;
            d * d
        })),
    )
}

/// A dated time-series column: scalar values aligned with window-end dates.
///
/// Shared carrier type for rolling analytics outputs. Concrete metrics
/// (rolling Sharpe, Sortino, volatility, etc.) re-use this struct via
/// this struct so they share field names, serde shape, and helper methods.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DatedSeries {
    /// Computed metric values, one per completed rolling window.
    pub values: Vec<f64>,
    /// Window-end dates aligned 1:1 with `values`.
    pub dates: Vec<Date>,
}

impl DatedSeries {
    /// Allocate an empty series with capacity for `cap` points.
    #[inline]
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            values: Vec::with_capacity(cap),
            dates: Vec::with_capacity(cap),
        }
    }
}

fn nan_series(dates: &[Date], start: usize, len: usize) -> DatedSeries {
    DatedSeries {
        values: vec![f64::NAN; len],
        dates: dates[start..start + len].to_vec(),
    }
}

/// Rolling Sharpe ratio over a sliding window.
///
/// Computes the Sharpe ratio independently for each `window`-length sub-slice
/// of `returns`, advancing one period at a time. Produces `n - window + 1`
/// values where `n = min(returns.len(), dates.len())`.
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns.
/// * `dates` - Date vector aligned with `returns`. Used only for labelling
///   output; must be at least as long as `returns`.
/// * `window` - Look-back window length in periods.
/// * `ann_factor` - Number of periods per year for annualization.
/// * `risk_free_rate` - Annualized risk-free rate to subtract.
///
/// # Returns
///
/// A [`DatedSeries`] with `values` and `dates` of equal length. Returns
/// empty vectors if `window` is zero or larger than the series length.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_analytics::risk_metrics::rolling_sharpe;
/// use finstack_quant_core::dates::{Date, Duration, Month};
///
/// let returns: Vec<f64> = (0..20).map(|i| (i as f64 - 10.0) * 0.001).collect();
/// let dates: Vec<Date> = (0..20)
///     .map(|i| Date::from_calendar_date(2025, Month::January, 1).unwrap()
///         + Duration::days(i))
///     .collect();
/// let rs = rolling_sharpe(&returns, &dates, 5, 252.0, 0.0);
/// assert_eq!(rs.values.len(), 16); // 20 − 5 + 1
/// ```
pub(crate) fn rolling_sharpe(
    returns: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
    risk_free_rate: f64,
) -> DatedSeries {
    let n = returns.len().min(dates.len());
    if n < window || window == 0 {
        return DatedSeries::default();
    }
    if invalid_annualization_factor(true, ann_factor) {
        return nan_series(dates, window - 1, n - window + 1);
    }
    let w = window as f64;
    let mut out = DatedSeries::with_capacity(n - window + 1);
    let mut date_idx = window - 1;
    rolling_mean_m2_kernel(returns, n, window, |mean, m2| {
        let ann_mean = mean * ann_factor;
        let var = if window == 1 {
            0.0
        } else {
            (m2 / (w - 1.0)).max(0.0)
        };
        let ann_vol = var.sqrt() * ann_factor.sqrt();
        out.values.push(sharpe(ann_mean, ann_vol, risk_free_rate));
        out.dates.push(dates[date_idx]);
        date_idx += 1;
    });
    out
}

/// Rolling annualized volatility over a sliding window.
///
/// Computes annualized volatility independently for each `window`-length
/// sub-slice, advancing one period at a time.
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `dates`      - Date vector aligned with `returns`.
/// * `window`     - Look-back window length in periods.
/// * `ann_factor` - Number of periods per year for annualization.
///
/// # Returns
///
/// A [`DatedSeries`] with `n - window + 1` values. Returns empty
/// vectors if `window` is zero or larger than the series length.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_analytics::risk_metrics::rolling_volatility;
/// use finstack_quant_core::dates::{Date, Duration, Month};
///
/// let returns: Vec<f64> = (0..20).map(|i| (i as f64 - 10.0) * 0.001).collect();
/// let dates: Vec<Date> = (0..20)
///     .map(|i| Date::from_calendar_date(2025, Month::January, 1).unwrap()
///         + Duration::days(i))
///     .collect();
/// let rv = rolling_volatility(&returns, &dates, 5, 252.0);
/// assert_eq!(rv.values.len(), 16);
/// ```
pub(crate) fn rolling_volatility(
    returns: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
) -> DatedSeries {
    let n = returns.len().min(dates.len());
    if n < window || window == 0 {
        return DatedSeries::default();
    }
    if invalid_annualization_factor(true, ann_factor) {
        return nan_series(dates, window - 1, n - window + 1);
    }
    let w = window as f64;
    let mut out = DatedSeries::with_capacity(n - window + 1);
    let mut date_idx = window - 1;
    rolling_mean_m2_kernel(returns, n, window, |_, m2| {
        let var = if window == 1 {
            0.0
        } else {
            (m2 / (w - 1.0)).max(0.0)
        };
        out.values.push(var.sqrt() * ann_factor.sqrt());
        out.dates.push(dates[date_idx]);
        date_idx += 1;
    });
    out
}

/// Rolling Sortino ratio over a sliding window.
///
/// Computes the Sortino ratio independently for each `window`-length
/// sub-slice, advancing one period at a time.
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `dates`      - Date vector aligned with `returns`.
/// * `window`     - Look-back window length in periods.
/// * `ann_factor` - Number of periods per year for annualization.
/// * `mar`        - Minimum acceptable per-period return. Downside is
///   measured as `Σ max(mar − r, 0)²` within each window.
///
/// # Returns
///
/// A [`DatedSeries`] with `n - window + 1` values. Returns empty
/// vectors if `window` is zero or larger than the series length.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_analytics::risk_metrics::rolling_sortino;
/// use finstack_quant_core::dates::{Date, Duration, Month};
///
/// let returns: Vec<f64> = (0..20).map(|i| (i as f64 - 10.0) * 0.001).collect();
/// let dates: Vec<Date> = (0..20)
///     .map(|i| Date::from_calendar_date(2025, Month::January, 1).unwrap()
///         + Duration::days(i))
///     .collect();
/// let rs = rolling_sortino(&returns, &dates, 5, 252.0, 0.0);
/// assert_eq!(rs.values.len(), 16);
/// ```
pub(crate) fn rolling_sortino(
    returns: &[f64],
    dates: &[Date],
    window: usize,
    ann_factor: f64,
    mar: f64,
) -> DatedSeries {
    let n = returns.len().min(dates.len());
    if n < window || window == 0 {
        return DatedSeries::default();
    }
    if invalid_annualization_factor(true, ann_factor) || !mar.is_finite() {
        return nan_series(dates, window - 1, n - window + 1);
    }
    let w = window as f64;
    let mut out = DatedSeries::with_capacity(n - window + 1);
    let mut date_idx = window - 1;
    rolling_sortino_kernel(returns, n, window, mar, |sum, sum_ds| {
        let m = sum / w - mar;
        let dd = (sum_ds / w).sqrt();
        out.values.push(if dd == 0.0 {
            if m > 0.0 {
                f64::INFINITY
            } else if m < 0.0 {
                f64::NEG_INFINITY
            } else {
                0.0
            }
        } else {
            (m * ann_factor) / (dd * ann_factor.sqrt())
        });
        out.dates.push(dates[date_idx]);
        date_idx += 1;
    });
    out
}

// ── Internal rolling kernels ──
//
// Each kernel advances a sliding window over `returns[..n]` and calls
// `emit` once per completed window.  Both the dated (struct) and undated
// (NaN-padded Vec) public functions delegate here; they share *identical*
// arithmetic and differ only in how the output is assembled.

/// Shared kernel for rolling metrics that only need window mean and M2.
///
/// Maintains `(mean, m2)` accumulators and calls `emit(mean, m2)` for every
/// completed window starting at index `window-1`.
fn rolling_mean_m2_kernel<F>(returns: &[f64], n: usize, window: usize, mut emit: F)
where
    F: FnMut(f64, f64),
{
    if window == 1 {
        for &value in &returns[..n] {
            emit(value, 0.0);
        }
        return;
    }

    let window_n = window as f64;
    let (mut mean, mut m2) = recompute_mean_m2(&returns[..window]);
    emit(mean, m2);
    let mut steps_since_recompute = 0_usize;
    for i in window..n {
        let add = returns[i];
        let rem = returns[i - window];

        let keep_n = window_n - 1.0;
        let mean_after_rem = (window_n * mean - rem) / keep_n;
        let m2_after_rem = m2 - (rem - mean) * (rem - mean_after_rem);

        let delta = add - mean_after_rem;
        mean = mean_after_rem + delta / window_n;
        m2 = (m2_after_rem + delta * (add - mean)).max(0.0);

        steps_since_recompute += 1;
        if steps_since_recompute >= ROLLING_KERNEL_RECOMPUTE_INTERVAL {
            let start = i + 1 - window;
            (mean, m2) = recompute_mean_m2(&returns[start..=i]);
            steps_since_recompute = 0;
        }
        emit(mean, m2);
    }
}

/// Kernel for the Sortino sliding window.
///
/// Maintains `(sum, sum_ds)` where `sum_ds = Σ min(r,0)²` and calls
/// `emit(sum, sum_ds)` for every completed window.
fn rolling_sortino_kernel<F>(returns: &[f64], n: usize, window: usize, mar: f64, mut emit: F)
where
    F: FnMut(f64, f64),
{
    let (mut sum, mut sum_ds) = recompute_sum_sum_ds(&returns[..window], mar);
    emit(sum, sum_ds);
    let mut steps_since_recompute = 0_usize;
    for i in window..n {
        let add = returns[i];
        let rem = returns[i - window];
        sum += add - rem;
        if add < mar {
            let d = mar - add;
            sum_ds += d * d;
        }
        if rem < mar {
            let d = mar - rem;
            sum_ds -= d * d;
        }
        sum_ds = sum_ds.max(0.0); // guard against floating-point underflow
        steps_since_recompute += 1;
        if steps_since_recompute >= ROLLING_KERNEL_RECOMPUTE_INTERVAL {
            let start = i + 1 - window;
            (sum, sum_ds) = recompute_sum_sum_ds(&returns[start..=i], mar);
            steps_since_recompute = 0;
        }
        emit(sum, sum_ds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::{Duration, Month};
    use crate::risk_metrics::return_based::{sortino, volatility};
    fn jan1(year: i32) -> Date {
        Date::from_calendar_date(year, Month::January, 1).expect("valid date")
    }

    #[test]
    fn rolling_sharpe_window() {
        let returns: Vec<f64> = (0..20).map(|i| (i as f64 - 10.0) * 0.001).collect();
        let dates: Vec<Date> = (0..20).map(|i| jan1(2025) + Duration::days(i)).collect();
        let rs = rolling_sharpe(&returns, &dates, 5, 252.0, 0.0);
        assert_eq!(rs.values.len(), 16);
    }

    #[test]
    fn rolling_volatility_window_count() {
        let returns: Vec<f64> = (0..20).map(|i| (i as f64 - 10.0) * 0.001).collect();
        let dates: Vec<Date> = (0..20).map(|i| jan1(2025) + Duration::days(i)).collect();
        let rv = rolling_volatility(&returns, &dates, 5, 252.0);
        assert_eq!(rv.values.len(), 16);
        assert_eq!(rv.dates.len(), 16);
        assert!(rv.values.iter().all(|&v| v > 0.0));
    }

    #[test]
    fn rolling_volatility_matches_pointwise() {
        let returns: Vec<f64> = (0..10).map(|i| (i as f64 - 5.0) * 0.01).collect();
        let dates: Vec<Date> = (0..10).map(|i| jan1(2025) + Duration::days(i)).collect();
        let rv = rolling_volatility(&returns, &dates, 5, 252.0);
        let first_window = volatility(&returns[0..5], true, 252.0);
        assert!((rv.values[0] - first_window).abs() < 1e-12);
    }

    #[test]
    fn rolling_volatility_empty_window() {
        let rv = rolling_volatility(&[0.01], &[jan1(2025)], 5, 252.0);
        assert!(rv.values.is_empty());
    }

    #[test]
    fn rolling_metrics_return_nan_values_for_invalid_ann_factor() {
        let returns: Vec<f64> = (0..10).map(|i| (i as f64 - 5.0) * 0.01).collect();
        let dates: Vec<Date> = (0..10).map(|i| jan1(2025) + Duration::days(i)).collect();

        let rv = rolling_volatility(&returns, &dates, 5, 0.0);
        assert_eq!(rv.values.len(), 6);
        assert_eq!(rv.dates.len(), 6);
        assert!(rv.values.iter().all(|value| value.is_nan()));

        let rs = rolling_sharpe(&returns, &dates, 5, -252.0, 0.0);
        assert_eq!(rs.values.len(), 6);
        assert_eq!(rs.dates.len(), 6);
        assert!(rs.values.iter().all(|value| value.is_nan()));

        let rso = rolling_sortino(&returns, &dates, 5, f64::INFINITY, 0.0);
        assert_eq!(rso.values.len(), 6);
        assert_eq!(rso.dates.len(), 6);
        assert!(rso.values.iter().all(|value| value.is_nan()));
    }

    #[test]
    fn rolling_sortino_window_count() {
        let returns: Vec<f64> = (0..20).map(|i| (i as f64 - 10.0) * 0.001).collect();
        let dates: Vec<Date> = (0..20).map(|i| jan1(2025) + Duration::days(i)).collect();
        let rs = rolling_sortino(&returns, &dates, 5, 252.0, 0.0);
        assert_eq!(rs.values.len(), 16);
        assert_eq!(rs.dates.len(), 16);
    }

    #[test]
    fn rolling_sortino_matches_pointwise() {
        let returns: Vec<f64> = (0..10).map(|i| (i as f64 - 5.0) * 0.01).collect();
        let dates: Vec<Date> = (0..10).map(|i| jan1(2025) + Duration::days(i)).collect();
        let rs = rolling_sortino(&returns, &dates, 5, 252.0, 0.0);
        let first_window = sortino(&returns[0..5], true, 252.0, 0.0);
        assert!((rs.values[0] - first_window).abs() < 1e-12);
    }

    #[test]
    fn rolling_sortino_mar_matches_pointwise() {
        // Oscillating returns so every 5-window contains values below MAR.
        let returns: Vec<f64> = (0..12)
            .map(|i: usize| if i.is_multiple_of(2) { -0.01 } else { 0.015 })
            .collect();
        let dates: Vec<Date> = (0..12).map(|i| jan1(2025) + Duration::days(i)).collect();
        let mar = 0.005;
        let rs = rolling_sortino(&returns, &dates, 5, 252.0, mar);
        for window_start in 0..=returns.len() - 5 {
            let expected = sortino(&returns[window_start..window_start + 5], true, 252.0, mar);
            let got = rs.values[window_start];
            assert!(
                (got - expected).abs() < 1e-10,
                "window {window_start}: got {got}, expected {expected}",
            );
        }
    }

    #[test]
    fn rolling_volatility_stays_stable_under_large_offset() {
        let mean = 1.0e8;
        let sigma = 1.0e-3;
        let returns: Vec<f64> = (0..300)
            .map(|i| match i % 3 {
                0 => mean - sigma,
                1 => mean,
                _ => mean + sigma,
            })
            .collect();
        let dates: Vec<Date> = (0..returns.len())
            .map(|i| jan1(2025) + Duration::days(i as i64))
            .collect();

        let rv = rolling_volatility(&returns, &dates, 3, 1.0);
        assert!(!rv.values.is_empty());
        for &value in &rv.values {
            assert!(
                (value - sigma).abs() < 1.0e-6,
                "expected rolling volatility near {sigma}, got {value}"
            );
        }
    }
}
