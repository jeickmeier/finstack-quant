//! Scalar function implementations for `CompiledExpr`.
//!
//! Contains per-function evaluation logic (lag, lead, diff, rolling_*, ewm_*,
//! cumulative aggregations, rank, quantile, etc.) separated from the core DAG
//! execution engine in `eval.rs`.
//!
//! # NaN policy (unified reducer conventions)
//!
//! - **Global reducers skip NaN**: `median`, `quantile`, `std`, and `var`
//!   exclude NaN observations from both the sample and the count `n`. An
//!   all-NaN (or empty) input broadcasts NaN; `std`/`var` additionally require
//!   at least two valid observations.
//! - **Rolling mean/sum/std/var propagate NaN**: a NaN anywhere in the window
//!   makes that window's output NaN (matching pandas' rolling default of
//!   `min_periods = window`).
//! - **`rolling_median`, `rolling_min`, `rolling_max`, `rolling_count` skip**
//!   missing values within the window; an all-NaN window yields NaN
//!   (count yields the number of finite values, possibly 0).
//! - **Cumulative ops** (`cumsum`, `cumprod`, `cummin`, `cummax`) skip NaN and
//!   carry the previous accumulator forward.
//! - **EWM functions** seed from the first non-NaN observation; leading NaNs
//!   emit NaN, and interior NaNs emit NaN without updating the recursion state.
//!
//! # Division-by-zero conventions
//!
//! `pct_change` and the `/` operator intentionally differ:
//!
//! - `pct_change(x, n)`: a zero base with a non-zero current value returns
//!   `±inf` (sign of the current value), and `0/0` returns `0.0` (no change).
//! - The binary `/` operator returns NaN for any division by zero.
//!
//! # Parameter arguments
//!
//! Window/step parameters (the second argument to `lag`, `diff`, `rolling_*`,
//! etc.) must be **constant series** — normally `Expr::literal`, which
//! broadcasts a constant. Passing a non-constant series (e.g. a data column)
//! is a validation error. Other scalar parameters (`ewm_*` alpha/adjust,
//! `quantile` q, `shift` n) read the first element of their argument series
//! by convention.

use super::eval::CompiledExpr;
use crate::math::{
    finite_count, finite_max_or_nan, finite_min_or_nan, quantile_linear_or_nan, NeumaierAccumulator,
};

impl CompiledExpr {
    #[inline]
    fn validate_window(raw: f64) -> Option<usize> {
        if !raw.is_finite() {
            return None;
        }
        if raw < 1.0 {
            return None;
        }
        if raw.fract() != 0.0 {
            return None;
        }
        // `usize::MAX as f64` rounds up to 2^64 on 64-bit hosts, so accepting
        // equality would cast an unrepresentable offset to `usize::MAX`.
        if raw >= usize::MAX as f64 {
            return None;
        }
        Some(raw as usize)
    }

    /// Read a scalar from a series that must be constant across every element.
    ///
    /// `series` is non-empty. A series whose elements are not bit-identical is
    /// a validation error. `what` is the argument name embedded in that error.
    #[inline]
    fn constant_scalar(series: &[f64], what: &str) -> crate::Result<f64> {
        let first = series[0];
        if series.iter().any(|v| v.to_bits() != first.to_bits()) {
            return Err(crate::Error::Validation(format!(
                "{what} must be a constant scalar (literal); got a non-constant series"
            )));
        }
        Ok(first)
    }

    /// Resolve a window/step parameter from the second argument series.
    ///
    /// Window/step arguments must be constant across the series — typically an
    /// `Expr::literal`, which broadcasts a constant. If a non-constant series
    /// (e.g. a data column) is supplied, a [`crate::Error::Validation`] is
    /// returned instead of silently using only the first element.
    ///
    /// Returns `Ok(None)` when the (constant) value is not a valid window —
    /// non-finite, `< 1`, or fractional — or when the argument is absent and
    /// no `default` exists; callers emit all-NaN output in that case (the
    /// engine's invalid-parameter convention).
    #[inline]
    fn window_arg(arg_results: &[&[f64]], default: Option<usize>) -> crate::Result<Option<usize>> {
        match arg_results.get(1) {
            Some(series) if !series.is_empty() => {
                let first = Self::constant_scalar(series, "expression window/step argument")?;
                Ok(Self::validate_window(first))
            }
            _ => Ok(default),
        }
    }

