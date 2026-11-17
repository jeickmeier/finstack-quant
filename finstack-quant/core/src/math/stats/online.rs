//! Single-pass mergeable accumulators for moments and covariance.

use super::{MAX_EXACT_COUNT, frexp_exp, ldexp, normalize_zero, same_float};
use crate::math::special_functions::student_t_inv_cdf;

/// Internal shifted and power-of-two scaled per-axis state shared by
/// [`OnlineStats`] and [`OnlineCovariance`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct AxisState {
    count: usize,
    first: f64,
    is_constant: bool,
    origin: f64,
    exp: i32,
    mean_s: f64,
    m2_s: f64,
}

impl AxisState {
    /// Advance `self` with finite `x` into a candidate `(next_state, delta_in_new_frame,
    /// delta2_in_new_frame, exp_shift_from_old)` without mutating `self`.
    fn step_candidate(self, x: f64, next_count: usize) -> (Self, f64, f64, i32) {
        if self.count == 0 {
            let first = normalize_zero(x);
            return (
                Self {
                    count: 1,
                    first,
                    is_constant: true,
                    origin: first,
                    exp: 0,
                    mean_s: 0.0,
                    m2_s: 0.0,
                },
                0.0,
                0.0,
                0,
            );
        }
        if self.is_constant && same_float(x, self.first) {
            return (
                Self {
                    count: next_count,
                    ..self
                },
                0.0,
                0.0,
                0,
            );
        }

        // Prepare candidate frame `(origin, exp)` with `mean_s`, `m2_s` expressed in it.
        let mut origin = self.origin;
        let mut exp = self.exp;
        let mut mean_s = self.mean_s;
        let mut m2_s = self.m2_s;
        let mut rebased = false;

        let mut d = x - origin;
        if !d.is_finite() {
            // Opposite-sign extreme values overflowed `x - origin`; rebase to `(0.0, 1024)`.
            origin = 0.0;
            let new_exp = 1024;
            mean_s = ldexp(self.origin, -1024)
                + if self.is_constant {
                    0.0
                } else {
                    ldexp(self.mean_s, self.exp - 1024)
                };
            m2_s = if self.is_constant {
                0.0
            } else {
                ldexp(self.m2_s, 2 * (self.exp - 1024))
            };
            exp = new_exp;
            d = x;
            rebased = true;
        }

        if d != 0.0 {
            let d_exp = frexp_exp(d);
            if self.is_constant && !rebased {
                exp = d_exp;
                mean_s = 0.0;
                m2_s = 0.0;
            } else if d_exp > exp {
                let shift = exp - d_exp;
                mean_s = ldexp(mean_s, shift);
                m2_s = ldexp(m2_s, 2 * shift);
                exp = d_exp;
            }
        }

        let z = ldexp(d, -exp);
        let delta = z - mean_s;
        let new_mean_s = mean_s + delta / (next_count as f64);
        let delta2 = z - new_mean_s;
        let new_m2_s = m2_s + delta * delta2;
        let total_shift = if self.is_constant { 0 } else { self.exp - exp };

        let next = Self {
            count: next_count,
            first: self.first,
            is_constant: false,
            origin,
            exp,
            mean_s: new_mean_s,
            m2_s: new_m2_s,
        };
        (next, delta, delta2, total_shift)
    }

