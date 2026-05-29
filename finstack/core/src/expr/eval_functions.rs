//! Scalar function implementations for `CompiledExpr`.
//!
//! Contains per-function evaluation logic (lag, lead, diff, rolling_*, ewm_*,
//! cumulative aggregations, rank, quantile, etc.) separated from the core DAG
//! execution engine in `eval.rs`.

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

    #[inline]
    pub(super) fn window_arg(arg_results: &[&[f64]], default: Option<usize>) -> Result<usize, ()> {
        if let Some(raw) = arg_results.get(1).and_then(|v| v.first()).copied() {
            Self::validate_window(raw).ok_or(())
        } else {
            default.ok_or(())
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

    pub(super) fn eval_lag(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_lag_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_lag_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let n = match Self::window_arg(arg_results, None) {
            Ok(n) => n,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        let base = arg_results[0];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = if i < n {
                f64::NAN
            } else {
                base.get(i - n).copied().unwrap_or(f64::NAN)
            };
        }
    }

    pub(super) fn eval_lead(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_lead_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_lead_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let n = match Self::window_arg(arg_results, None) {
            Ok(n) => n,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        let base = arg_results[0];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = base.get(i + n).copied().unwrap_or(f64::NAN);
        }
    }

    pub(super) fn eval_diff(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_diff_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_diff_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let n = match Self::window_arg(arg_results, Some(1)) {
            Ok(n) => n,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = match (base.get(i), i.checked_sub(n).and_then(|j| base.get(j))) {
                (Some(&cur), Some(&prev)) => cur - prev,
                _ => f64::NAN,
            };
        }
    }

    pub(super) fn eval_pct_change(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_pct_change_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_pct_change_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let n = match Self::window_arg(arg_results, Some(1)) {
            Ok(n) => n,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = match (base.get(i), i.checked_sub(n).and_then(|j| base.get(j))) {
                (Some(&0.0), Some(&0.0)) => 0.0,
                (Some(&cur), Some(&0.0)) => cur.signum() * f64::INFINITY,
                (Some(&cur), Some(&prev)) => (cur / prev) - 1.0,
                _ => f64::NAN,
            };
        }
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

    pub(super) fn eval_rolling_mean(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_mean_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_rolling_mean_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let win = match Self::window_arg(arg_results, None) {
            Ok(win) => win,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
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
    }

    pub(super) fn eval_rolling_sum(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_sum_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_rolling_sum_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let win = match Self::window_arg(arg_results, None) {
            Ok(win) => win,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        if base.iter().any(|v| v.is_nan()) {
            Self::rolling_apply_into(base, win, out, &mut |w| w.iter().copied().sum());
        } else {
            Self::rolling_sum_incremental(base, win, out);
        }
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
        let mut prev: f64 = 0.0;
        let mut weighted_sum: f64 = 0.0;
        let mut wsum: f64 = 0.0;
        for (i, &x) in base.iter().enumerate() {
            if i == 0 {
                prev = x;
                weighted_sum = x;
                wsum = 1.0;
                out[0] = x;
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

    fn eval_std_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        let data = &arg_results[0];
        if data.len() > 1 {
            let mean = data.iter().copied().sum::<f64>() / data.len() as f64;
            let variance = data
                .iter()
                .map(|&x| {
                    let dx = x - mean;
                    dx * dx
                })
                .sum::<f64>()
                / data.len() as f64;
            let std = variance.sqrt();
            for v in out.iter_mut().take(len) {
                *v = std;
            }
        } else {
            for v in out.iter_mut().take(len) {
                *v = f64::NAN;
            }
        }
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

    fn eval_var_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        let data = &arg_results[0];
        if data.len() > 1 {
            let mean = data.iter().copied().sum::<f64>() / data.len() as f64;
            let variance = data
                .iter()
                .map(|&x| {
                    let dx = x - mean;
                    dx * dx
                })
                .sum::<f64>()
                / data.len() as f64;
            for v in out.iter_mut().take(len) {
                *v = variance;
            }
        } else {
            for v in out.iter_mut().take(len) {
                *v = f64::NAN;
            }
        }
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

    fn eval_median_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        let data = &arg_results[0];
        if !data.is_empty() {
            let mut guard = self
                .scratch
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let tmp = &mut guard.tmp;
            tmp.truncate(0);
            tmp.extend_from_slice(data);
            tmp.sort_unstable_by(|a, b| a.total_cmp(b));
            let n = tmp.len();
            let median = if n % 2 == 1 {
                tmp[n / 2]
            } else {
                (tmp[n / 2 - 1] + tmp[n / 2]) * (0.5)
            };
            for v in out.iter_mut().take(len) {
                *v = median;
            }
        } else {
            for v in out.iter_mut().take(len) {
                *v = f64::NAN;
            }
        }
    }

    pub(super) fn eval_rolling_std(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_std_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_rolling_std_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let win = match Self::window_arg(arg_results, None) {
            Ok(win) => win,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        if base.iter().any(|v| v.is_nan()) {
            Self::rolling_apply_into(base, win, out, &mut |w| {
                let m = w.iter().copied().sum::<f64>() / (w.len() as f64);
                let var = w
                    .iter()
                    .map(|v| {
                        let dv = *v - m;
                        dv * dv
                    })
                    .sum::<f64>()
                    / (w.len() as f64);
                var.sqrt()
            });
        } else {
            Self::rolling_var_incremental(base, win, out);
            for v in out.iter_mut() {
                if !v.is_nan() {
                    *v = v.sqrt();
                }
            }
        }
    }

    pub(super) fn eval_rolling_var(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_var_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_rolling_var_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let win = match Self::window_arg(arg_results, None) {
            Ok(win) => win,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        if base.iter().any(|v| v.is_nan()) {
            Self::rolling_apply_into(base, win, out, &mut |w| {
                let m = w.iter().copied().sum::<f64>() / (w.len() as f64);
                w.iter()
                    .map(|v| {
                        let dv = *v - m;
                        dv * dv
                    })
                    .sum::<f64>()
                    / (w.len() as f64)
            });
        } else {
            Self::rolling_var_incremental(base, win, out);
        }
    }

    /// O(n) incremental rolling population variance via running sum and sum-of-squares.
    /// Requires NaN-free input; caller must check.
    fn rolling_var_incremental(base: &[f64], win: usize, out: &mut [f64]) {
        let len = base.len();
        if win == 0 {
            out.fill(f64::NAN);
            return;
        }
        let w = win as f64;
        let (mut s, mut s2) = (0.0_f64, 0.0_f64);
        for i in 0..len {
            s += base[i];
            s2 += base[i] * base[i];
            if i >= win {
                s -= base[i - win];
                s2 -= base[i - win] * base[i - win];
            }
            if i + 1 >= win {
                let mean = s / w;
                out[i] = (s2 / w - mean * mean).max(0.0);
            } else {
                out[i] = f64::NAN;
            }
        }
    }

    pub(super) fn eval_rolling_median(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_median_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_rolling_median_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let win = match Self::window_arg(arg_results, None) {
            Ok(win) => win,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        let mut guard = self
            .scratch
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let wbuf = &mut guard.window;
        for i in 0..len {
            if i + 1 < win {
                out[i] = f64::NAN;
            } else {
                let start = i + 1 - win;
                let slice = &base[start..=i];
                wbuf.truncate(0);
                wbuf.extend_from_slice(slice);
                wbuf.sort_unstable_by(|a, b| a.total_cmp(b));
                let k = wbuf.len();
                out[i] = if k % 2 == 1 {
                    wbuf[k / 2]
                } else {
                    (wbuf[k / 2 - 1] + wbuf[k / 2]) * (0.5)
                };
            }
        }
    }

    pub(super) fn eval_shift(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_shift_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_shift_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        if arg_results.len() >= 2 && !arg_results[1].is_empty() {
            let base = arg_results[0];
            let raw = arg_results[1][0];
            #[allow(clippy::as_conversions)]
            let n = if raw.is_finite() && raw >= f64::from(i32::MIN) && raw <= f64::from(i32::MAX) {
                raw as i32
            } else {
                out.fill(f64::NAN);
                return;
            };
            for (i, slot) in out.iter_mut().enumerate() {
                let shifted_idx = i as i32 - n;
                *slot = if shifted_idx >= 0 && shifted_idx < base.len() as i32 {
                    base[shifted_idx as usize]
                } else {
                    f64::NAN
                };
            }
            return;
        }
        out.fill(f64::NAN);
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

    pub(super) fn eval_rolling_min(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_min_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_rolling_min_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let win = match Self::window_arg(arg_results, None) {
            Ok(win) => win,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        Self::rolling_apply_into(base, win, out, &mut finite_min_or_nan);
    }

    pub(super) fn eval_rolling_max(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_max_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_rolling_max_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let win = match Self::window_arg(arg_results, None) {
            Ok(win) => win,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        Self::rolling_apply_into(base, win, out, &mut finite_max_or_nan);
    }

    pub(super) fn eval_rolling_count(&self, arg_results: &[&[f64]]) -> Vec<f64> {
        let len = arg_results.first().map(|a| a.len()).unwrap_or(0);
        let mut out = vec![0.0; len];
        self.eval_rolling_count_into(arg_results, &mut out);
        out
    }

    pub(super) fn eval_rolling_count_into(&self, arg_results: &[&[f64]], out: &mut [f64]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        if arg_results.is_empty() {
            out.fill(f64::NAN);
            return;
        }
        let base = arg_results[0];
        let win = match Self::window_arg(arg_results, None) {
            Ok(win) => win,
            Err(_) => {
                out.fill(f64::NAN);
                return;
            }
        };
        Self::rolling_apply_into(base, win, out, &mut |w| finite_count(w) as f64);
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

            let mut ema = base[0];
            let mut ema_sq = base[0] * base[0];
            let mut n: f64 = 1.0;

            out.push(0.0);

            for &value in base.iter().skip(1) {
                if !value.is_nan() {
                    n += 1.0;
                    let weight = if adjust {
                        alpha / (1.0 - (1.0 - alpha).powf(n))
                    } else {
                        alpha
                    };
                    ema = ((1.0 - weight) * ema) + (weight * value);
                    ema_sq = ((1.0 - weight) * ema_sq) + (weight * value * value);
                    let variance = (ema_sq - ema * ema).max(0.0);
                    out.push(if take_sqrt { variance.sqrt() } else { variance });
                } else {
                    out.push(f64::NAN);
                }
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
            Function::Lag => Ok(self.eval_lag(arg_results)),
            Function::Lead => Ok(self.eval_lead(arg_results)),
            Function::Diff => Ok(self.eval_diff(arg_results)),
            Function::PctChange => Ok(self.eval_pct_change(arg_results)),
            Function::CumSum => Ok(self.eval_cum_sum(arg_results)),
            Function::CumProd => Ok(self.eval_cum_prod(arg_results)),
            Function::CumMin => Ok(self.eval_cum_min(arg_results)),
            Function::CumMax => Ok(self.eval_cum_max(arg_results)),
            Function::RollingMean => Ok(self.eval_rolling_mean(arg_results)),
            Function::RollingSum => Ok(self.eval_rolling_sum(arg_results)),
            Function::EwmMean => Ok(self.eval_ewm_mean(arg_results)),
            Function::Std => Ok(self.eval_std(arg_results)),
            Function::Var => Ok(self.eval_var(arg_results)),
            Function::Median => Ok(self.eval_median(arg_results)),
            Function::RollingStd => Ok(self.eval_rolling_std(arg_results)),
            Function::RollingVar => Ok(self.eval_rolling_var(arg_results)),
            Function::RollingMedian => Ok(self.eval_rolling_median(arg_results)),
            Function::Shift => Ok(self.eval_shift(arg_results)),
            Function::Rank => Ok(self.eval_rank(arg_results)),
            Function::Quantile => Ok(self.eval_quantile(arg_results)),
            Function::RollingMin => Ok(self.eval_rolling_min(arg_results)),
            Function::RollingMax => Ok(self.eval_rolling_max(arg_results)),
            Function::RollingCount => Ok(self.eval_rolling_count(arg_results)),
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