    #[inline]
    fn rolling_apply_into(
        base: &[f64],
        win: usize,
        out: &mut [f64],
        op: &mut impl FnMut(&[f64]) -> f64,
    ) {
        let len = base.len();
        if win == 0 {
            out.fill(f64::NAN);
            return;
        }
        debug_assert_eq!(out.len(), len);
        for i in 0..len {
            if i + 1 < win {
                out[i] = f64::NAN;
            } else {
                out[i] = op(&base[i + 1 - win..=i]);
            }
        }
    }

    /// Resolve a rolling window, or fill `out` and return `Ok(())` when the
    /// window is absent or invalid.
    ///
    /// An empty output returns before the window is validated, matching the
    /// historical rolling entry points: a zero-length series does not reject a
    /// non-constant window argument.
    #[inline]
    fn rolling_with(
        arg_results: &[&[f64]],
        out: &mut [f64],
        op: impl FnOnce(&[f64], usize, &mut [f64]),
    ) -> crate::Result<()> {
        if out.is_empty() {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        op(arg_results[0], win, out);
        Ok(())
    }

    pub(super) fn eval_lag_into(arg_results: &[&[f64]], out: &mut [f64]) -> crate::Result<()> {
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let Some(n) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        let base = arg_results[0];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = if i < n {
                f64::NAN
            } else {
                base.get(i - n).copied().unwrap_or(f64::NAN)
            };
        }
        Ok(())
    }

