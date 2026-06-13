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

use super::ast::Function;
use super::context::SimpleContext;
use super::eval::CompiledExpr;
use crate::math::{finite_count, finite_max_or_nan, finite_min_or_nan, quantile_linear_or_nan};

impl CompiledExpr {
    // --- Scalar evaluator helpers ---

    #[inline]
    pub(super) fn validate_window(raw: f64) -> Option<usize> {
        if !raw.is_finite() {
            return None;
        }
        if raw < 1.0 {
            return None;
        }
        if raw.fract() != 0.0 {
            return None;
        }
        if raw > usize::MAX as f64 {
            return None;
        }
        Some(raw as usize)
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
    pub(super) fn window_arg(
        arg_results: &[&[f64]],
        default: Option<usize>,
    ) -> crate::Result<Option<usize>> {
        match arg_results.get(1) {
            Some(series) if !series.is_empty() => {
                let first = series[0];
                // Bit-level comparison is NaN-safe and deterministic.
                if series.iter().any(|v| v.to_bits() != first.to_bits()) {
                    return Err(crate::Error::Validation(
                        "expression window/step argument must be a constant scalar (literal); got a non-constant series".to_string(),
                    ));
                }
                Ok(Self::validate_window(first))
            }
            _ => Ok(default),
        }
    }

    #[inline]
    pub(super) fn rolling_apply_into(
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

    // --- Per-function evaluators ---

    pub(super) fn eval_lag(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_lag_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_lag_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
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

    pub(super) fn eval_lead(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_lead_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_lead_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
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
            *slot = base.get(i + n).copied().unwrap_or(f64::NAN);
        }
        Ok(())
    }

    pub(super) fn eval_diff(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_diff_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_diff_into(
        &self,
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
                (Some(&cur), Some(&prev)) => cur - prev,
                _ => f64::NAN,
            };
        }
        Ok(())
    }

    pub(super) fn eval_pct_change(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_pct_change_into(arg_results, &mut out)?;
        Ok(out)
    }

    /// Percentage change over step `n`.
    ///
    /// Division-by-zero convention (intentionally different from the binary
    /// `/` operator, which returns NaN on any zero divisor): a zero base with
    /// a non-zero current value returns `±inf` (sign of the current value),
    /// and `0/0` returns `0.0` (interpreted as "no change").
    pub(super) fn eval_pct_change_into(
        &self,
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

    pub(super) fn eval_cum_sum(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = Vec::with_capacity(len);
        if !arg_results.is_empty() {
            let base = arg_results[0];
            let mut acc = 0.0;
            for &v in base {
                if !v.is_nan() {
                    acc += v;
                }
                out.push(acc);
            }
        }
        out
    }

    pub(super) fn eval_cum_prod(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = Vec::with_capacity(len);
        if !arg_results.is_empty() {
            let base = arg_results[0];
            let mut acc = 1.0;
            for &v in base {
                if !v.is_nan() {
                    acc *= v;
                }
                out.push(acc);
            }
        }
        out
    }

    pub(super) fn eval_cum_min(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = Vec::with_capacity(len);
        if !arg_results.is_empty() {
            let base = arg_results[0];
            let mut cur = f64::INFINITY;
            for &v in base {
                if !v.is_nan() {
                    cur = if cur < v { cur } else { v };
                }
                out.push(cur);
            }
        }
        out
    }

    pub(super) fn eval_cum_max(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = Vec::with_capacity(len);
        if !arg_results.is_empty() {
            let base = arg_results[0];
            let mut cur = f64::NEG_INFINITY;
            for &v in base {
                if !v.is_nan() {
                    cur = if cur > v { cur } else { v };
                }
                out.push(cur);
            }
        }
        out
    }

    pub(super) fn eval_rolling_mean(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_mean_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_rolling_mean_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        let len = out.len();
        if len == 0 {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        if base.iter().any(|v| v.is_nan()) {
            Self::rolling_apply_into(base, win, out, &mut |w| {
                w.iter().copied().sum::<f64>() / w.len() as f64
            });
        } else {
            Self::rolling_sum_incremental(base, win, out);
            let w = win as f64;
            for v in out.iter_mut() {
                if !v.is_nan() {
                    *v /= w;
                }
            }
        }
        Ok(())
    }

    pub(super) fn eval_rolling_sum(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_sum_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_rolling_sum_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        let len = out.len();
        if len == 0 {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        if base.iter().any(|v| v.is_nan()) {
            Self::rolling_apply_into(base, win, out, &mut |w| w.iter().copied().sum());
        } else {
            Self::rolling_sum_incremental(base, win, out);
        }
        Ok(())
    }

    /// O(n) incremental rolling sum (shared by rolling_sum and rolling_mean).
    /// Requires NaN-free input; caller must check.
    fn rolling_sum_incremental(base: &[f64], win: usize, out: &mut [f64]) {
        let len = base.len();
        if win == 0 {
            out.fill(f64::NAN);
            return;
        }
        let mut sum = 0.0_f64;
        for i in 0..len {
            sum += base[i];
            if i >= win {
                sum -= base[i - win];
            }
            if i + 1 >= win {
                out[i] = sum;
            } else {
                out[i] = f64::NAN;
            }
        }
    }

    /// Resolve the EWM smoothing factor `alpha` to the valid pandas domain `(0, 1]`.
    ///
    /// `ewm_mean` and `ewm_std`/`ewm_var` previously disagreed on how `alpha` was
    /// interpreted (the mean used the raw value while the variance clamped to
    /// `[0.001, 0.999]`, which silently rescaled slow EWMs and wrongly excluded
    /// the valid `alpha = 1.0`). Both now funnel through this helper so a given
    /// `alpha` always means the same thing. Non-finite or non-positive inputs are
    /// floored to a tiny epsilon (which keeps the `adjust = true` bias weight
    /// `alpha / (1 - (1 - alpha)^n)` away from `0/0`); values above 1 are capped.
    #[inline]
    fn resolve_ewm_alpha(raw: f64) -> f64 {
        const EWM_ALPHA_FLOOR: f64 = 1e-12;
        if !raw.is_finite() {
            return EWM_ALPHA_FLOOR;
        }
        raw.clamp(EWM_ALPHA_FLOOR, 1.0)
    }

    pub(super) fn eval_ewm_mean(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        if arg_results.len() >= 2 && !arg_results[1].is_empty() {
            let mut out = vec![0.0; len];
            self.eval_ewm_mean_into(arg_results, &mut out);
            return out;
        }
        Vec::with_capacity(len)
    }

    fn eval_ewm_mean_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        let base = &arg_results[0];
        let alpha = Self::resolve_ewm_alpha(arg_results[1][0]);
        let adjust = if arg_results.len() >= 3 && !arg_results[2].is_empty() {
            arg_results[2][0] != 0.0
        } else {
            true
        };
        // Seed the recursion from the first non-NaN observation; leading NaNs
        // emit NaN and must not poison the state (previously a leading NaN
        // seeded `prev`/`weighted_sum` and every subsequent output was NaN —
        // see ). Interior NaNs
        // after the seed match `eval_ewm_variance_core`: the state is
        // unchanged and NaN is emitted for that position.
        let mut prev: f64 = 0.0;
        let mut weighted_sum: f64 = 0.0;
        let mut wsum: f64 = 0.0;
        let mut seeded = false;
        for (i, &x) in base.iter().enumerate() {
            if x.is_nan() {
                out[i] = f64::NAN;
                continue;
            }
            if !seeded {
                prev = x;
                weighted_sum = x;
                wsum = 1.0;
                seeded = true;
                out[i] = x;
                continue;
            }
            if adjust {
                weighted_sum = x + (1.0 - alpha) * weighted_sum;
                wsum = 1.0 + (1.0 - alpha) * wsum;
                out[i] = weighted_sum / wsum;
            } else {
                prev = alpha * x + (1.0 - alpha) * prev;
                out[i] = prev;
            }
        }
    }

    pub(super) fn eval_std(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        if !arg_results.is_empty() {
            let mut out = vec![0.0; len];
            self.eval_std_into(arg_results, &mut out);
            return out;
        }
        Vec::with_capacity(len)
    }

    /// Population standard deviation over all non-NaN observations.
    ///
    /// NaN policy: NaNs are excluded from both the sample and the count `n`
    /// (matching `median`/`quantile` and the statements-layer reducers).
    /// Fewer than two valid observations broadcasts NaN.
    fn eval_std_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        Self::eval_population_var_into(arg_results, out, true);
    }

    pub(super) fn eval_var(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        if !arg_results.is_empty() {
            let mut out = vec![0.0; len];
            self.eval_var_into(arg_results, &mut out);
            return out;
        }
        Vec::with_capacity(len)
    }

    /// Population variance over all non-NaN observations (see [`Self::eval_std_into`]).
    fn eval_var_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        Self::eval_population_var_into(arg_results, out, false);
    }

    /// Shared two-pass population variance/std over non-NaN observations.
    ///
    /// Two-pass (mean, then squared deviations) is numerically stable; the
    /// naive `E[x^2] - E[x]^2` form suffers catastrophic cancellation when
    /// the mean is large relative to the spread.
    fn eval_population_var_into(arg_results: &[&[f64]], out: &mut [f64], take_sqrt: bool) {
        let data = &arg_results[0];
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

    pub(super) fn eval_median(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        if !arg_results.is_empty() {
            let mut out = vec![0.0; len];
            self.eval_median_into(arg_results, &mut out);
            return out;
        }
        Vec::with_capacity(len)
    }

    /// Median over all non-NaN observations.
    ///
    /// NaN policy: NaNs are excluded from both the sample and the count `n`
    /// (the same skip-NaN policy as `quantile`); an all-NaN or empty input
    /// broadcasts NaN. Previously NaNs sorted as the largest values and
    /// shifted the midpoint (e.g. `median([1,2,3,NaN])` returned `2.5`).
    fn eval_median_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let data = &arg_results[0];
        let mut guard = self
            .scratch
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = &mut guard.tmp;
        tmp.truncate(0);
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

    pub(super) fn eval_rolling_std(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_std_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_rolling_std_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        let len = out.len();
        if len == 0 {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        Self::rolling_apply_into(base, win, out, &mut |w| {
            Self::window_population_var(w).sqrt()
        });
        Ok(())
    }

    pub(super) fn eval_rolling_var(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_var_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_rolling_var_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        let len = out.len();
        if len == 0 {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        Self::rolling_apply_into(base, win, out, &mut |w| Self::window_population_var(w));
        Ok(())
    }

    /// Two-pass population variance of one window, shared by `rolling_std`
    /// and `rolling_var` for both NaN-containing and NaN-free inputs.
    ///
    /// The previous NaN-free fast path used the running `E[x^2] - E[x]^2`
    /// form, which suffers catastrophic cancellation when the mean is large
    /// relative to the spread (e.g. prices near 1e8 with tiny variance) and
    /// could disagree with the NaN path. A two-pass computation per window is
    /// numerically stable and keeps a single algorithm for both paths.
    /// A NaN inside the window propagates (window result is NaN).
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

    pub(super) fn eval_rolling_median(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_median_into(arg_results, &mut out)?;
        Ok(out)
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
        let len = out.len();
        if len == 0 {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        let mut guard = self
            .scratch
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let wbuf = &mut guard.window;
        Self::rolling_apply_into(base, win, out, &mut |w| {
            wbuf.truncate(0);
            wbuf.extend(w.iter().copied().filter(|v| !v.is_nan()));
            Self::median_of_unsorted(wbuf)
        });
        Ok(())
    }

    pub(super) fn eval_shift(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_shift_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_shift_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
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
                let shifted_idx = i as i32 - n;
                *slot = if shifted_idx >= 0 && shifted_idx < base.len() as i32 {
                    base[shifted_idx as usize]
                } else {
                    f64::NAN
                };
            }
            return Ok(());
        }
        out.fill(f64::NAN);
        Ok(())
    }

    pub(super) fn eval_abs(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        if let Some(base) = arg_results.first() {
            base.iter().map(|v| v.abs()).collect()
        } else {
            Vec::new()
        }
    }

    pub(super) fn eval_sign(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        if let Some(base) = arg_results.first() {
            base.iter()
                .map(|v| {
                    if v.is_nan() {
                        f64::NAN
                    } else if *v > 0.0 {
                        1.0
                    } else if *v < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    pub(super) fn eval_rank(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        if !arg_results.is_empty() {
            let base = &arg_results[0];
            let mut indexed: Vec<(f64, usize)> =
                base.iter().enumerate().map(|(i, &v)| (v, i)).collect();
            indexed.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            let mut out: Vec<f64> = vec![0.0; len];
            let mut current_rank: f64 = 1.0;
            let mut last_value: f64 = f64::NAN;
            for (value, orig_idx) in indexed {
                if !value.is_nan() {
                    // Exact comparison is intentional: values come from
                    // sort_unstable_by(total_cmp) so bit-identical values are adjacent.
                    #[allow(clippy::float_cmp)]
                    if value != last_value && !last_value.is_nan() {
                        current_rank += 1.0;
                    }
                    out[orig_idx] = current_rank;
                    last_value = value;
                } else {
                    out[orig_idx] = f64::NAN;
                }
            }
            return out;
        }
        Vec::with_capacity(len)
    }

    pub(super) fn eval_quantile(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        if arg_results.len() >= 2 && !arg_results[1].is_empty() {
            let base = &arg_results[0];
            let q = arg_results[1][0].clamp(0.0, 1.0);
            let valid_values: Vec<f64> = base
                .iter()
                .filter_map(|&x| if x.is_nan() { None } else { Some(x) })
                .collect();
            let mut out = vec![0.0; len];
            if !valid_values.is_empty() {
                let quantile_value = quantile_linear_or_nan(&valid_values, q);
                for v in out.iter_mut().take(len) {
                    *v = quantile_value;
                }
            } else {
                for v in out.iter_mut().take(len) {
                    *v = f64::NAN;
                }
            }
            return out;
        }
        Vec::with_capacity(len)
    }

    pub(super) fn eval_rolling_min(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_min_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_rolling_min_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        let len = out.len();
        if len == 0 {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        Self::rolling_apply_into(base, win, out, &mut finite_min_or_nan);
        Ok(())
    }

    pub(super) fn eval_rolling_max(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_max_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_rolling_max_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        let len = out.len();
        if len == 0 {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        Self::rolling_apply_into(base, win, out, &mut finite_max_or_nan);
        Ok(())
    }

    pub(super) fn eval_rolling_count(&self, arg_results: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_count_into(arg_results, &mut out)?;
        Ok(out)
    }

    pub(super) fn eval_rolling_count_into(
        &self,
        arg_results: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        let len = out.len();
        if len == 0 {
            return Ok(());
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return Ok(());
        }
        let base = arg_results[0];
        let Some(win) = Self::window_arg(arg_results, None)? else {
            out.fill(f64::NAN);
            return Ok(());
        };
        Self::rolling_apply_into(base, win, out, &mut |w| finite_count(w) as f64);
        Ok(())
    }

    pub(super) fn eval_ewm_std(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        Self::eval_ewm_variance_core(arg_results, true)
    }

    pub(super) fn eval_ewm_var(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        Self::eval_ewm_variance_core(arg_results, false)
    }

    fn eval_ewm_variance_core(arg_results: &[&[f64]], take_sqrt: bool) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out: Vec<f64> = Vec::with_capacity(len);
        if arg_results.len() >= 2 && !arg_results[1].is_empty() {
            let base = &arg_results[0];
            if base.is_empty() {
                return out;
            }
            let alpha = Self::resolve_ewm_alpha(arg_results[1][0]);
            let adjust = arg_results
                .get(2)
                .and_then(|v| v.first())
                .map(|&x| x > 0.0)
                .unwrap_or(true);

            // Seed the EMA state from the first non-NaN observation; leading
            // NaNs emit NaN and must not poison the recursion (previously a
            // leading NaN seeded `ema`/`ema_sq` and `.max(0.0)` silently
            // converted the resulting NaN variance to 0.0 — see
            // ). Interior NaNs
            // after the seed keep the existing skip-NaN semantics: the state
            // is unchanged and NaN is emitted for that position.
            let mut ema = 0.0_f64;
            let mut ema_sq = 0.0_f64;
            // Integer observation counter so the adjust weight uses `powi`,
            // which is a deterministic multiplication chain across platforms
            // (`powf` with an integral exponent may differ in the last ulp
            // between libm implementations).
            let mut n: i32 = 0;
            let mut seeded = false;

            for &value in base.iter() {
                if value.is_nan() {
                    out.push(f64::NAN);
                    continue;
                }
                if !seeded {
                    ema = value;
                    ema_sq = value * value;
                    n = 1;
                    seeded = true;
                    // Existing convention: variance of a single observation is 0.0.
                    out.push(0.0);
                    continue;
                }
                n = n.saturating_add(1);
                let weight = if adjust {
                    alpha / (1.0 - (1.0 - alpha).powi(n))
                } else {
                    alpha
                };
                ema = ((1.0 - weight) * ema) + (weight * value);
                ema_sq = ((1.0 - weight) * ema_sq) + (weight * value * value);
                // Clamp small negative values from floating-point cancellation
                // to 0.0, but let a NaN variance stay NaN instead of becoming 0.0.
                let raw = ema_sq - ema * ema;
                let variance = if raw.is_nan() { f64::NAN } else { raw.max(0.0) };
                out.push(if take_sqrt { variance.sqrt() } else { variance });
            }
        }
        out
    }

    // --- Function dispatch ---

    pub(super) fn eval_function_core(
        &self,
        fun: Function,
        arg_results: &[&[f64]],
        _ctx: &SimpleContext,
        _cols: &[&[f64]],
    ) -> crate::Result<Vec<f64>> {
        match fun {
            Function::Lag => self.eval_lag(arg_results),
            Function::Lead => self.eval_lead(arg_results),
            Function::Diff => self.eval_diff(arg_results),
            Function::PctChange => self.eval_pct_change(arg_results),
            Function::CumSum => Ok(self.eval_cum_sum(arg_results)),
            Function::CumProd => Ok(self.eval_cum_prod(arg_results)),
            Function::CumMin => Ok(self.eval_cum_min(arg_results)),
            Function::CumMax => Ok(self.eval_cum_max(arg_results)),
            Function::RollingMean => self.eval_rolling_mean(arg_results),
            Function::RollingSum => self.eval_rolling_sum(arg_results),
            Function::EwmMean => Ok(self.eval_ewm_mean(arg_results)),
            Function::Std => Ok(self.eval_std(arg_results)),
            Function::Var => Ok(self.eval_var(arg_results)),
            Function::Median => Ok(self.eval_median(arg_results)),
            Function::RollingStd => self.eval_rolling_std(arg_results),
            Function::RollingVar => self.eval_rolling_var(arg_results),
            Function::RollingMedian => self.eval_rolling_median(arg_results),
            Function::Shift => self.eval_shift(arg_results),
            Function::Rank => Ok(self.eval_rank(arg_results)),
            Function::Quantile => Ok(self.eval_quantile(arg_results)),
            Function::RollingMin => self.eval_rolling_min(arg_results),
            Function::RollingMax => self.eval_rolling_max(arg_results),
            Function::RollingCount => self.eval_rolling_count(arg_results),
            Function::EwmStd => Ok(self.eval_ewm_std(arg_results)),
            Function::EwmVar => Ok(self.eval_ewm_var(arg_results)),
            Function::Abs => Ok(self.eval_abs(arg_results)),
            Function::Sign => Ok(self.eval_sign(arg_results)),
            Function::Sum
            | Function::Mean
            | Function::Ttm
            | Function::Ytd
            | Function::Qtd
            | Function::FiscalYtd
            | Function::Annualize
            | Function::AnnualizeRate
            | Function::Coalesce
            | Function::GrowthRate => {
                debug_assert!(
                    !fun.is_scalar_evaluable(),
                    "Function::is_scalar_evaluable disagrees with eval dispatch for {fun:?}"
                );
                Err(crate::Error::Validation(format!(
                    "Expression function '{fun}' is a statements-layer function; evaluate it via the statements crate instead of core::expr::eval"
                )))
            }
        }
    }
}
