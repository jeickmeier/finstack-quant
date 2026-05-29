//! Drawdown computation: series, episode detection, averaging, and CDaR.
//!
//! Crate-internal except for [`DrawdownEpisode`] (re-exported at the crate
//! root). `///` doc examples target crate developers and are marked `ignore`.
//!
//! Drawdown measures the peak-to-trough decline in cumulative wealth.
//! This module provides five levels of granularity:
//! - [`to_drawdown_series`]: per-period drawdown depth as a time series.
//! - [`drawdown_details`]: structured episodes (start, valley, recovery).
//! - [`mean_episode_drawdown`]: scalar average of the worst N episode minima.
//! - [`mean_drawdown`]: arithmetic mean of a drawdown path.
//! - [`cdar`]: Conditional Drawdown at Risk at a given confidence level.

use crate::dates::Date;
use crate::math::stats::quantile;

/// Drawdown episode with start, valley, optional recovery, and max drawdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DrawdownEpisode {
    /// Date when the drawdown began (peak date).
    pub start: Date,
    /// Date of the maximum drawdown depth.
    pub valley: Date,
    /// Date when wealth recovered to the prior peak (None if still in drawdown).
    pub end: Option<Date>,
    /// Calendar days from start to end (or last observation).
    pub duration_days: i64,
    /// Maximum drawdown depth (negative fraction, e.g. −0.25 for a 25% loss).
    pub max_drawdown: f64,
    /// Near-recovery threshold: the drawdown level at which 99% of the
    /// peak-to-trough loss has been recovered (i.e. `max_drawdown * 0.01`,
    /// a value slightly below zero). Useful for identifying "almost recovered"
    /// drawdowns where the series is within 1% of the prior peak.
    pub near_recovery_threshold: f64,
    /// `true` when the series began already in drawdown (no observed prior
    /// peak), so `start` is the first observation rather than a true peak.
    /// Survival / duration analytics should treat such episodes as
    /// left-censored.
    #[serde(default)]
    pub truncated_at_start: bool,
}

/// Compute a drawdown series from a simple-return series.
///
/// At each time step `i`, the drawdown depth is:
///
/// ```text
/// dd[i] = wealth[i] / peak[i] - 1  (≤ 0)
/// ```ignore
///
/// where `wealth[i] = Π(1 + r[j]) for j ≤ i` and `peak[i]` is the running
/// maximum of wealth up to and including `i`.
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns (e.g., `0.01` = +1 %).
///   Returns below `−1.0` drive wealth negative, which is mathematically
///   valid for leveraged positions but physically meaningless for long-only
///   portfolios. Callers should validate inputs if negative wealth is
///   not expected. `NaN` values propagate silently through all downstream
///   drawdown metrics.
///
/// # Returns
///
/// A `Vec<f64>` of the same length as `returns`. Each value is ≤ 0;
/// a value of `0.0` means wealth is at or above its prior peak.
/// Returns an empty vector if `returns` is empty.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::to_drawdown_series;
///
/// // No drawdown while returns are positive.
/// let dd = to_drawdown_series(&[0.10, 0.05]);
/// assert!(dd.iter().all(|&v| v.abs() < 1e-12));
///
/// // 20% loss after a 10% gain: wealth = 1.10 * 0.80 = 0.88 → dd ≈ −0.2
/// let dd = to_drawdown_series(&[0.10, -0.20]);
/// assert!(dd[1] < -0.18);
/// ```
pub(crate) fn to_drawdown_series(returns: &[f64]) -> Vec<f64> {
    if returns.is_empty() {
        return vec![];
    }
    let mut wealth = 1.0;
    let mut peak = 1.0;
    let mut dd = Vec::with_capacity(returns.len());
    for &r in returns {
        wealth *= 1.0 + r;
        if wealth > peak {
            peak = wealth;
        }
        dd.push(wealth / peak - 1.0);
    }
    dd
}