    /// Combine two non-empty `AxisState` values into `(merged_state, delta_s,
    /// eff_n, exp_shift_a, exp_shift_b)` where `delta_s = mean_b_c - mean_a_c`
    /// in the common `(origin, exp)` frame.
    fn merge_candidate(a: Self, b: Self, total_count: usize) -> (Self, f64, f64, i32, i32) {
        if a.is_constant && b.is_constant && same_float(a.first, b.first) {
            return (
                Self {
                    count: total_count,
                    ..a
                },
                0.0,
                0.0,
                0,
                0,
            );
        }

        let origin_diff = b.origin - a.origin;
        let (common_origin, common_exp, mean_a_c, mean_b_c) = if origin_diff.is_finite() {
            let mut max_exp = i32::MIN;
            if !a.is_constant {
                max_exp = max_exp.max(a.exp);
            }
            if !b.is_constant {
                max_exp = max_exp.max(b.exp);
            }
            if origin_diff != 0.0 {
                max_exp = max_exp.max(frexp_exp(origin_diff));
            }
            let ce = if max_exp == i32::MIN { 0 } else { max_exp };
            let ma = if a.is_constant {
                0.0
            } else {
                ldexp(a.mean_s, a.exp - ce)
            };
            let mb = ldexp(origin_diff, -ce)
                + if b.is_constant {
                    0.0
                } else {
                    ldexp(b.mean_s, b.exp - ce)
                };
            (a.origin, ce, ma, mb)
        } else {
            let ce = 1024;
            let ma = ldexp(a.origin, -ce)
                + if a.is_constant {
                    0.0
                } else {
                    ldexp(a.mean_s, a.exp - ce)
                };
            let mb = ldexp(b.origin, -ce)
                + if b.is_constant {
                    0.0
                } else {
                    ldexp(b.mean_s, b.exp - ce)
                };
            (0.0, ce, ma, mb)
        };

        let shift_a = if a.is_constant { 0 } else { a.exp - common_exp };
        let shift_b = if b.is_constant { 0 } else { b.exp - common_exp };
        let m2_a_c = if a.is_constant {
            0.0
        } else {
            ldexp(a.m2_s, 2 * shift_a)
        };
        let m2_b_c = if b.is_constant {
            0.0
        } else {
            ldexp(b.m2_s, 2 * shift_b)
        };

        let n_a = a.count as f64;
        let n_b = b.count as f64;
        let w_b = n_b / (total_count as f64);
        let eff_n = n_a * w_b;
        let delta = mean_b_c - mean_a_c;
        let merged_mean_s = mean_a_c + delta * w_b;
        let merged_m2_s = m2_a_c + m2_b_c + delta * delta * eff_n;

        let merged = Self {
            count: total_count,
            first: a.first,
            is_constant: false,
            origin: common_origin,
            exp: common_exp,
            mean_s: merged_mean_s,
            m2_s: merged_m2_s,
        };
        (merged, delta, eff_n, shift_a, shift_b)
    }

    fn get_mean(&self) -> crate::Result<Option<f64>> {
        if self.count == 0 {
            return Ok(None);
        }
        if self.is_constant {
            return Ok(Some(self.first));
        }
        let m = self.origin + ldexp(self.mean_s, self.exp);
        if !m.is_finite() {
            return Err(crate::Error::Internal(
                "Online mean overflowed to a non-finite value".into(),
            ));
        }
        Ok(Some(normalize_zero(m)))
    }

    fn get_population_variance(&self) -> crate::Result<Option<f64>> {
        if self.count == 0 {
            return Ok(None);
        }
        if self.count == 1 || self.is_constant {
            return Ok(Some(0.0));
        }
        let v = ldexp(self.m2_s / (self.count as f64), 2 * self.exp);
        if !v.is_finite() || v <= 0.0 {
            return Err(crate::Error::Internal(
                "Population variance overflowed or underflowed out of the positive finite range"
                    .into(),
            ));
        }
        Ok(Some(v))
    }

    fn get_variance(&self) -> crate::Result<Option<f64>> {
        if self.count < 2 {
            return Ok(None);
        }
        if self.is_constant {
            return Ok(Some(0.0));
        }
        let v = ldexp(self.m2_s / ((self.count - 1) as f64), 2 * self.exp);
        if !v.is_finite() || v <= 0.0 {
            return Err(crate::Error::Internal(
                "Sample variance overflowed or underflowed out of the positive finite range".into(),
            ));
        }
        Ok(Some(v))
    }

    fn get_std_dev(&self) -> crate::Result<Option<f64>> {
        if self.count < 2 {
            return Ok(None);
        }
        if self.is_constant {
            return Ok(Some(0.0));
        }
        let s = ldexp((self.m2_s / ((self.count - 1) as f64)).sqrt(), self.exp);
        if !s.is_finite() || s <= 0.0 {
            return Err(crate::Error::Internal(
                "Sample standard deviation overflowed or underflowed out of the positive finite range"
                    .into(),
            ));
        }
        Ok(Some(s))
    }
}