    pub(super) fn eval_lead_into(arg_results: &[&[f64]], out: &mut [f64]) -> crate::Result<()> {
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let Some(n) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        let base = arg_results[0];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = i
                .checked_add(n)
                .and_then(|index| base.get(index))
                .copied()
                .unwrap_or(f64::NAN);
        }
        Ok(())
    }

    pub(super) fn eval_diff_into(arg_results: &[&[f64]], out: &mut [f64]) -> crate::Result<()> {
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(n) = Self::window_arg(arg_results, Some(1))? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = match (base.get(i), i.checked_sub(n).and_then(|j| base.get(j))) {
                (Some(&cur), Some(&prev)) => cur - prev,
                _ => f64::NAN,
            };
        }
        Ok(())
    }

    /// Percentage change over step `n`.
    ///
    /// Division-by-zero convention (intentionally different from the binary
    /// `/` operator, which returns NaN on any zero divisor): a zero base with
    /// a non-zero current value returns `±inf` (sign of the current value),
    /// and `0/0` returns `0.0` (interpreted as "no change").
    pub(super) fn eval_pct_change_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(n) = Self::window_arg(arg_results, Some(1))? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = match (base.get(i), i.checked_sub(n).and_then(|j| base.get(j))) {
                (Some(&0.0), Some(&0.0)) => 0.0,
                (Some(&cur), Some(&0.0)) => cur.signum() * f64::INFINITY,
                (Some(&cur), Some(&prev)) => (cur / prev) - 1.0,
                _ => f64::NAN,
            };
        }
        Ok(())
    }

    /// Left-to-right cumulative scan. NaNs are skipped and the previous
    /// accumulator is written through.
    ///
    /// `cumsum` seeds `0.0` and adds. `cumprod` seeds `1.0` and multiplies.
    /// `cummin` seeds `+∞`. `cummax` seeds `-∞`. A missing input fills `out`
    /// with NaN. A result shorter than `out` NaN-fills the tail.
    fn eval_cum_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
        mut acc: f64,
        mut step: impl FnMut(f64, f64) -> f64,
    ) {
        let Some(base) = arg_results.first() else {
            out.fill(f64::NAN);
            return;
        };
        let n = out.len().min(base.len());
        for i in 0..n {
            let value = base[i];
            if !value.is_nan() {
                acc = step(acc, value);
            }
            out[i] = acc;
        }
        out[n..].fill(f64::NAN);
    }

    pub(super) fn eval_cum_sum_into(arg_results: &[&[f64]], out: &mut [f64]) {
        Self::eval_cum_into(arg_results, out, 0.0, |acc, value| acc + value);
    }

    pub(super) fn eval_cum_prod_into(arg_results: &[&[f64]], out: &mut [f64]) {
        Self::eval_cum_into(arg_results, out, 1.0, |acc, value| acc * value);
    }

    pub(super) fn eval_cum_min_into(arg_results: &[&[f64]], out: &mut [f64]) {
        Self::eval_cum_into(arg_results, out, f64::INFINITY, |acc, value| {
            if acc < value {
                acc
            } else {
                value
            }
        });
    }

    pub(super) fn eval_cum_max_into(arg_results: &[&[f64]], out: &mut [f64]) {
        Self::eval_cum_into(arg_results, out, f64::NEG_INFINITY, |acc, value| {
            if acc > value {
                acc
            } else {
                value
            }
        });
    }

    pub(super) fn eval_rolling_mean_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        Self::rolling_with(arg_results, out, |base, win, out| {
            Self::rolling_sum_incremental(base, win, out);
            let width = win as f64;
            for value in out.iter_mut() {
                *value /= width;
            }
        })
    }

    pub(super) fn eval_rolling_sum_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        Self::rolling_with(arg_results, out, |base, win, out| {
            Self::rolling_sum_incremental(base, win, out);
        })
    }

    /// Compensated rolling sum shared by all rolling_sum/rolling_mean inputs.
    ///
    /// Nonfinite membership is counted separately so expired NaNs/infinities
    /// cannot poison later windows. Rebuilding once per window bounds accumulated
    /// update error while retaining O(n) work for ordinary finite arithmetic.
    fn rolling_sum_incremental(base: &[f64], win: usize, out: &mut [f64]) {
        let len = base.len();
        if win == 0 {
            out.fill(f64::NAN);
            return;
        }
        let mut sum = NeumaierAccumulator::new();
        let mut nan_count = 0_usize;
        let mut positive_infinities = 0_usize;
        let mut negative_infinities = 0_usize;
        for i in 0..len {
            if i >= win {
                let expired = base[i - win];
                if expired.is_nan() {
                    nan_count -= 1;
                } else if expired == f64::INFINITY {
                    positive_infinities -= 1;
                } else if expired == f64::NEG_INFINITY {
                    negative_infinities -= 1;
                } else {
                    sum.add(-expired);
                }
            }
            let value = base[i];
            if value.is_nan() {
                nan_count += 1;
            } else if value == f64::INFINITY {
                positive_infinities += 1;
            } else if value == f64::NEG_INFINITY {
                negative_infinities += 1;
            } else {
                sum.add(value);
            }

            // An exceptional overflow may require more frequent rebuilding;
            // it must not leave the accumulator corrupted after it expires.
            if (i >= win && i % win == 0) || !sum.total().is_finite() {
                sum = NeumaierAccumulator::new();
                for &value in &base[(i + 1).saturating_sub(win)..=i] {
                    if value.is_finite() {
                        sum.add(value);
                    }
                }
            }
            out[i] = if i + 1 < win
                || nan_count > 0
                || (positive_infinities > 0 && negative_infinities > 0)
            {
                f64::NAN
            } else if positive_infinities > 0 {
                f64::INFINITY
            } else if negative_infinities > 0 {
                f64::NEG_INFINITY
            } else {
                sum.total()
            };
        }
    }

    /// Resolve the EWM smoothing factor `alpha` to the valid pandas domain `(0, 1]`.
    ///
    /// Shared by `ewm_mean` and `ewm_std`/`ewm_var` so a given `alpha` means
    /// the same thing in each. Non-finite or non-positive inputs are floored to
    /// a tiny positive smoothing factor; values above 1 are capped.
    #[inline]
    fn resolve_ewm_alpha(raw: f64) -> f64 {
        const EWM_ALPHA_FLOOR: f64 = 1e-12;
        if !raw.is_finite() {
            return EWM_ALPHA_FLOOR;
        }
        raw.clamp(EWM_ALPHA_FLOOR, 1.0)
    }

    pub(super) fn eval_ewm_mean_into(arg_results: &[&[f64]], out: &mut [f64]) {
        if out.is_empty() {
            return;
        }
        if arg_results.len() < 2 || arg_results[1].is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let alpha = Self::resolve_ewm_alpha(arg_results[1][0]);
        let adjust = if arg_results.len() >= 3 && !arg_results[2].is_empty() {
            arg_results[2][0] != 0.0
        } else {
            true
        };
        // Seed the recursion from the first non-NaN observation; leading NaNs
        // emit NaN and must not poison the state. Interior NaNs after the seed
        // match `eval_ewm_variance_into`: the state is unchanged and NaN is
        // emitted for that position.
        let mut prev: f64 = 0.0;
        let mut weighted_sum: f64 = 0.0;
        let mut wsum: f64 = 0.0;
        let mut seeded = false;
        let filled = base.len().min(out.len());
        for (slot, &x) in out.iter_mut().zip(base) {
            if x.is_nan() {
                *slot = f64::NAN;
                continue;
            }
            if !seeded {
                prev = x;
                weighted_sum = x;
                wsum = 1.0;
                seeded = true;
                *slot = x;
                continue;
            }
            if adjust {
                weighted_sum = x + (1.0 - alpha) * weighted_sum;
                wsum = 1.0 + (1.0 - alpha) * wsum;
                *slot = weighted_sum / wsum;
            } else {
                prev = alpha * x + (1.0 - alpha) * prev;
                *slot = prev;
            }
        }
        // A short input leaves no observation for the tail.
        out[filled..].fill(f64::NAN);
    }

    /// Population variance, or standard deviation when `take_sqrt` is set.
    ///
    /// NaNs are excluded from both the sample and the count `n` (matching
    /// `median`/`quantile`). Fewer than two valid observations broadcasts NaN
    /// across `out`. The result is a scalar broadcast, including when `out`
    /// is longer than the input.
    ///
    /// Two-pass (mean, then squared deviations) is numerically stable; the
    /// naive `E[x^2] - E[x]^2` form suffers catastrophic cancellation when
    /// the mean is large relative to the spread.
    pub(super) fn eval_population_var_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
        take_sqrt: bool,
    ) {
        let Some(data) = arg_results.first() else {
            out.fill(f64::NAN);
            return;
        };
        let mut n = 0_usize;
        let mut sum = 0.0_f64;
        for &x in data.iter() {
            if !x.is_nan() {
                n += 1;
                sum += x;
            }
        }
        let value = if n > 1 {
            let mean = sum / n as f64;
            let variance = data
                .iter()
                .filter(|x| !x.is_nan())
                .map(|&x| {
                    let dx = x - mean;
                    dx * dx
                })
                .sum::<f64>()
                / n as f64;
            if take_sqrt {
                variance.sqrt()
            } else {
                variance
            }
        } else {
            f64::NAN
        };
        out.fill(value);
    }

    /// Median over all non-NaN observations.
    ///
    /// NaN policy: NaNs are excluded from both the sample and the count `n`
    /// (the same skip-NaN policy as `quantile`); an all-NaN or empty input
    /// broadcasts NaN, and `median([1, 2, 3, NaN])` is `2`.
    pub(super) fn eval_median_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let Some(data) = arg_results.first() else {
            out.fill(f64::NAN);
            return;
        };
        let mut guard = self
            .scratch
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = &mut guard.tmp;
        tmp.clear();
        tmp.extend(data.iter().copied().filter(|v| !v.is_nan()));
        let median = Self::median_of_unsorted(tmp);
        out.fill(median);
    }

    /// Sort the buffer in place and return its median, or NaN when empty.
    #[inline]
    fn median_of_unsorted(buf: &mut [f64]) -> f64 {
        let n = buf.len();
        if n == 0 {
            return f64::NAN;
        }
        buf.sort_unstable_by(|a, b| a.total_cmp(b));
        if n % 2 == 1 {
            buf[n / 2]
        } else {
            (buf[n / 2 - 1] + buf[n / 2]) * 0.5
        }
    }

    pub(super) fn eval_rolling_std_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        Self::rolling_with(arg_results, out, |base, win, out| {
            Self::rolling_apply_into(base, win, out, &mut |window| {
                Self::window_population_var(window).sqrt()
            });
        })
    }

    pub(super) fn eval_rolling_var_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        Self::rolling_with(arg_results, out, |base, win, out| {
            Self::rolling_apply_into(base, win, out, &mut |window| {
                Self::window_population_var(window)
            });
        })
    }

    /// Two-pass population variance of one window, shared by `rolling_std`
    /// and `rolling_var`.
    ///
    /// Two passes avoid the catastrophic cancellation of the running
    /// `E[x^2] - E[x]^2` form when the mean is large relative to the spread
    /// (e.g. prices near 1e8 with tiny variance). A NaN inside the window
    /// propagates (window result is NaN).
    #[inline]
    fn window_population_var(w: &[f64]) -> f64 {
        let n = w.len() as f64;
        let mean = w.iter().copied().sum::<f64>() / n;
        w.iter()
            .map(|v| {
                let dv = *v - mean;
                dv * dv
            })
            .sum::<f64>()
            / n
    }

    /// Rolling median over a fixed row window.
    ///
    /// NaN policy: NaNs inside a window are excluded from both the sample and
    /// the count (matching the global `median`/`quantile` skip-NaN policy);
    /// an all-NaN window yields NaN.
    pub(super) fn eval_rolling_median_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        Self::rolling_with(arg_results, out, |base, win, out| {
            let mut guard = self
                .scratch
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let wbuf = &mut guard.window;
            Self::rolling_apply_into(base, win, out, &mut |window| {
                wbuf.clear();
                wbuf.extend(window.iter().copied().filter(|v| !v.is_nan()));
                Self::median_of_unsorted(wbuf)
            });
        })
    }

    pub(super) fn eval_shift_into(arg_results: &[&[f64]], out: &mut [f64]) -> crate::Result<()> {
        if arg_results.len() >= 2 && !arg_results[1].is_empty() {
            let base = arg_results[0];
            let raw = arg_results[1][0];
            #[allow(clippy::as_conversions)]
            let n = if raw.is_finite() && raw >= f64::from(i32::MIN) && raw <= f64::from(i32::MAX) {
                raw as i32
            } else {
                out.fill(f64::NAN);
                return Ok(());
            };
            for (i, slot) in out.iter_mut().enumerate() {
                let shifted_idx = if n >= 0 {
                    i.checked_sub(n as usize)
                } else {
                    i.checked_add(n.unsigned_abs() as usize)
                };
                *slot = shifted_idx
                    .and_then(|index| base.get(index))
                    .copied()
                    .unwrap_or(f64::NAN);
            }
            return Ok(());
        }
        out.fill(f64::NAN);
        Ok(())
    }

    /// Map `f` across the first argument.
    ///
    /// A missing input fills `out` with NaN. A result shorter than `out`
    /// NaN-fills the tail.
    pub(super) fn eval_unary_into(arg_results: &[&[f64]], out: &mut [f64], f: impl Fn(f64) -> f64) {
        let Some(base) = arg_results.first() else {
            out.fill(f64::NAN);
            return;
        };
        let n = out.len().min(base.len());
        for i in 0..n {
            out[i] = f(base[i]);
        }
        out[n..].fill(f64::NAN);
    }

    /// Expression `sign`: positive → `1`, negative → `-1`, both zeros → `0`.
    ///
    /// `f64::signum` preserves the sign of zero, so it is not this function.
    fn sign_or_zero(value: f64) -> f64 {
        if value.is_nan() {
            f64::NAN
        } else if value > 0.0 {
            1.0
        } else if value < 0.0 {
            -1.0
        } else {
            0.0
        }
    }

    pub(super) fn eval_sign_into(arg_results: &[&[f64]], out: &mut [f64]) {
        Self::eval_unary_into(arg_results, out, Self::sign_or_zero);
    }

    pub(super) fn eval_pow_into(arg_results: &[&[f64]], out: &mut [f64]) {
        let (Some(base), Some(exponent)) = (arg_results.first(), arg_results.get(1)) else {
            out.fill(f64::NAN);
            return;
        };
        let n = out.len().min(base.len());
        for i in 0..n {
            out[i] = match exponent.get(i) {
                Some(&power) => base[i].powf(power),
                None => f64::NAN,
            };
        }
        out[n..].fill(f64::NAN);
    }

    /// Evaluate `round(x[, digits])` with ties rounding half away from zero.
    ///
    /// `digits` (optional, default 0) must be a constant integral scalar;
    /// a non-constant series is a validation error (matching the
    /// window/step-argument convention) and a non-integral or out-of-range
    /// constant yields all-NaN output.
    pub(super) fn eval_round_into(arg_results: &[&[f64]], out: &mut [f64]) -> crate::Result<()> {
        let digits = match arg_results.get(1) {
            Some(series) if !series.is_empty() => {
                let first = Self::constant_scalar(series, "round() digits argument")?;
                if !first.is_finite()
                    || first.fract() != 0.0
                    || first < i32::MIN as f64
                    || first > i32::MAX as f64
                {
                    None
                } else {
                    Some(first as i32)
                }
            }
            _ => Some(0),
        };
        let Some(digits) = digits else {
            Self::eval_unary_into(arg_results, out, |_| f64::NAN);
            return Ok(());
        };
        Self::eval_unary_into(arg_results, out, |value| {
            crate::math::round_half_away(value, digits)
        });
        Ok(())
    }

    pub(super) fn eval_clamp_into(arg_results: &[&[f64]], out: &mut [f64]) {
        let (Some(base), Some(lo), Some(hi)) =
            (arg_results.first(), arg_results.get(1), arg_results.get(2))
        else {
            out.fill(f64::NAN);
            return;
        };
        let n = out.len().min(base.len());
        for i in 0..n {
            out[i] = match (lo.get(i), hi.get(i)) {
                (Some(&lower), Some(&upper)) => crate::math::clamp_or_nan(base[i], lower, upper),
                _ => f64::NAN,
            };
        }
        out[n..].fill(f64::NAN);
    }

    pub(super) fn eval_rank_into(arg_results: &[&[f64]], out: &mut [f64]) {
        let Some(base) = arg_results.first() else {
            out.fill(f64::NAN);
            return;
        };
        let mut indexed: Vec<(f64, usize)> =
            base.iter().enumerate().map(|(i, &v)| (v, i)).collect();
        indexed.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut current_rank: f64 = 1.0;
        let mut next_rank: f64 = 1.0;
        let mut last_value: Option<f64> = None;
        for (value, orig_idx) in indexed {
            let ranked = if value.is_nan() {
                f64::NAN
            } else {
                // Exact comparison is intentional: values come from
                // sort_unstable_by(total_cmp) so bit-identical values are adjacent.
                #[allow(clippy::float_cmp)]
                if last_value.is_none_or(|last| value != last) {
                    current_rank = next_rank;
                }
                last_value = Some(value);
                next_rank += 1.0;
                current_rank
            };
            if orig_idx < out.len() {
                out[orig_idx] = ranked;
            }
        }
        let n = out.len().min(base.len());
        out[n..].fill(f64::NAN);
    }

    pub(super) fn eval_quantile_into(arg_results: &[&[f64]], out: &mut [f64]) {
        let Some(base) = arg_results.first() else {
            out.fill(f64::NAN);
            return;
        };
        if arg_results.len() < 2 || arg_results[1].is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let q = arg_results[1][0].clamp(0.0, 1.0);
        let valid_values: Vec<f64> = base.iter().copied().filter(|x| !x.is_nan()).collect();
        let quantile_value = if valid_values.is_empty() {
            f64::NAN
        } else {
            quantile_linear_or_nan(&valid_values, q)
        };
        let n = out.len().min(base.len());
        out[..n].fill(quantile_value);
        out[n..].fill(f64::NAN);
    }

    pub(super) fn eval_rolling_min_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        Self::rolling_with(arg_results, out, |base, win, out| {
            Self::rolling_apply_into(base, win, out, &mut finite_min_or_nan);
        })
    }

    pub(super) fn eval_rolling_max_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        Self::rolling_with(arg_results, out, |base, win, out| {
            Self::rolling_apply_into(base, win, out, &mut finite_max_or_nan);
        })
    }

    pub(super) fn eval_rolling_count_into(
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        Self::rolling_with(arg_results, out, |base, win, out| {
            Self::rolling_apply_into(base, win, out, &mut |window| finite_count(window) as f64);
        })
    }

    /// Exponentially weighted population variance, or standard deviation when
    /// `take_sqrt` is set.
    ///
    /// A missing smoothing factor fills `out` with NaN. Variance of a single
    /// observation is `0.0`. `adjust` is true only for a strictly positive flag.
    pub(super) fn eval_ewm_variance_into(arg_results: &[&[f64]], out: &mut [f64], take_sqrt: bool) {
        if arg_results.len() < 2 || arg_results[1].is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        if base.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let alpha = Self::resolve_ewm_alpha(arg_results[1][0]);
        let adjust = arg_results
            .get(2)
            .and_then(|v| v.first())
            .map(|&x| x > 0.0)
            .unwrap_or(true);

        // Keep the mean as an offset from the latest observation. Both
        // the weighted variance recurrence and its mean differences then
        // avoid subtracting large raw moments or repeatedly rounding a
        // large absolute mean. NaNs emit NaN without advancing the state.
        let mut origin = 0.0_f64;
        let mut mean_offset = 0.0_f64;
        let mut variance = 0.0_f64;
        let mut weight_sum = 0.0_f64;
        let mut seeded = false;

        let n = out.len().min(base.len());
        for (i, &value) in base.iter().enumerate().take(n) {
            if value.is_nan() {
                out[i] = f64::NAN;
                continue;
            }
            if !seeded {
                origin = value;
                weight_sum = 1.0;
                seeded = true;
                // Existing convention: variance of a single observation is 0.0.
                out[i] = 0.0;
                continue;
            }
            let weight = if adjust {
                weight_sum = 1.0 + (1.0 - alpha) * weight_sum;
                1.0 / weight_sum
            } else {
                alpha
            };
            if weight >= 1.0 {
                variance = 0.0;
                mean_offset = 0.0;
            } else {
                let delta = (value - origin) - mean_offset;
                variance = (1.0 - weight) * (variance + weight * delta * delta);
                mean_offset = -(1.0 - weight) * delta;
            }
            origin = value;
            out[i] = if take_sqrt { variance.sqrt() } else { variance };
        }
        out[n..].fill(f64::NAN);
    }
}