/// Detect individual drawdown episodes from a drawdown series.
///
/// An episode begins when the drawdown series first drops below zero
/// (i.e., wealth falls from its peak) and ends when it returns to zero
/// (recovery to a new peak). The function collects all such episodes,
/// sorts them by severity (most negative `max_drawdown` first), and
/// returns the worst `n`.
///
/// # Arguments
///
/// * `drawdown` - Pre-computed drawdown series, as produced by [`to_drawdown_series`].
/// * `dates`    - Date vector aligned with `drawdown`. Must be the same length or longer.
/// * `n`        - Maximum number of episodes to return.
///
/// # Returns
///
/// Up to `n` [`DrawdownEpisode`] structs sorted by `max_drawdown` ascending
/// (most severe first). Returns an empty vector if `drawdown` or `dates` is empty.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::{drawdown_details, to_drawdown_series};
/// use finstack_core::dates::{Date, Month};
///
/// let returns = [0.10, -0.20, 0.05, 0.10];
/// let dd = to_drawdown_series(&returns);
/// let dates: Vec<Date> = (1..=4)
///     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
///     .collect();
/// let episodes = drawdown_details(&dd, &dates, 5);
/// assert!(!episodes.is_empty());
/// assert!(episodes[0].max_drawdown < 0.0);
/// ```
pub(crate) fn drawdown_details(drawdown: &[f64], dates: &[Date], n: usize) -> Vec<DrawdownEpisode> {
    if drawdown.is_empty() || dates.is_empty() {
        return vec![];
    }
    let len = drawdown.len().min(dates.len());
    let dates = &dates[..len];

    let mut episodes: Vec<DrawdownEpisode> = Vec::new();
    for_each_episode(
        &drawdown[..len],
        |start_idx, valley_idx, end_idx, valley_val, truncated_at_start| {
            episodes.push(make_episode(
                dates,
                start_idx,
                valley_idx,
                end_idx,
                valley_val,
                len - 1,
                truncated_at_start,
            ));
        },
    );

    episodes.sort_by(|a, b| {
        a.max_drawdown
            .partial_cmp(&b.max_drawdown)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    episodes.truncate(n);
    episodes
}

/// Shared drawdown-episode state machine.
///
/// Scans `drawdown` for episodes (contiguous runs where `d < -1e-15`) and
/// invokes `emit(start_idx, valley_idx, end_idx, valley_val)` for each
/// detected episode. `start_idx` is the index of the peak that preceded
/// the episode (or `0` if the series begins in drawdown); `end_idx` is
/// `Some(i)` when the episode recovers at index `i`, or `None` when the
/// episode extends to the end of the series.
fn for_each_episode<F>(drawdown: &[f64], mut emit: F)
where
    F: FnMut(usize, usize, Option<usize>, f64, bool),
{
    let mut in_dd = false;
    let mut start_idx = 0usize;
    let mut truncated_at_start = false;
    let mut valley_idx = 0usize;
    let mut valley_val = 0.0_f64;

    for (i, &d) in drawdown.iter().enumerate() {
        if d < -1e-15 {
            if !in_dd {
                in_dd = true;
                if i > 0 {
                    start_idx = i - 1;
                    truncated_at_start = false;
                } else {
                    start_idx = 0;
                    truncated_at_start = true;
                }
                valley_idx = i;
                valley_val = d;
            } else if d < valley_val {
                valley_idx = i;
                valley_val = d;
            }
        } else if in_dd {
            emit(
                start_idx,
                valley_idx,
                Some(i),
                valley_val,
                truncated_at_start,
            );
            in_dd = false;
        }
    }
    if in_dd {
        emit(start_idx, valley_idx, None, valley_val, truncated_at_start);
    }
}

#[allow(clippy::too_many_arguments)]
fn make_episode(
    dates: &[Date],
    start_idx: usize,
    valley_idx: usize,
    end_idx: Option<usize>,
    valley_val: f64,
    last_data_idx: usize,
    truncated_at_start: bool,
) -> DrawdownEpisode {
    let start = dates[start_idx];
    let valley = dates[valley_idx];
    let end = end_idx.map(|i| dates[i]);
    let end_date = end.unwrap_or(dates[last_data_idx]);
    let duration_days = (end_date - start).whole_days();
    DrawdownEpisode {
        start,
        valley,
        end,
        duration_days,
        max_drawdown: valley_val,
        near_recovery_threshold: valley_val * 0.01,
        truncated_at_start,
    }
}

/// Average of the top-N worst drawdown episode minima.
///
/// Identifies the `n` largest drawdown episodes directly from the drawdown
/// path and returns the arithmetic mean of their episode minima.
///
/// This is the **episode-based** average used in the Sterling ratio
/// (Kestner, 1996). For the simple arithmetic mean of the full drawdown
/// path, use [`mean_drawdown`] instead.
///
/// # Arguments
///
/// * `drawdown` - Pre-computed drawdown series (values ≤ 0).
/// * `n`        - Number of worst episodes to average.
///
/// # Returns
///
/// Mean of the `n` worst `max_drawdown` values (a negative number), or
/// `0.0` if `drawdown` is empty or no episodes are found.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::{mean_episode_drawdown, to_drawdown_series};
///
/// let returns = [0.05, -0.15, 0.10, -0.08, 0.03];
/// let dd = to_drawdown_series(&returns);
/// let avg = mean_episode_drawdown(&dd, 3);
/// assert!(avg < 0.0);
/// ```
#[must_use]
pub(crate) fn mean_episode_drawdown(drawdown: &[f64], n: usize) -> f64 {
    let episode_depths = worst_episode_depths(drawdown, n);
    if episode_depths.is_empty() {
        return 0.0;
    }
    let sum: f64 = episode_depths.iter().sum();
    sum / episode_depths.len() as f64
}

/// Extract the worst `n` episode depths from a drawdown series, sorted
/// ascending (most severe first). Shared by [`mean_episode_drawdown`] and
/// [`drawdown_details`]-derived callers.
fn worst_episode_depths(drawdown: &[f64], n: usize) -> Vec<f64> {
    if drawdown.is_empty() {
        return vec![];
    }

    let mut depths = Vec::new();
    for_each_episode(drawdown, |_, _, _, valley_val, _| depths.push(valley_val));
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    depths.truncate(n);
    depths
}

/// Maximum drawdown duration in calendar days across all episodes.
///
/// Identifies all drawdown episodes and returns the longest `duration_days`.
///
/// # Arguments
///
/// * `drawdown` - Pre-computed drawdown series (values ≤ 0).
/// * `dates`    - Date vector aligned with `drawdown`.
///
/// # Returns
///
/// Duration in calendar days of the longest drawdown episode. Returns `0`
/// if no episodes are found.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::{max_drawdown_duration, to_drawdown_series};
/// use finstack_core::dates::{Date, Month};
///
/// let returns = [0.10, -0.20, 0.05, 0.10, -0.05, -0.03];
/// let dd = to_drawdown_series(&returns);
/// let dates: Vec<Date> = (1..=6)
///     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
///     .collect();
/// let max_dur = max_drawdown_duration(&dd, &dates);
/// assert!(max_dur > 0);
/// ```
pub(crate) fn max_drawdown_duration(drawdown: &[f64], dates: &[Date]) -> i64 {
    if drawdown.is_empty() || dates.is_empty() {
        return 0;
    }
    let len = drawdown.len().min(dates.len());
    let last_idx = len - 1;
    let mut max_days: i64 = 0;
    for_each_episode(&drawdown[..len], |start_idx, _, end_idx, _, _| {
        let end_date = match end_idx {
            Some(i) => dates[i],
            None => dates[last_idx],
        };
        let days = (end_date - dates[start_idx]).whole_days();
        if days > max_days {
            max_days = days;
        }
    });
    max_days
}

/// Conditional Drawdown at Risk (CDaR) at the given confidence level.
///
/// The expected drawdown depth in the tail beyond the `(1 − α)` quantile
/// of the drawdown distribution:
///
/// ```text
/// CDaR_α = E[ |dd| | |dd| ≥ q_{1−α}(|dd|) ]
/// ```ignore
///
/// CDaR is the drawdown analogue of Expected Shortfall (CVaR).
///
/// # Arguments
///
/// * `drawdown`   - Pre-computed drawdown series (values ≤ 0), as produced
///   by [`to_drawdown_series`].
/// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95`.
///
/// # Returns
///
/// The CDaR as a **non-positive** scalar, matching the sign convention of
/// the rest of the drawdown family ([`max_drawdown`], [`mean_drawdown`]).
/// For example, a 95% CDaR of `-0.25` means the average drawdown depth in
/// the worst 5% tail is 25%.
/// Returns `0.0` for an empty slice.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::cdar;
///
/// let dd = [-0.01, -0.05, -0.10, -0.15, -0.20, 0.0, -0.03, -0.08, -0.12, -0.18];
/// let c = cdar(&dd, 0.80);
/// assert!(c < -0.10);
/// ```
///
/// # References
///
/// - Chekhlov, Uryasev & Zabarankin (2005): see docs/REFERENCES.md#chekhlov2005
#[must_use]
pub(crate) fn cdar(drawdown: &[f64], confidence: f64) -> f64 {
    if drawdown.is_empty() {
        return 0.0;
    }
    let mut abs_dd: Vec<f64> = drawdown.iter().map(|&d| d.abs()).collect();
    let threshold = quantile(&mut abs_dd, confidence);
    // Note: `abs_dd` is partially reordered by `quantile` (nth_element partition),
    // not sorted. The fold below is order-independent so this is correct.
    let (sum, count) = abs_dd
        .iter()
        .filter(|&&d| d >= threshold)
        .fold((0.0_f64, 0usize), |(s, n), &d| (s + d, n + 1));
    let tail_avg = if count == 0 {
        threshold
    } else {
        sum / count as f64
    };
    -tail_avg
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::dates::{Duration, Month};

    fn make_dates(n: usize) -> Vec<Date> {
        (0..n)
            .map(|i| {
                Date::from_calendar_date(2025, Month::January, 1).expect("valid date")
                    + Duration::days(i as i64)
            })
            .collect()
    }

    #[test]
    fn drawdown_series_no_loss() {
        let r = [0.01, 0.02, 0.03];
        let dd = to_drawdown_series(&r);
        assert!(dd.iter().all(|&v| v.abs() < 1e-12));
    }

    #[test]
    fn drawdown_series_basic() {
        let r = [0.10, -0.20, 0.05, 0.10];
        let dd = to_drawdown_series(&r);
        assert_eq!(dd.len(), 4);
        assert!(dd[0].abs() < 1e-12); // no DD after gain
        assert!(dd[1] < -0.1); // DD after big loss
    }

    #[test]
    fn drawdown_details_basic() {
        let r = [0.10, -0.20, 0.05, 0.10, -0.05, -0.03];
        let dd = to_drawdown_series(&r);
        let dates = make_dates(r.len());
        let episodes = drawdown_details(&dd, &dates, 5);
        assert!(!episodes.is_empty());
        assert!(episodes[0].max_drawdown < 0.0);
    }

    #[test]
    fn drawdown_details_bounds_unrecovered_episode_to_aligned_dates() {
        let drawdown = [0.0, -0.10, -0.20, -0.15];
        let dates = make_dates(6);
        let episodes = drawdown_details(&drawdown, &dates, 5);
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].start, dates[0]);
        assert_eq!(episodes[0].valley, dates[2]);
        assert_eq!(episodes[0].end, None);
        assert_eq!(episodes[0].duration_days, 3);
    }

    #[test]
    fn mean_episode_drawdown_empty() {
        assert_eq!(mean_episode_drawdown(&[], 5), 0.0);
    }

    #[test]
    fn mean_episode_drawdown_deterministic() {
        let drawdown = [0.0, -0.10, -0.20, 0.0, -0.15, 0.0];
        // Two episodes: depths −0.20 and −0.15; average of worst 2 = (−0.20 + −0.15)/2
        let avg = mean_episode_drawdown(&drawdown, 2);
        assert!((avg - (-0.175)).abs() < 1e-12);
    }

    #[test]
    fn cdar_hand_calc() {
        // dd = [−0.10, −0.20, −0.05, −0.15, −0.25, −0.30, −0.02, −0.08, −0.12, −0.18]
        // abs = [0.10, 0.20, 0.05, 0.15, 0.25, 0.30, 0.02, 0.08, 0.12, 0.18]
        // sorted: [0.02, 0.05, 0.08, 0.10, 0.12, 0.15, 0.18, 0.20, 0.25, 0.30]
        // quantile(0.80): h = 9*0.8 = 7.2, lo=7 (0.20), hi=8 (0.25), frac=0.2
        // threshold = 0.20 + 0.2*(0.25−0.20) = 0.21
        // Tail (abs_dd ≥ 0.21): [0.25, 0.30]
        // CDaR = (0.25 + 0.30) / 2 = 0.275
        let dd = [
            -0.10, -0.20, -0.05, -0.15, -0.25, -0.30, -0.02, -0.08, -0.12, -0.18,
        ];
        let c = cdar(&dd, 0.80);
        assert!((c - (-0.275)).abs() < 1e-10);
    }

    #[test]
    fn cdar_worse_than_max_drawdown_var() {
        // CDaR at any confidence ≥ the quantile threshold (it's a tail average,
        // returned with non-positive sign).
        let dd = [
            -0.01, -0.05, -0.10, -0.15, -0.20, 0.0, -0.03, -0.08, -0.12, -0.18,
        ];
        let c95 = cdar(&dd, 0.95);
        let c80 = cdar(&dd, 0.80);
        // Higher confidence → fewer, more extreme tail observations → more
        // negative CDaR (larger in magnitude).
        assert!(c95 <= c80);
    }

    #[test]
    fn cdar_empty() {
        assert_eq!(cdar(&[], 0.95), 0.0);
    }

    #[test]
    fn cdar_no_drawdown() {
        let dd = [0.0, 0.0, 0.0];
        assert_eq!(cdar(&dd, 0.95), 0.0);
    }

    #[test]
    fn cdar_uniform_drawdown() {
        // All drawdowns identical at −5% → CDaR = −5% regardless of confidence
        let dd = [-0.05; 20];
        let c = cdar(&dd, 0.95);
        assert!((c - (-0.05)).abs() < 1e-12);
    }
}