/// Online statistics accumulator using shifted, power-of-two scaled Welford
/// recurrence.
///
/// Updates via [`Self::update`] and merges via [`Self::merge`] are atomic: if an
/// input is non-finite or causes the count limit (`2^53`) to be exceeded, the
/// accumulator state is left unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OnlineStats {
    state: AxisState,
}

impl OnlineStats {
    /// Create a new empty statistics accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Incorporate a finite observation into the accumulator atomically.
    ///
    /// On error, the running state is left unchanged.
    ///
    /// # Arguments
    ///
    /// * `value` - Finite observation to incorporate into the running sample
    ///   moments.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
    ///   when `value` is `NaN` or infinite.
    /// * Returns [`crate::Error::Validation`] if adding the observation would
    ///   exceed the exact binary64 count limit (`2^53`).
    pub fn update(&mut self, value: f64) -> crate::Result<()> {
        if !value.is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(value),
            }
            .into());
        }
        let next_count = self
            .state
            .count
            .checked_add(1)
            .filter(|&n| n <= MAX_EXACT_COUNT)
            .ok_or_else(|| {
                crate::Error::Validation(format!(
                    "OnlineStats observation count exceeds maximum exact count {MAX_EXACT_COUNT}"
                ))
            })?;
        let (next, _, _, _) = self.state.step_candidate(value, next_count);
        self.state = next;
        Ok(())
    }

    /// Merge another statistics accumulator into `self` atomically using
    /// shifted, power-of-two scaled Chan parallel combination.
    ///
    /// On error (e.g., count overflow or exceeding `2^53`), `self` is unchanged.
    ///
    /// # Arguments
    ///
    /// * `other` - Second accumulator whose observations are merged into `self`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Validation`] if the combined count overflows
    ///   `usize` or exceeds `2^53`.
    pub fn merge(&mut self, other: &Self) -> crate::Result<()> {
        if other.state.count == 0 {
            return Ok(());
        }
        let total_count = self
            .state
            .count
            .checked_add(other.state.count)
            .filter(|&n| n <= MAX_EXACT_COUNT)
            .ok_or_else(|| {
                crate::Error::Validation(format!(
                    "Merged OnlineStats count exceeds maximum exact count {MAX_EXACT_COUNT}"
                ))
            })?;
        if self.state.count == 0 {
            self.state = other.state;
            return Ok(());
        }
        let (merged, _, _, _, _) = AxisState::merge_candidate(self.state, other.state, total_count);
        self.state = merged;
        Ok(())
    }

    /// Number of observations accumulated so far.
    #[must_use]
    pub fn get_count(&self) -> usize {
        self.state.count
    }

    /// Sample arithmetic mean, or `Ok(None)` when no observations are recorded.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if the reconstructed mean is non-finite.
    pub fn get_mean(&self) -> crate::Result<Option<f64>> {
        self.state.get_mean()
    }

    /// Sample variance (unbiased `n - 1` denominator), or `Ok(None)` when `count < 2`.
    ///
    /// Returns `Ok(Some(0.0))` for constant samples with `count >= 2`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if variance of a non-constant sample
    ///   overflows or underflows out of the positive finite range.
    pub fn get_variance(&self) -> crate::Result<Option<f64>> {
        self.state.get_variance()
    }

    /// Population variance (`n` denominator), or `Ok(None)` when `count == 0`.
    ///
    /// Returns `Ok(Some(0.0))` when `count == 1` or all observations are identical.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if variance of a non-constant sample
    ///   overflows or underflows out of the positive finite range.
    pub fn get_population_variance(&self) -> crate::Result<Option<f64>> {
        self.state.get_population_variance()
    }

    /// Sample standard deviation directly from scaled moments, or `Ok(None)` when
    /// `count < 2`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if standard deviation of a
    ///   non-constant sample overflows or underflows out of the positive finite range.
    pub fn get_std_dev(&self) -> crate::Result<Option<f64>> {
        self.state.get_std_dev()
    }

    /// Standard error of the mean (`s / sqrt(n)`) directly from scaled moments,
    /// without materializing variance or standard deviation first.
    ///
    /// Returns `Ok(None)` when `count < 2`, and `Ok(Some(0.0))` for constant samples.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if the standard error of a
    ///   non-constant sample overflows or underflows out of the positive finite range.
    pub fn get_stderr(&self) -> crate::Result<Option<f64>> {
        let n = self.state.count;
        if n < 2 {
            return Ok(None);
        }
        if self.state.is_constant {
            return Ok(Some(0.0));
        }
        let se = ldexp(
            ((self.state.m2_s / ((n - 1) as f64)) / (n as f64)).sqrt(),
            self.state.exp,
        );
        if !se.is_finite() || se <= 0.0 {
            return Err(crate::Error::Internal(
                "Standard error overflowed or underflowed out of the positive finite range".into(),
            ));
        }
        Ok(Some(se))
    }

    /// Half-width of the symmetric two-sided `(1 - alpha)` Student-t confidence
    /// interval for an equally weighted sample mean.
    ///
    /// Validates `alpha` in `(0, 1)` before checking sample count or constant-sample
    /// shortcuts. Returns `Ok(None)` when `count < 2`, and `Ok(Some(0.0))` for a
    /// constant sample without invoking inverse-t tail arithmetic.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Finite two-sided significance probability strictly in
    ///   `(0.0, 1.0)`; for example, `0.05` yields the 95% interval half-width.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Validation`] if `alpha` is non-finite or
    ///   outside `(0.0, 1.0)`.
    /// * Returns [`crate::Error::Internal`] if standard error cannot be
    ///   represented, `alpha / 2.0` underflows to zero, Student-t lower-tail
    ///   inversion fails, or the half-width overflows/underflows out of `(0, inf)`.
    pub fn get_ci_half_width(&self, alpha: f64) -> crate::Result<Option<f64>> {
        if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
            return Err(crate::Error::Validation(format!(
                "Confidence interval significance alpha must be finite and in (0, 1), got {alpha}"
            )));
        }
        if self.state.count < 2 {
            return Ok(None);
        }
        if self.state.is_constant {
            return Ok(Some(0.0));
        }
        let Some(se) = self.get_stderr()? else {
            return Err(crate::Error::Internal(
                "Standard error unexpectedly unavailable for confidence half-width".into(),
            ));
        };
        let half_alpha = alpha * 0.5;
        if !half_alpha.is_finite() || half_alpha <= 0.0 {
            return Err(crate::Error::Internal(
                "Confidence interval tail probability alpha / 2 underflowed to zero".into(),
            ));
        }
        let df = (self.state.count - 1) as f64;
        let t_low = student_t_inv_cdf(half_alpha, df)
            .map_err(|err| crate::Error::Internal(format!("Student-t tail failure: {err}")))?;
        if !t_low.is_finite() || t_low >= 0.0 {
            return Err(crate::Error::Internal(format!(
                "Invalid lower Student-t critical value {t_low} for alpha={alpha}, df={df}"
            )));
        }
        let h = (-t_low) * se;
        if !h.is_finite() || h <= 0.0 {
            return Err(crate::Error::Internal(
                "Confidence half-width overflowed or underflowed out of the positive finite range"
                    .into(),
            ));
        }
        Ok(Some(h))
    }

    /// Two-sided `(1 - alpha)` Student-t confidence interval `(lower, upper)`
    /// for the population mean.
    ///
    /// Validates `alpha` in `(0, 1)` before checking sample count. Returns
    /// `Ok(None)` when `count < 2`, and `Ok(Some((mean, mean)))` when constant.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Finite two-sided significance probability strictly in
    ///   `(0.0, 1.0)`; for example, `0.05` requests a 95% confidence interval.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Validation`] if `alpha` is non-finite or
    ///   outside `(0.0, 1.0)`.
    /// * Returns [`crate::Error::Internal`] if the half-width or mean cannot be
    ///   resolved, or if positive half-width causes bounds to overflow or collapse
    ///   onto `mean`.
    pub fn get_confidence_interval(&self, alpha: f64) -> crate::Result<Option<(f64, f64)>> {
        let Some(h) = self.get_ci_half_width(alpha)? else {
            return Ok(None);
        };
        let Some(mean) = self.get_mean()? else {
            return Err(crate::Error::Internal(
                "Mean unexpectedly unavailable for confidence interval".into(),
            ));
        };
        if self.state.is_constant {
            return Ok(Some((mean, mean)));
        }
        let lo = mean - h;
        let hi = mean + h;
        if !lo.is_finite() || !hi.is_finite() || !(lo < mean && mean < hi) {
            return Err(crate::Error::Internal(
                "Confidence interval bounds overflowed or collapsed onto the sample mean".into(),
            ));
        }
        Ok(Some((normalize_zero(lo), normalize_zero(hi))))
    }

    /// Reset to empty state.
    pub fn reset(&mut self) {
        self.state = AxisState::default();
    }
}

