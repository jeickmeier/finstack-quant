//! Hazard-curve factories, checked local integration, survival/PD queries, and accessors.

use super::super::common::{signed_curve_year_fraction, stable_positive_log_ratio};
use super::*;

/// Result of direct local intensity integration over `[a, b]`.
///
/// Separates representable non-negative finite integrals from genuine overflow
/// of positive products/sums so probability callers return saturating `S = 0` /
/// conditional `PD = 1`, whereas explicit finite-integral callers fail closed.
enum HazardIntegral {
    Finite(f64),
    Overflow,
}

impl HazardCurve {
    /// Start building a hazard curve with identifier `id`.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"ACME-HZD"`).
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> HazardCurveBuilder {
        HazardCurveBuilder {
            id: id.into(),
            base: None,
            points: Vec::new(),
            recovery_rate: None,
            issuer: None,
            seniority: None,
            currency: None,
            day_count: None,
            calendar_id: None,
            hazard_calibration: None,
            fx_policy: None,
            source: None,
        }
    }

    /// Construct a flat (constant-intensity) hazard curve.
    ///
    /// Stores a single ending pillar `(1.0, hazard_rate)` whose constant tail
    /// continuation yields `S(t) = exp(-hazard_rate * t)` for every `t >= 0`.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"ACME-HZD"`).
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `hazard_rate` - Constant annual default intensity as a decimal
    ///   fraction (`0.02` is 2% per year); must be finite and non-negative.
    /// * `recovery_rate` - Assumed recovery on default as a decimal fraction
    ///   in `[0, 1]`.
    /// * `day_count` - Explicit supported curve day-count convention (cannot be
    ///   calendar-dependent `Bus252`, `ActActIsma`, `Act365L`, or
    ///   `ThirtyE360Isda`).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `hazard_rate` is non-finite or negative,
    /// `recovery_rate` is outside `[0, 1]`, or `day_count` is unsupported.
    pub fn flat(
        id: impl Into<CurveId>,
        base_date: Date,
        hazard_rate: f64,
        recovery_rate: f64,
        day_count: DayCount,
    ) -> crate::Result<Self> {
        Self::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .knots([(1.0, hazard_rate)])
            .recovery_rate(recovery_rate)
            .build()
    }