// ── Drawdown-derived risk ratios ──
//
// These functions take a pre-computed drawdown series or summary scalars
// derived from one, making them natural companions to the drawdown primitives
// already in this module.

/// Ulcer index: root-mean-square of the drawdown series.
///
/// Measures the depth and duration of drawdowns from a pre-computed
/// drawdown series. A higher Ulcer Index indicates more persistent or
/// deeper losses ("investor distress").
///
/// ```text
/// UI = sqrt(mean(dd_i^2))
/// ```ignore
///
/// # Arguments
///
/// * `drawdown` - Pre-computed drawdown series (values ≤ 0), as produced
///   by [`to_drawdown_series`].
///
/// # Returns
///
/// The Ulcer Index (a non-negative scalar). Returns `0.0` for an empty slice.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::ulcer_index;
///
/// // Flat drawdown of −10% throughout → UI = 0.10.
/// let dd = [-0.10, -0.10, -0.10];
/// assert!((ulcer_index(&dd) - 0.10).abs() < 1e-12);
/// ```
///
/// # References
///
/// - Martin (1987): see docs/REFERENCES.md#martinUlcer1987
#[must_use]
pub(crate) fn ulcer_index(drawdown: &[f64]) -> f64 {
    if drawdown.is_empty() {
        return 0.0;
    }
    let ss: f64 = drawdown.iter().map(|&d| d * d).sum();
    (ss / drawdown.len() as f64).sqrt()
}