/// Online covariance accumulator using shifted, power-of-two scaled Welford/Chan
/// recurrences.
///
/// Updates via [`Self::update`] and merges via [`Self::merge`] are atomic.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OnlineCovariance {
    x: AxisState,
    y: AxisState,
    c_s: f64,
}

impl OnlineCovariance {
    /// Create a new empty covariance accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update with a finite sample pair `(x, y)` atomically.
    ///
    /// On error, the running state is untouched.
    ///
    /// # Arguments
    ///
    /// * `x` - Finite observation of the first variable.
    /// * `y` - Finite paired observation of the second variable.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
    ///   when either `x` or `y` is `NaN` or infinite.
    /// * Returns [`crate::Error::Validation`] if adding the observation exceeds
    ///   the exact binary64 count limit (`2^53`).
    pub fn update(&mut self, x: f64, y: f64) -> crate::Result<()> {
        if !x.is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(x),
            }
            .into());
        }
        if !y.is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(y),
            }
            .into());
        }
        let next_count = self
            .x
            .count
            .checked_add(1)
            .filter(|&n| n <= MAX_EXACT_COUNT)
            .ok_or_else(|| {
                crate::Error::Validation(format!(
                    "OnlineCovariance observation count exceeds maximum exact count {MAX_EXACT_COUNT}"
                ))
            })?;

        let (next_x, dx, _, shift_x) = self.x.step_candidate(x, next_count);
        let (next_y, _, d2y, shift_y) = self.y.step_candidate(y, next_count);
        let new_c_s = if next_x.is_constant || next_y.is_constant {
            0.0
        } else {
            let scaled_c = if self.x.is_constant || self.y.is_constant {
                0.0
            } else {
                ldexp(self.c_s, shift_x + shift_y)
            };
            scaled_c + dx * d2y
        };
        self.x = next_x;
        self.y = next_y;
        self.c_s = new_c_s;
        Ok(())
    }

    /// Merge another covariance accumulator into `self` atomically.
    ///
    /// On error (e.g., count overflow or exceeding `2^53`), `self` is unchanged.
    ///
    /// # Arguments
    ///
    /// * `other` - Second covariance accumulator whose paired observations are
    ///   merged into `self`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Validation`] if the combined count overflows
    ///   `usize` or exceeds `2^53`.
    pub fn merge(&mut self, other: &Self) -> crate::Result<()> {
        if other.x.count == 0 {
            return Ok(());
        }
        let total_count = self
            .x
            .count
            .checked_add(other.x.count)
            .filter(|&n| n <= MAX_EXACT_COUNT)
            .ok_or_else(|| {
                crate::Error::Validation(format!(
                    "Merged OnlineCovariance count exceeds maximum exact count {MAX_EXACT_COUNT}"
                ))
            })?;
        if self.x.count == 0 {
            *self = *other;
            return Ok(());
        }

        let (merged_x, delta_x, eff_n, shift_xa, shift_xb) =
            AxisState::merge_candidate(self.x, other.x, total_count);
        let (merged_y, delta_y, _, shift_ya, shift_yb) =
            AxisState::merge_candidate(self.y, other.y, total_count);

        let new_c_s = if merged_x.is_constant || merged_y.is_constant {
            0.0
        } else {
            let c_a = if self.x.is_constant || self.y.is_constant {
                0.0
            } else {
                ldexp(self.c_s, shift_xa + shift_ya)
            };
            let c_b = if other.x.is_constant || other.y.is_constant {
                0.0
            } else {
                ldexp(other.c_s, shift_xb + shift_yb)
            };
            c_a + c_b + delta_x * (delta_y * eff_n)
        };

        self.x = merged_x;
        self.y = merged_y;
        self.c_s = new_c_s;
        Ok(())
    }

    /// Number of paired samples accumulated so far.
    #[must_use]
    pub fn get_count(&self) -> usize {
        self.x.count
    }

    /// Sample mean of `x`, or `Ok(None)` when empty.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if the reconstructed mean is non-finite.
    pub fn get_mean_x(&self) -> crate::Result<Option<f64>> {
        self.x.get_mean()
    }

    /// Sample mean of `y`, or `Ok(None)` when empty.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if the reconstructed mean is non-finite.
    pub fn get_mean_y(&self) -> crate::Result<Option<f64>> {
        self.y.get_mean()
    }

    /// Sample variance of `x` (unbiased), or `Ok(None)` when `count < 2`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if variance of a non-constant series
    ///   overflows or underflows out of the positive finite range.
    pub fn get_variance_x(&self) -> crate::Result<Option<f64>> {
        self.x.get_variance()
    }

    /// Sample variance of `y` (unbiased), or `Ok(None)` when `count < 2`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if variance of a non-constant series
    ///   overflows or underflows out of the positive finite range.
    pub fn get_variance_y(&self) -> crate::Result<Option<f64>> {
        self.y.get_variance()
    }

    /// Sample standard deviation of `x`, or `Ok(None)` when `count < 2`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if standard deviation of a
    ///   non-constant series overflows or underflows out of the positive finite range.
    pub fn get_std_dev_x(&self) -> crate::Result<Option<f64>> {
        self.x.get_std_dev()
    }

    /// Sample standard deviation of `y`, or `Ok(None)` when `count < 2`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if standard deviation of a
    ///   non-constant series overflows or underflows out of the positive finite range.
    pub fn get_std_dev_y(&self) -> crate::Result<Option<f64>> {
        self.y.get_std_dev()
    }

    /// Sample covariance (unbiased `n - 1` denominator), or `Ok(None)` when `count < 2`.
    ///
    /// Returns `Ok(Some(0.0))` when either series is constant with `count >= 2`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if a nonzero co-moment overflows or
    ///   underflows out of the nonzero finite range.
    pub fn get_covariance(&self) -> crate::Result<Option<f64>> {
        if self.x.count < 2 {
            return Ok(None);
        }
        if self.x.is_constant || self.y.is_constant || self.c_s == 0.0 {
            return Ok(Some(0.0));
        }
        let raw = self.c_s / ((self.x.count - 1) as f64);
        let Some(exp_sum) = self.x.exp.checked_add(self.y.exp) else {
            return Err(crate::Error::Internal(
                "Covariance exponent sum overflowed".into(),
            ));
        };
        let cov = ldexp(raw, exp_sum);
        if !cov.is_finite() || cov == 0.0 {
            return Err(crate::Error::Internal(
                "Sample covariance overflowed or underflowed out of the nonzero finite range"
                    .into(),
            ));
        }
        Ok(Some(normalize_zero(cov)))
    }

    /// Pearson correlation coefficient without degenerate zero/clamp defaults.
    ///
    /// Returns `Ok(None)` when `count < 2` or either series is constant.
    /// Never clamps numerical excursions outside `[-1.0, 1.0]`.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if numerical evaluation yields a
    ///   non-finite value or a correlation outside `[-1.0, 1.0]`.
    pub fn get_correlation(&self) -> crate::Result<Option<f64>> {
        if self.x.count < 2 || self.x.is_constant || self.y.is_constant {
            return Ok(None);
        }
        let denom = self.x.m2_s.sqrt() * self.y.m2_s.sqrt();
        let r = if denom.is_finite() && denom > 0.0 {
            self.c_s / denom
        } else {
            f64::NAN
        };
        if r.is_finite() && (-1.0..=1.0).contains(&r) {
            return Ok(Some(normalize_zero(r)));
        }
        if !self.x.m2_s.is_finite()
            || self.x.m2_s <= 0.0
            || !self.y.m2_s.is_finite()
            || self.y.m2_s <= 0.0
            || !self.c_s.is_finite()
        {
            return Err(crate::Error::Internal(
                "Pearson correlation denominator failed to remain positive and finite".into(),
            ));
        }
        if self.c_s == 0.0 {
            return Ok(Some(0.0));
        }
        let hx = frexp_exp(self.x.m2_s).div_euclid(2);
        let hy = frexp_exp(self.y.m2_s).div_euclid(2);
        let mx = ldexp(self.x.m2_s, -2 * hx);
        let my = ldexp(self.y.m2_s, -2 * hy);
        let cn = ldexp(self.c_s, -(hx + hy));
        if !cn.is_finite() {
            return Err(crate::Error::Internal(format!(
                "Pearson correlation {r} escaped the valid [-1, 1] range"
            )));
        }
        let c2_hi = cn * cn;
        let c2_lo = cn.mul_add(cn, -c2_hi);
        let p_hi = mx * my;
        let p_lo = mx.mul_add(my, -p_hi);
        let cauchy_exceeded = match c2_hi.total_cmp(&p_hi) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => c2_lo > p_lo,
        };
        if cauchy_exceeded {
            return Err(crate::Error::Internal(format!(
                "Pearson correlation {r} escaped the valid [-1, 1] range"
            )));
        }
        let q_hi = c2_hi / p_hi;
        let rem = (-q_hi).mul_add(p_hi, c2_hi) + c2_lo - q_hi * p_lo;
        let q = q_hi + rem / p_hi;
        let mag = q.sqrt();
        if !mag.is_finite() || !(0.0..=1.0).contains(&mag) {
            return Err(crate::Error::Internal(
                "Pearson correlation escaped the valid [-1, 1] range".into(),
            ));
        }
        Ok(Some(normalize_zero(cn.signum() * mag)))
    }

    /// Optimal beta coefficient `Cov(X, Y) / Var(Y)` for control-variate
    /// adjustment `X - beta * (Y - E[Y])`.
    ///
    /// Returns `Ok(None)` when `count < 2` or `Y` is constant, and `Ok(Some(0.0))`
    /// when `X` is constant and `Y` varies.
    ///
    /// # Errors
    ///
    /// * Returns [`crate::Error::Internal`] if a nonzero coefficient overflows
    ///   or underflows out of the nonzero finite range.
    pub fn get_optimal_beta(&self) -> crate::Result<Option<f64>> {
        if self.x.count < 2 || self.y.is_constant {
            return Ok(None);
        }
        if self.x.is_constant || self.c_s == 0.0 {
            return Ok(Some(0.0));
        }
        let raw = self.c_s / self.y.m2_s;
        let Some(exp_diff) = self.x.exp.checked_sub(self.y.exp) else {
            return Err(crate::Error::Internal(
                "Control beta exponent difference overflowed".into(),
            ));
        };
        let beta = ldexp(raw, exp_diff);
        if !beta.is_finite() || beta == 0.0 {
            return Err(crate::Error::Internal(
                "Control beta overflowed or underflowed out of the nonzero finite range".into(),
            ));
        }
        Ok(Some(normalize_zero(beta)))
    }

    /// Reset to empty state.
    pub fn reset(&mut self) {
        self.x = AxisState::default();
        self.y = AxisState::default();
        self.c_s = 0.0;
    }
}