    /// Construct a hazard curve from strictly ordered survival-probability pillars.
    ///
    /// Each positive-time pillar `(tᵢ, Sᵢ)` defines the piecewise-constant
    /// intensity over `(tᵢ₋₁, tᵢ]` via `-ln(Sᵢ / Sᵢ₋₁) / (tᵢ - tᵢ₋₁)`. An
    /// optional leading `(0.0, 1.0)` point is admitted as normalization without
    /// emitting a `t = 0` knot; at least one `t > 0` pillar is required.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier.
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `points` - Strictly ascending `(time_years, survival_probability)`
    ///   pillars with finite `t >= 0` (`t = 0` permitted only at index 0 with
    ///   exact `S = 1.0`), `0 < S <= 1`, and non-increasing survival.
    /// * `recovery_rate` - Assumed recovery on default as a decimal fraction
    ///   in `[0, 1]`.
    /// * `day_count` - Explicit supported curve day-count convention.
    ///
    /// # Errors
    ///
    /// Returns an error when `points` is empty or has no `t > 0` pillar, times
    /// are negative or not strictly increasing as supplied, an origin point has
    /// `S != 1.0`, any survival probability is outside `(0, 1]` or increases
    /// with time, derived intensity overflows or underflows non-equal pillars
    /// to zero, `recovery_rate` is outside `[0, 1]`, or `day_count` is invalid.
    pub fn from_survival_probs(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(f64, f64)],
        recovery_rate: f64,
        day_count: DayCount,
    ) -> crate::Result<Self> {
        if points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        crate::validation::validate_f64_unit_interval(recovery_rate, "recovery_rate")?;

        let mut knots = Vec::with_capacity(points.len());
        let mut prev_t = 0.0_f64;
        let mut prev_sp = 1.0_f64;
        for (idx, &(t, sp)) in points.iter().enumerate() {
            if !t.is_finite() || t < 0.0 {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs time must be finite and non-negative, got t={t}"
                )));
            }
            if !sp.is_finite() || sp <= 0.0 || sp > 1.0 {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs survival probability must be in (0, 1], got {sp} at t={t}"
                )));
            }
            if idx == 0 && exact_f64_eq(t, 0.0) {
                if !exact_f64_eq(sp, 1.0) {
                    return Err(crate::Error::Validation(format!(
                        "HazardCurve::from_survival_probs survival probability at t=0 must equal 1.0 exactly, got {sp}"
                    )));
                }
                continue;
            }
            if !exact_f64_eq(prev_t, 0.0) && t <= prev_t {
                return Err(InputError::NonMonotonicKnots.into());
            }
            if t <= 0.0 {
                return Err(InputError::NonMonotonicKnots.into());
            }
            if sp > prev_sp {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs survival probability must be non-increasing, got {sp} at t={t} after {prev_sp}"
                )));
            }
            let lambda = if exact_f64_eq(sp, prev_sp) {
                0.0
            } else {
                let dt = t - prev_t;
                if !dt.is_finite() || dt <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "HazardCurve::from_survival_probs positive time step invalid over [{prev_t}, {t}]"
                    )));
                }
                let neg_log = stable_positive_log_ratio(prev_sp, sp)?;
                let rate = neg_log / dt;
                if !rate.is_finite() || rate <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "HazardCurve::from_survival_probs derived hazard rate overflowed or underflowed at t={t}"
                    )));
                }
                rate
            };
            knots.push((t, lambda));
            prev_t = t;
            prev_sp = sp;
        }
        if knots.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        Self::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .knots(knots)
            .recovery_rate(recovery_rate)
            .build()
    }

    /// Directly integrate piecewise-constant intensities over `0 <= a <= b`.
    fn integrate_segments<I>(
        segments: I,
        tail_lambda: f64,
        a: f64,
        b: f64,
    ) -> crate::Result<HazardIntegral>
    where
        I: Iterator<Item = (f64, f64)>,
    {
        if exact_f64_eq(a, b) {
            return Ok(HazardIntegral::Finite(0.0));
        }
        let mut acc = crate::math::summation::NeumaierAccumulator::new();
        let mut prev_time = 0.0_f64;
        for (time, lambda) in segments {
            let lo = a.max(prev_time);
            let hi = b.min(time);
            if hi > lo {
                let overlap = hi - lo;
                if !exact_f64_eq(lambda, 0.0) {
                    let inc = lambda * overlap;
                    if !inc.is_finite() {
                        return Ok(HazardIntegral::Overflow);
                    }
                    if exact_f64_eq(inc, 0.0) {
                        return Err(crate::Error::Validation(
                            "integrated hazard contribution underflowed positive product to zero"
                                .to_string(),
                        ));
                    }
                    if acc.add(inc).is_err() {
                        return Ok(HazardIntegral::Overflow);
                    }
                }
            }
            if time >= b {
                let total = match acc.get_total() {
                    Ok(v) if v.is_finite() => v.max(0.0),
                    _ => return Ok(HazardIntegral::Overflow),
                };
                return Ok(HazardIntegral::Finite(if exact_f64_eq(total, 0.0) {
                    0.0
                } else {
                    total
                }));
            }
            prev_time = time;
        }
        let tail_start = a.max(prev_time);
        if b > tail_start && !exact_f64_eq(tail_lambda, 0.0) {
            let overlap = b - tail_start;
            let inc = tail_lambda * overlap;
            if !inc.is_finite() {
                return Ok(HazardIntegral::Overflow);
            }
            if exact_f64_eq(inc, 0.0) {
                return Err(crate::Error::Validation(
                    "integrated tail hazard contribution underflowed positive product to zero"
                        .to_string(),
                ));
            }
            if acc.add(inc).is_err() {
                return Ok(HazardIntegral::Overflow);
            }
        }
        let total = match acc.get_total() {
            Ok(v) if v.is_finite() => v.max(0.0),
            _ => return Ok(HazardIntegral::Overflow),
        };
        Ok(HazardIntegral::Finite(if exact_f64_eq(total, 0.0) {
            0.0
        } else {
            total
        }))
    }

    /// Evaluate piecewise-constant left-continuous hazard intensity `λ(t)` on `knots`/`lambdas`.
    #[inline]
    pub(super) fn hazard_rate_slices(knots: &[f64], lambdas: &[f64], t: f64) -> f64 {
        if exact_f64_eq(t, 0.0) {
            return lambdas[0];
        }
        let idx = knots.partition_point(|&k| k < t).min(lambdas.len() - 1);
        lambdas[idx]
    }

    /// Evaluate piecewise-constant hazard intensity `λ(t)` on `(time, lambda)` pairs for `t >= 0`.
    #[inline]
    pub(super) fn hazard_rate_in_pairs(pairs: &[(f64, f64)], t: f64) -> f64 {
        if exact_f64_eq(t, 0.0) {
            return pairs[0].1;
        }
        let idx = pairs.partition_point(|&(k, _)| k < t).min(pairs.len() - 1);
        pairs[idx].1
    }

    #[inline]
    fn integrate_local(&self, a: f64, b: f64) -> crate::Result<HazardIntegral> {
        let tail_lambda = self.lambdas[self.lambdas.len() - 1];
        Self::integrate_segments(
            self.knots.iter().copied().zip(self.lambdas.iter().copied()),
            tail_lambda,
            a,
            b,
        )
    }

    #[inline]
    fn integrate_source(
        src: &HazardConditionSource,
        a: f64,
        b: f64,
    ) -> crate::Result<HazardIntegral> {
        let tail_lambda = src.knot_points[src.knot_points.len() - 1].1;
        Self::integrate_segments(src.knot_points.iter().copied(), tail_lambda, a, b)
    }

    /// Integrated hazard `J(t1, t2) = ∫_{t1}^{t2} λ(u) du` for finite `0 <= t1 <= t2`.
    ///
    /// # Arguments
    ///
    /// * `t1` - Finite non-negative start year fraction from [`base_date`](Self::base_date).
    /// * `t2` - Finite end year fraction from [`base_date`](Self::base_date) with `t2 >= t1`.
    ///
    /// # Errors
    ///
    /// Returns an error if `t1` or `t2` is non-finite, `t1 < 0.0`, `t2 < t1`,
    /// an active positive hazard contribution underflows to zero, or the
    /// integral overflows `f64`.
    #[must_use = "computed integrated hazard should not be discarded"]
    pub fn integrated_hazard(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t1 < 0.0 || t2 < t1 {
            return Err(crate::Error::Validation(format!(
                "integrated_hazard requires finite 0 <= t1 <= t2 (got t1={t1}, t2={t2})"
            )));
        }
        match self.integrate_local(t1, t2)? {
            HazardIntegral::Finite(val) => Ok(val),
            HazardIntegral::Overflow => Err(crate::Error::Validation(format!(
                "integrated_hazard overflowed f64 over [{t1}, {t2}]"
            ))),
        }
    }

    /// Integrated hazard `∫_{start}^{end} λ(u) du` between two chronological dates.
    ///
    /// Conditioned curves map both dates onto their retained source clock.
    ///
    /// # Arguments
    ///
    /// * `start` - Start date on or after [`base_date`](Self::base_date).
    /// * `end` - End date on or after `start`.
    ///
    /// # Errors
    ///
    /// Returns an error if `start < base_date()`, `end < start`, day-count
    /// mapping fails, a positive contribution underflows to zero, or the
    /// integral overflows `f64`.
    #[must_use = "computed integrated hazard should not be discarded"]
    pub fn integrated_hazard_between_dates(&self, start: Date, end: Date) -> crate::Result<f64> {
        let (a, b, integral) = self.integrated_between_dates_classified(start, end)?;
        match integral {
            HazardIntegral::Finite(val) => Ok(val),
            HazardIntegral::Overflow => Err(crate::Error::Validation(format!(
                "integrated_hazard_between_dates overflowed f64 over [{a}, {b}]"
            ))),
        }
    }

    fn integrated_between_dates_classified(
        &self,
        start: Date,
        end: Date,
    ) -> crate::Result<(f64, f64, HazardIntegral)> {
        if start < self.base {
            return Err(crate::Error::Validation(format!(
                "date query {start} precedes HazardCurve base_date ({})",
                self.base
            )));
        }
        if end < start {
            return Err(crate::Error::Validation(format!(
                "date interval requires end >= start (start={start}, end={end})"
            )));
        }
        if let Some(src) = &self.source {
            let start_t = self.source_year_fraction_to(src, start)?;
            let end_t = self.source_year_fraction_to(src, end)?;
            if !start_t.is_finite() || !end_t.is_finite() || start_t < src.offset || end_t < start_t
            {
                return Err(crate::Error::Validation(format!(
                    "mapped source dates must be finite and ordered from offset {} (got [{start_t}, {end_t}])",
                    src.offset
                )));
            }
            if start == end || exact_f64_eq(start_t, end_t) {
                return Ok((start_t, end_t, HazardIntegral::Finite(0.0)));
            }
            let integral = Self::integrate_source(src, start_t, end_t)?;
            return Ok((start_t, end_t, integral));
        }

        let start_t = self.year_fraction_to(start)?;
        let end_t = self.year_fraction_to(end)?;
        if !start_t.is_finite() || !end_t.is_finite() || start_t < 0.0 || end_t < start_t {
            return Err(crate::Error::Validation(format!(
                "mapped dates must be finite and non-negative with start_t <= end_t (got [{start_t}, {end_t}])"
            )));
        }
        if start == end || exact_f64_eq(start_t, end_t) {
            return Ok((start_t, end_t, HazardIntegral::Finite(0.0)));
        }
        let integral = self.integrate_local(start_t, end_t)?;
        Ok((start_t, end_t, integral))
    }

    /// Survival probability `S(t) = exp(-∫₀ᵗ λ(u) du)` at time `t >= 0`.
    ///
    /// # Arguments
    ///
    /// * `t` - Finite non-negative year fraction from [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite or negative (`t < 0.0`), or if a
    /// positive hazard contribution underflows to zero during integration.
    #[must_use = "computed survival probability should not be discarded"]
    pub fn sp(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "HazardCurve::sp requires finite non-negative time t >= 0, got {t}"
            )));
        }
        if exact_f64_eq(t, 0.0) {
            return Ok(1.0);
        }
        match self.integrate_local(0.0, t)? {
            HazardIntegral::Finite(integral) => Ok((-integral).exp()),
            HazardIntegral::Overflow => Ok(0.0),
        }
    }

    /// Conditional survival probability `P(τ > t2 | τ > t1) = exp(-∫_{t1}^{t2} λ(u) du)`.
    ///
    /// Evaluates the local hazard integral directly so conditional survival
    /// remains well-defined and free of `0.0 / 0.0` `NaN` when unconditional
    /// survival `S(t1)` underflows on distressed long-dated curves.
    ///
    /// # Arguments
    ///
    /// * `t1` - Finite non-negative conditioning time in years from the base date.
    /// * `t2` - Finite target time in years, greater than or equal to `t1`.
    ///
    /// # Errors
    ///
    /// Returns an error if `t1` or `t2` is non-finite, `t1 < 0.0`, `t2 < t1`,
    /// or a positive hazard contribution underflows to zero.
    #[must_use = "computed conditional survival probability should not be discarded"]
    pub fn sp_between(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t1 < 0.0 || t2 < t1 {
            return Err(crate::Error::Validation(format!(
                "sp_between requires finite 0 <= t1 <= t2 (got t1={t1}, t2={t2})"
            )));
        }
        if exact_f64_eq(t1, t2) {
            return Ok(1.0);
        }
        match self.integrate_local(t1, t2)? {
            HazardIntegral::Finite(integral) => Ok((-integral).exp()),
            HazardIntegral::Overflow => Ok(0.0),
        }
    }

    /// Unconditional default probability on `[t1, t2]`, equal to `S(t1) - S(t2)`.
    ///
    /// Computes `exp(-J(0, t1)) * (-expm1(-J(t1, t2)))` locally in log/scaled
    /// form so small interval probabilities remain accurate even after large
    /// cumulative hazard histories.
    ///
    /// # Arguments
    ///
    /// * `t1` - Finite non-negative start year fraction (`t1 >= 0.0`).
    /// * `t2` - Finite end year fraction (`t2 >= t1`).
    ///
    /// # Errors
    ///
    /// Returns an error if `t1` or `t2` is non-finite, `t1 < 0.0`, `t2 < t1`,
    /// or local integration encounters positive-product zero underflow.
    #[must_use = "computed default probability should not be discarded"]
    pub fn default_prob(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t1 < 0.0 || t2 < t1 {
            return Err(crate::Error::Validation(format!(
                "default_prob requires finite 0 <= t1 <= t2 (got t1={t1}, t2={t2})"
            )));
        }
        if exact_f64_eq(t1, t2) {
            return Ok(0.0);
        }
        let j0 = match self.integrate_local(0.0, t1)? {
            HazardIntegral::Finite(v) => v,
            HazardIntegral::Overflow => return Ok(0.0),
        };
        let cond_pd = match self.integrate_local(t1, t2)? {
            HazardIntegral::Finite(v) => -(-v).exp_m1(),
            HazardIntegral::Overflow => 1.0,
        };
        if exact_f64_eq(cond_pd, 0.0) {
            return Ok(0.0);
        }
        let pd = if j0 > 700.0 {
            (cond_pd.ln() - j0).exp()
        } else {
            (-j0).exp() * cond_pd
        };
        Ok(pd.clamp(0.0, 1.0))
    }

    /// Default probability between two dates, conditional on survival to `start`.
    ///
    /// Evaluates `1 - exp(-∫_{start}^{end} λ(u) du)` directly over the local
    /// intensity intervals (using the retained source clock when conditioned).
    ///
    /// # Arguments
    ///
    /// * `start` - Conditioning date on or after [`base_date`](Self::base_date).
    /// * `end` - End date on or after `start`. Equal dates return `0.0`.
    ///
    /// # Errors
    ///
    /// Returns an error if `start < base_date()`, `end < start`, day-count
    /// mapping fails, or local integration underflows a positive contribution.
    #[must_use = "computed conditional default probability should not be discarded"]
    pub fn conditional_default_prob(&self, start: Date, end: Date) -> crate::Result<f64> {
        let (_, _, integral) = self.integrated_between_dates_classified(start, end)?;
        match integral {
            HazardIntegral::Finite(v) => Ok((-(-v).exp_m1()).clamp(0.0, 1.0)),
            HazardIntegral::Overflow => Ok(1.0),
        }
    }

    /// Left-continuous piecewise-constant hazard intensity `λ(t)` at time `t >= 0`.
    ///
    /// Intensity `λᵢ` applies on `(tᵢ₋₁, tᵢ]` (`t₀ = 0`), with `hazard_rate(0.0)
    /// == λ₁` as the right limit at origin and constant tail `λₙ` beyond `tₙ`.
    ///
    /// # Arguments
    ///
    /// * `t` - Finite non-negative year fraction from [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite or `t < 0.0`.
    #[must_use = "computed hazard rate should not be discarded"]
    pub fn hazard_rate(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "HazardCurve::hazard_rate requires finite non-negative time t >= 0, got {t}"
            )));
        }
        Ok(Self::hazard_rate_slices(&self.knots, &self.lambdas, t))
    }

    /// Survival probability on a specific calendar date `date >= base_date()`.
    ///
    /// Conditioned views evaluate directly from [`base_date`](Self::base_date)
    /// using the retained source clock and intensity schedule.
    ///
    /// # Arguments
    ///
    /// * `date` - Target calendar date on or after [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `date < base_date()`, day-count mapping fails, or
    /// local integration fails.
    #[inline]
    #[must_use = "computed survival probability should not be discarded"]
    pub fn sp_on_date(&self, date: Date) -> crate::Result<f64> {
        if date < self.base {
            return Err(crate::Error::Validation(format!(
                "sp_on_date date {date} precedes HazardCurve base_date ({})",
                self.base
            )));
        }
        if let Some(src) = &self.source {
            let end_t = self.source_year_fraction_to(src, date)?;
            if !end_t.is_finite() || end_t < src.offset {
                return Err(crate::Error::Validation(format!(
                    "mapped date {date} must be finite and >= condition offset {}",
                    src.offset
                )));
            }
            if date == self.base || exact_f64_eq(end_t, src.offset) {
                return Ok(1.0);
            }
            return Ok(match Self::integrate_source(src, src.offset, end_t)? {
                HazardIntegral::Finite(v) => (-v).exp(),
                HazardIntegral::Overflow => 0.0,
            });
        }
        let t = self.year_fraction_to(date)?;
        self.sp(t)
    }

    /// Hazard intensity on a specific calendar date `date >= base_date()`.
    ///
    /// # Arguments
    ///
    /// * `date` - Target calendar date on or after [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `date < base_date()` or day-count calculation fails.
    #[inline]
    #[must_use = "computed hazard rate should not be discarded"]
    pub fn hazard_rate_on_date(&self, date: Date) -> crate::Result<f64> {
        if date < self.base {
            return Err(crate::Error::Validation(format!(
                "hazard_rate_on_date date {date} precedes HazardCurve base_date ({})",
                self.base
            )));
        }
        if let Some(src) = &self.source {
            let t = self.source_year_fraction_to(src, date)?;
            if !t.is_finite() || t < src.offset {
                return Err(crate::Error::Validation(format!(
                    "mapped date {date} must be finite and >= condition offset {}",
                    src.offset
                )));
            }
            if date == self.base || exact_f64_eq(t, src.offset) {
                return Ok(self.lambdas[0]);
            }
            return Ok(Self::hazard_rate_in_pairs(&src.knot_points, t));
        }
        let t = self.year_fraction_to(date)?;
        self.hazard_rate(t)
    }

    /// Evaluate survival probabilities at the provided calendar dates (`date >= base_date()`).
    ///
    /// # Arguments
    ///
    /// * `dates` - Calendar dates on or after [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if any date precedes `base_date()` or day-count
    /// calculation fails. No partial vector is returned on error.
    #[must_use = "computed survival probabilities should not be discarded"]
    pub fn survival_at_dates(&self, dates: &[Date]) -> crate::Result<Vec<f64>> {
        dates
            .iter()
            .map(|&date| Ok(self.sp_on_date(date)?.clamp(0.0, 1.0)))
            .collect()
    }

    /// Unique identifier used to register and resolve this hazard curve.
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Curve valuation **base date**.
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Original unconditioned base date anchoring source hazard coordinates.
    #[inline]
    pub fn source_base_date(&self) -> Date {
        self.source.as_ref().map_or(self.base, |s| s.base_date)
    }

    /// Day-count context of the curve's time axis (including holiday calendar if set).
    #[must_use]
    pub fn day_count_context(&self) -> crate::dates::DayCountContext<'static> {
        crate::dates::DayCountContext {
            calendar: self.calendar.as_ref().map(|cal| cal.calendar),
            ..crate::dates::DayCountContext::default()
        }
    }

    /// Canonical registered identifier of the curve's holiday calendar, if any.
    #[must_use]
    pub fn calendar_id(&self) -> Option<&str> {
        self.calendar.as_ref().map(|cal| cal.id.as_str())
    }

    /// Evaluate piecewise-constant hazard intensity `λ(t)` on the curve's original source clock.
    ///
    /// # Arguments
    ///
    /// * `source_t` - Non-negative time in curve day-count years measured from
    ///   [`Self::source_base_date`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `source_t` is non-finite or negative.
    pub fn source_hazard_rate(&self, source_t: f64) -> crate::Result<f64> {
        if !source_t.is_finite() || source_t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "source hazard time must be finite and non-negative, got {source_t}"
            )));
        }
        if let Some(src) = &self.source {
            return Ok(Self::hazard_rate_in_pairs(&src.knot_points, source_t));
        }
        self.hazard_rate(source_t)
    }

    /// Recovery rate metadata used when mapping spreads and hazards during bootstrap.
    pub fn recovery_rate(&self) -> f64 {
        self.recovery_rate
    }

    /// Day count convention associated with this curve's time axis.
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Currency of the protection leg.
    pub fn currency(&self) -> Option<Currency> {
        self.currency
    }

    /// Issuer name metadata.
    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_deref()
    }

    /// Debt seniority metadata associated with this curve.
    #[inline]
    #[must_use]
    pub const fn seniority(&self) -> Option<Seniority> {
        self.seniority
    }

    /// Opaque FX policy stamp set by the curve constructor.
    #[inline]
    pub fn fx_policy(&self) -> Option<&str> {
        self.fx_policy.as_deref()
    }

    /// Knot coordinates in years from the base date, in increasing order.
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Continuous decimal hazard intensities per year, aligned with the knots.
    pub fn hazard_rates(&self) -> &[f64] {
        &self.lambdas
    }

    /// Iterate over aligned knot years and continuous decimal hazard rates.
    pub fn iter_knots(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.knots.iter().copied().zip(self.lambdas.iter().copied())
    }

    /// Exact inputs used to calibrate this curve, when available.
    #[inline]
    #[must_use]
    pub fn hazard_calibration(
        &self,
    ) -> Option<&crate::market_data::term_structures::HazardCalibrationRecipe> {
        self.hazard_calibration.as_ref()
    }

    /// Number of knot points in the curve.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.knots.len()
    }

    /// Returns `true` if the curve has no knot points.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Compute signed year fraction from `self.base` using the curve's calendar context.
    #[inline]
    fn year_fraction_to(&self, date: Date) -> crate::Result<f64> {
        signed_curve_year_fraction(self.base, date, self.day_count, self.calendar.as_ref())
    }

    /// Compute signed year fraction from `src.base_date` using the curve's calendar context.
    #[inline]
    fn source_year_fraction_to(
        &self,
        src: &HazardConditionSource,
        date: Date,
    ) -> crate::Result<f64> {
        signed_curve_year_fraction(src.base_date, date, self.day_count, self.calendar.as_ref())
    }
}