/// Pain index: mean absolute drawdown over the full series.
///
/// ```text
/// Pain = (1/n) Σ |dd_i|
/// ```ignore
///
/// Less sensitive to outlier drawdowns than max drawdown.
///
/// # Arguments
///
/// * `drawdown` - Pre-computed drawdown series (values ≤ 0).
///
/// # Returns
///
/// The pain index (non-negative). Returns `0.0` for an empty slice.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::pain_index;
///
/// let dd = [-0.05, -0.10, 0.0, -0.03];
/// let pi = pain_index(&dd);
/// assert!((pi - 0.045).abs() < 1e-12);
/// ```
#[must_use]
pub(crate) fn pain_index(drawdown: &[f64]) -> f64 {
    if drawdown.is_empty() {
        return 0.0;
    }
    let sum: f64 = drawdown.iter().map(|&d| d.abs()).sum();
    sum / drawdown.len() as f64
}

/// Maximum drawdown depth from a pre-computed drawdown series.
///
/// Returns the most negative value in `drawdown`, or `0.0` for an empty slice.
///
/// To compute directly from a returns series, compose with
/// [`to_drawdown_series`]: `max_drawdown(&to_drawdown_series(&returns))`.
#[must_use]
pub(crate) fn max_drawdown(drawdown: &[f64]) -> f64 {
    drawdown.iter().copied().fold(0.0_f64, f64::min)
}

/// Arithmetic mean of a pre-computed drawdown path.
///
/// Since drawdown values are non-positive, the result is typically negative
/// or zero. Returns `0.0` for an empty slice.
///
/// For the average of the worst-N *episode* minima (the Sterling-style
/// measure), use [`mean_episode_drawdown`] instead.
///
/// # Arguments
///
/// * `drawdowns` - Slice of per-period drawdown depths (from
///   [`to_drawdown_series`]).
#[must_use]
pub(crate) fn mean_drawdown(drawdowns: &[f64]) -> f64 {
    if drawdowns.is_empty() {
        0.0
    } else {
        drawdowns.iter().copied().sum::<f64>() / drawdowns.len() as f64
    }
}

/// `numerator / denominator` with signed infinity when `denominator == 0.0`
/// and `0.0` when both are zero. Shared by all six drawdown-derived ratios.
#[inline]
fn ratio_or_sign_infinity(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        if numerator > 0.0 {
            f64::INFINITY
        } else if numerator < 0.0 {
            f64::NEG_INFINITY
        } else {
            0.0
        }
    } else {
        numerator / denominator
    }
}

/// Calmar ratio = CAGR / |max drawdown|.
///
/// Compares annualized growth against the worst peak-to-trough loss,
/// making it particularly useful for evaluating trend-following strategies.
///
/// # Arguments
///
/// * `cagr_val` - Compound annual growth rate (already computed).
/// * `max_dd` - Maximum drawdown depth (a negative number, e.g., `-0.25`
///   for a 25% drawdown).
///
/// # Returns
///
/// The Calmar ratio (positive when CAGR and max drawdown have the same sign).
/// Returns `f64::INFINITY` if `max_dd` is zero and `cagr_val` is positive,
/// `f64::NEG_INFINITY` if negative, or `0.0` if both are zero.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::calmar;
///
/// // 15% CAGR with 30% max drawdown → Calmar ≈ 0.5
/// assert!((calmar(0.15, -0.30) - 0.5).abs() < 1e-12);
/// assert_eq!(calmar(0.15, 0.0), f64::INFINITY);
/// ```
///
/// # References
///
/// - Young (1991): see docs/REFERENCES.md#youngCalmar1991
#[must_use]
pub(crate) fn calmar(cagr_val: f64, max_dd: f64) -> f64 {
    ratio_or_sign_infinity(cagr_val, max_dd.abs())
}

/// Recovery factor: total return / |max drawdown|.
///
/// Measures how many times the portfolio has recovered its worst loss.
/// A higher value indicates greater resilience.
///
/// # Arguments
///
/// * `total_return` - Total compounded return over the period.
/// * `max_dd`       - Maximum drawdown (negative number, e.g. `−0.25`).
///
/// # Returns
///
/// The recovery factor. Returns `f64::INFINITY` if `max_dd` is zero and
/// `total_return` is positive, `f64::NEG_INFINITY` if negative, or `0.0`
/// if both are zero.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::recovery_factor;
///
/// // 50% total return with 25% max drawdown → 2.0.
/// assert!((recovery_factor(0.50, -0.25) - 2.0).abs() < 1e-12);
/// assert_eq!(recovery_factor(0.50, 0.0), f64::INFINITY);
/// ```
#[must_use]
pub(crate) fn recovery_factor(total_return: f64, max_dd: f64) -> f64 {
    ratio_or_sign_infinity(total_return, max_dd.abs())
}

/// Martin ratio (Ulcer Performance Index): CAGR / Ulcer Index.
///
/// Measures return per unit of drawdown-based risk. Named after Peter
/// Martin who introduced the Ulcer Index.
///
/// # Arguments
///
/// * `cagr_val` - Compound annual growth rate.
/// * `ulcer`    - Ulcer Index (from [`ulcer_index`]).
///
/// # Returns
///
/// The Martin ratio. Sentinels follow the shared drawdown-ratio convention:
///
/// - `±∞` when `ulcer == 0.0` and `cagr_val ≠ 0.0` (positive numerator → `+∞`,
///   negative → `−∞`)
/// - `0.0` when both `cagr_val == 0.0` and `ulcer == 0.0`
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::martin_ratio;
///
/// assert!((martin_ratio(0.10, 0.05) - 2.0).abs() < 1e-12);
/// assert_eq!(martin_ratio(0.10, 0.0), f64::INFINITY);
/// assert_eq!(martin_ratio(0.0, 0.0), 0.0);
/// ```
///
/// # References
///
/// - Martin (1987): see docs/REFERENCES.md#martinUlcer1987
#[must_use]
pub(crate) fn martin_ratio(cagr_val: f64, ulcer: f64) -> f64 {
    ratio_or_sign_infinity(cagr_val, ulcer)
}

/// Sterling ratio: risk-adjusted return using average drawdown.
///
/// ```text
/// Sterling = (CAGR − R_f) / |mean_episode_drawdown|
/// ```ignore
///
/// # Arguments
///
/// * `cagr_val`       - Compound annual growth rate.
/// * `avg_dd`         - Average of the top-N worst drawdowns (negative number).
/// * `risk_free_rate` - Annualized risk-free rate.
///
/// # Returns
///
/// The Sterling ratio. Returns `±∞` if `avg_dd` is zero and the excess
/// return is nonzero, or `0.0` if both are zero.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::sterling_ratio;
///
/// // 12% CAGR, 2% risk-free, −10% avg drawdown → 1.0.
/// assert!((sterling_ratio(0.12, -0.10, 0.02) - 1.0).abs() < 1e-12);
/// ```
///
/// # References
///
/// - Kestner (1996): see docs/REFERENCES.md#kestner1996
#[must_use]
pub(crate) fn sterling_ratio(cagr_val: f64, avg_dd: f64, risk_free_rate: f64) -> f64 {
    ratio_or_sign_infinity(cagr_val - risk_free_rate, avg_dd.abs())
}

/// Burke ratio: return per unit of drawdown-based risk (RMS of drawdowns).
///
/// ```text
/// Burke = (CAGR − R_f) / sqrt( (1/n) Σ dd_i² )
/// ```ignore
///
/// where `dd_i` are the max-drawdown depths of the top-N episodes.
///
/// # Arguments
///
/// * `cagr_val`       - Compound annual growth rate.
/// * `dd_episodes`    - Slice of max-drawdown values from each episode (negative).
/// * `risk_free_rate` - Annualized risk-free rate.
///
/// # Returns
///
/// The Burke ratio. Returns `0.0` if `dd_episodes` is empty, or `±∞`
/// if the RMS of episodes is zero with nonzero excess return.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::burke_ratio;
///
/// let dds = [-0.10, -0.05, -0.03];
/// let b = burke_ratio(0.15, &dds, 0.02);
/// assert!(b > 0.0);
/// ```
///
/// # References
///
/// - Burke (1994): see docs/REFERENCES.md#burke1994
#[must_use]
pub(crate) fn burke_ratio(cagr_val: f64, dd_episodes: &[f64], risk_free_rate: f64) -> f64 {
    if dd_episodes.is_empty() {
        return 0.0;
    }
    let n = dd_episodes.len() as f64;
    let ss: f64 = dd_episodes.iter().map(|&d| d * d).sum();
    let rms = (ss / n).sqrt();
    ratio_or_sign_infinity(cagr_val - risk_free_rate, rms)
}

/// Pain ratio: return per unit of average drawdown pain.
///
/// ```text
/// Pain Ratio = (CAGR − R_f) / Pain Index
/// ```ignore
///
/// # Arguments
///
/// * `cagr_val`       - Compound annual growth rate.
/// * `pain`           - Pain index (from [`pain_index`]).
/// * `risk_free_rate` - Annualized risk-free rate.
///
/// # Returns
///
/// The pain ratio. Returns `±∞` if the pain index is zero and the
/// excess return is nonzero, or `0.0` if both are zero.
///
/// # Examples
///
/// ```ignore
/// use finstack_analytics::drawdown::pain_ratio;
///
/// assert!((pain_ratio(0.10, 0.05, 0.02) - 1.6).abs() < 1e-12);
/// ```
#[must_use]
pub(crate) fn pain_ratio(cagr_val: f64, pain: f64, risk_free_rate: f64) -> f64 {
    ratio_or_sign_infinity(cagr_val - risk_free_rate, pain)
}

#[cfg(test)]
mod drawdown_ratio_tests {
    use super::*;

    #[test]
    fn ulcer_index_flat() {
        let dd = [-0.10, -0.10, -0.10];
        assert!((ulcer_index(&dd) - 0.10).abs() < 1e-12);
    }

    #[test]
    fn ulcer_index_empty() {
        assert_eq!(ulcer_index(&[]), 0.0);
    }

    #[test]
    fn pain_index_hand_calc() {
        let dd = [-0.05, -0.10, 0.0, -0.03];
        let pi = pain_index(&dd);
        assert!((pi - 0.045).abs() < 1e-14);
    }

    #[test]
    fn pain_index_empty() {
        assert_eq!(pain_index(&[]), 0.0);
    }

    #[test]
    fn calmar_hand_calc() {
        assert!((calmar(0.15, -0.30) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn calmar_zero_dd() {
        assert_eq!(calmar(0.15, 0.0), f64::INFINITY);
        assert_eq!(calmar(-0.05, 0.0), f64::NEG_INFINITY);
        assert_eq!(calmar(0.0, 0.0), 0.0);
    }

    #[test]
    fn recovery_factor_hand_calc() {
        assert!((recovery_factor(0.50, -0.25) - 2.0).abs() < 1e-12);
        assert!((recovery_factor(-0.10, -0.30) - (-1.0 / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn recovery_factor_zero_dd() {
        assert_eq!(recovery_factor(0.50, 0.0), f64::INFINITY);
        assert_eq!(recovery_factor(-0.10, 0.0), f64::NEG_INFINITY);
        assert_eq!(recovery_factor(0.0, 0.0), 0.0);
    }

    #[test]
    fn martin_ratio_hand_calc() {
        assert!((martin_ratio(0.10, 0.05) - 2.0).abs() < 1e-12);
        assert_eq!(martin_ratio(0.10, 0.0), f64::INFINITY);
        assert_eq!(martin_ratio(-0.05, 0.0), f64::NEG_INFINITY);
        assert_eq!(martin_ratio(0.0, 0.0), 0.0);
    }

    #[test]
    fn sterling_ratio_hand_calc() {
        assert!((sterling_ratio(0.12, -0.10, 0.02) - 1.0).abs() < 1e-12);
        assert!((sterling_ratio(0.15, -0.06, 0.03) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn sterling_ratio_zero_dd() {
        assert_eq!(sterling_ratio(0.12, 0.0, 0.02), f64::INFINITY);
        assert_eq!(sterling_ratio(0.01, 0.0, 0.02), f64::NEG_INFINITY);
        assert_eq!(sterling_ratio(0.02, 0.0, 0.02), 0.0);
    }

    #[test]
    fn burke_ratio_hand_calc() {
        let dds = [-0.10, -0.10];
        let b = burke_ratio(0.12, &dds, 0.02);
        assert!((b - 1.0).abs() < 1e-12);
    }

    #[test]
    fn burke_ratio_empty() {
        assert_eq!(burke_ratio(0.15, &[], 0.02), 0.0);
    }

    #[test]
    fn pain_ratio_hand_calc() {
        assert!((pain_ratio(0.10, 0.05, 0.02) - 1.6).abs() < 1e-12);
        assert!((pain_ratio(0.08, 0.04, 0.0) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn pain_ratio_zero_pain() {
        assert_eq!(pain_ratio(0.10, 0.0, 0.0), f64::INFINITY);
        assert_eq!(pain_ratio(-0.02, 0.0, 0.0), f64::NEG_INFINITY);
        assert_eq!(pain_ratio(0.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn drawdown_composite_helpers_compose_correctly() {
        let returns = [0.01, -0.02, 0.015, -0.005, 0.012, 0.008];
        let ann = 252.0;
        let cagr_val =
            crate::risk_metrics::cagr(&returns, crate::risk_metrics::CagrBasis::factor(ann))
                .expect("valid CAGR");
        let dd = to_drawdown_series(&returns);
        let max_dd = dd.iter().copied().fold(0.0_f64, f64::min);
        let ulcer = ulcer_index(&dd);
        let pain = pain_index(&dd);
        let avg_episode_dd = mean_episode_drawdown(&dd, usize::MAX);

        assert!(recovery_factor(crate::returns::comp_total(&returns), max_dd).is_finite());
        assert!(martin_ratio(cagr_val, ulcer).is_finite());
        assert!(sterling_ratio(cagr_val, avg_episode_dd, 0.01).is_finite());
        assert!(pain_ratio(cagr_val, pain, 0.01).is_finite());
    }
}
