//! Curve evaluation and accessors.

use super::*;

impl HazardCurve {
    /// Start building a hazard curve with identifier `id`.
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
            day_count: DayCount::Act365F,
            par_points: Vec::new(),
            par_interp: ParInterp::Linear,
            max_hazard_rate: DEFAULT_MAX_HAZARD_RATE,
            hazard_calibration: None,
            fx_policy: None,
        }
    }

    /// Construct a flat (constant-intensity) hazard curve.
    ///
    /// The curve stores a single knot at `t = 1` carrying `hazard_rate`; with
    /// flat-forward extrapolation this gives `S(t) = exp(-hazard_rate * t)`
    /// for every non-negative maturity.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"ACME-HZD"`).
    /// * `base_date` - Valuation date anchoring `t = 0`; the curve day count
    ///   is `Act/365F`.
    /// * `hazard_rate` - Constant annual default intensity as a decimal
    ///   fraction (`0.02` is 2% per year); must be finite and non-negative.
    /// * `recovery_rate` - Assumed recovery on default as a decimal fraction
    ///   in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `hazard_rate` is non-finite, negative or
    /// exceeds the default `max_hazard_rate` of `10.0`, or if `recovery_rate`
    /// is outside `[0, 1]`.
    pub fn flat(
        id: impl Into<CurveId>,
        base_date: Date,
        hazard_rate: f64,
        recovery_rate: f64,
    ) -> crate::Result<Self> {
        if !hazard_rate.is_finite() {
            return Err(crate::Error::Validation(format!(
                "HazardCurve::flat hazard_rate must be finite, got {hazard_rate}"
            )));
        }
        Self::builder(id)
            .base_date(base_date)
            .knots([(1.0, hazard_rate)])
            .recovery_rate(recovery_rate)
            .build()
    }

    /// Construct a hazard curve from survival-probability pillars.
    ///
    /// Each pillar `(t, S(t))` is converted to the piecewise-constant hazard
    /// rate of the segment ending at `t`:
    /// `λᵢ = -ln(S(tᵢ) / S(tᵢ₋₁)) / (tᵢ - tᵢ₋₁)` with `S(0) = 1`. A pillar at
    /// `t = 0` is accepted only when its survival probability is `1`.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier.
    /// * `base_date` - Valuation date anchoring `t = 0`; the curve day count
    ///   is `Act/365F`.
    /// * `points` - `(time_years, survival_probability)` pillars. Times must be
    ///   finite and non-negative and may be supplied in any order; survival
    ///   probabilities must lie in `(0, 1]` and be non-increasing in time.
    /// * `recovery_rate` - Assumed recovery on default as a decimal fraction
    ///   in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `points` is empty, a time is negative or
    /// duplicated, a survival probability is outside `(0, 1]` or increases
    /// with time, or `recovery_rate` is outside `[0, 1]`.
    pub fn from_survival_probs(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(f64, f64)],
        recovery_rate: f64,
    ) -> crate::Result<Self> {
        if points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut knots = Vec::with_capacity(sorted.len());
        let mut prev_t = 0.0_f64;
        let mut prev_sp = 1.0_f64;
        for &(t, sp) in &sorted {
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
            if t == 0.0 {
                if (sp - 1.0).abs() > 1e-12 {
                    return Err(crate::Error::Validation(format!(
                        "HazardCurve::from_survival_probs survival probability at t=0 must be 1, got {sp}"
                    )));
                }
                continue;
            }
            if t <= prev_t {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs times must be strictly increasing, got duplicate t={t}"
                )));
            }
            if sp > prev_sp {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve::from_survival_probs survival probability must be non-increasing, got {sp} at t={t} after {prev_sp}"
                )));
            }
            let lambda = if sp.total_cmp(&prev_sp).is_eq() {
                0.0
            } else {
                (prev_sp.ln() - sp.ln()) / (t - prev_t)
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
            .knots(knots)
            .recovery_rate(recovery_rate)
            .build()
    }

    /// Integrate piecewise-constant hazard `∫_{start}^{end} λ(u) du` over `0 <= start <= end`
    /// with finite `start` and `end`.
    #[inline]
    pub(super) fn cumulative_hazard_between(&self, start_time: f64, end_time: f64) -> f64 {
        if end_time <= start_time || end_time <= 0.0 {
            return 0.0;
        }
        let a = start_time.max(0.0);
        let b = end_time;
        let mut acc = crate::math::summation::NeumaierAccumulator::new();
        let mut previous_time = 0.0_f64;
        for (&time, &lambda) in self.knots.iter().zip(self.lambdas.iter()) {
            let overlap = b.min(time) - a.max(previous_time);
            if overlap > 0.0 && lambda > 0.0 {
                acc.add(lambda * overlap);
            }
            if time >= b {
                return acc.total().max(0.0);
            }
            previous_time = time;
        }
        let tail_start = a.max(previous_time);
        if b > tail_start {
            let tail_lambda = self.lambdas[self.lambdas.len() - 1];
            if tail_lambda > 0.0 {
                acc.add(tail_lambda * (b - tail_start));
            }
        }
        acc.total().max(0.0)
    }

    /// Survival probability S(t) up to time `t` (in **years**).
    ///
    /// # Arguments
    ///
    /// * `t` - Year-fraction time from the curve base date to the query point
    #[must_use]
    pub fn sp(&self, t: f64) -> f64 {
        if t.is_nan() {
            return f64::NAN;
        }
        if t <= 0.0 {
            return 1.0;
        }
        if t == f64::INFINITY {
            let tail_lambda = self.lambdas[self.lambdas.len() - 1];
            if tail_lambda > 0.0 {
                return 0.0;
            }
            let last_t = self.knots.last().copied().unwrap_or(0.0);
            return (-self.cumulative_hazard_between(0.0, last_t)).exp();
        }
        (-self.cumulative_hazard_between(0.0, t)).exp()
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
    /// Returns [`crate::Error::Validation`] if `t1` or `t2` is non-finite,
    /// `t1 < 0`, or `t2 < t1`.
    #[must_use = "computed conditional survival probability should not be discarded"]
    pub fn sp_between(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t1 < 0.0 || t2 < t1 {
            return Err(crate::Error::Validation(format!(
                "sp_between requires finite 0 <= t1 <= t2 (got t1={t1}, t2={t2})"
            )));
        }
        if t1 >= t2 {
            return Ok(1.0);
        }
        Ok((-self.cumulative_hazard_between(t1, t2)).exp())
    }

    /// Default probability between `t1` and `t2`.
    ///
    /// Returns `S(t1) - S(t2) = S(t1) * (1 - exp(-∫_{t1}^{t2} λ(u) du))`, the
    /// unconditional probability of default occurring in `[t1, t2]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `t1` or `t2` is non-finite or `t2 < t1`.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year-fraction of the default interval being queried
    /// * `t2` - End year-fraction of the default interval being queried
    #[must_use = "computed default probability should not be discarded"]
    pub fn default_prob(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t2 < t1 {
            return Err(crate::Error::Validation(format!(
                "default_prob requires finite t2 >= t1 (t1={t1}, t2={t2})"
            )));
        }
        if t2 <= 0.0 || t1 >= t2 {
            return Ok(0.0);
        }
        let a = t1.max(0.0);
        let sp_a = (-self.cumulative_hazard_between(0.0, a)).exp();
        let cond_pd = -(-self.cumulative_hazard_between(a, t2)).exp_m1();
        Ok((sp_a * cond_pd).clamp(0.0, 1.0))
    }

    /// Default probability between two dates, conditional on survival to `start`.
    ///
    /// Computes `1 - exp(-integral(lambda))` directly over the intervening
    /// piecewise-constant hazard segments. This remains well-defined when
    /// unconditional survival at `start` underflows; it does not infer an
    /// observed default from low survival probabilities.
    ///
    /// # Arguments
    ///
    /// * `start` - Conditioning date, converted from the curve base date with
    ///   the curve's day-count convention. Dates before the base are clamped
    ///   to time zero, matching date-based survival queries.
    /// * `end` - End date of the default interval; must be on or after `start`.
    ///   Equal dates return zero. The final hazard rate extends beyond the
    ///   last knot.
    ///
    /// # Returns
    ///
    /// Conditional default probability as a decimal fraction in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns a validation error for reversed dates or if the curve's
    /// day-count convention cannot compute either date's year fraction.
    #[must_use = "computed conditional default probability should not be discarded"]
    pub fn conditional_default_prob(&self, start: Date, end: Date) -> crate::Result<f64> {
        if end < start {
            return Err(crate::Error::Validation(format!(
                "conditional_default_prob requires end >= start (start={start}, end={end})"
            )));
        }
        if start == end {
            return Ok(0.0);
        }
        let start_time = self.year_fraction_to(start)?.max(0.0);
        let end_time = self.year_fraction_to(end)?.max(0.0);
        Ok(-(-self.cumulative_hazard_between(start_time, end_time)).exp_m1())
    }

    /// Instantaneous hazard rate λ(t) at time `t`.
    ///
    /// For piecewise-constant hazard curves, this returns the lambda value
    /// corresponding to the interval containing `t`.
    ///
    /// Hazards are right-continuous at knot boundaries. λᵢ applies to the
    /// segment *ending* at knot tᵢ (ISDA-style): `hazard_rate(tᵢ) = λᵢ` and
    /// the next segment starts immediately after tᵢ. An explicit t≈0 knot is
    /// ignored for this lookup except at `t <= 0`, which returns the first
    /// stored lambda.
    ///
    /// # Arguments
    ///
    /// * `t` - Year-fraction from the curve base date to the query point; values
    ///   at or below zero return the first segment's hazard rate
    ///
    /// # Panics
    ///
    /// Panics only if the curve's internal invariant is violated and it has no
    /// hazard-rate segment. Public constructors reject that state.
    #[must_use]
    pub fn hazard_rate(&self, t: f64) -> f64 {
        // A valid hazard curve always has at least one lambda.
        assert!(
            !self.lambdas.is_empty(),
            "HazardCurve invariant violated: empty lambdas"
        );
        if t <= 0.0 {
            return self.lambdas[0];
        }

        let idx = self.knots.partition_point(|&k| k < t);
        let idx = idx.min(self.lambdas.len() - 1);
        self.lambdas[idx]
    }

    /// Survival probability on a specific calendar date using the curve's day-count.
    ///
    /// This is the date-based equivalent of [`sp`](Self::sp), consistent with
    /// `DiscountCurve::df_on_date` and `ForwardCurve::df_on_date`.
    ///
    /// # Errors
    ///
    /// Returns an error if the year fraction calculation fails.
    #[inline]
    #[must_use = "computed survival probability should not be discarded"]
    pub fn sp_on_date(&self, date: Date) -> crate::Result<f64> {
        let t = self.year_fraction_to(date)?;
        Ok(self.sp(t))
    }

    /// Hazard rate on a specific calendar date using the curve's day-count.
    ///
    /// This is the date-based equivalent of [`hazard_rate`](Self::hazard_rate).
    ///
    /// # Errors
    ///
    /// Returns an error if the year fraction calculation fails.
    #[inline]
    #[must_use = "computed hazard rate should not be discarded"]
    pub fn hazard_rate_on_date(&self, date: Date) -> crate::Result<f64> {
        let t = self.year_fraction_to(date)?;
        Ok(self.hazard_rate(t))
    }

    /// Evaluate survival probabilities at the provided calendar dates.
    ///
    /// Each date is converted to a year fraction from [`Self::base_date`] with
    /// this curve's [`Self::day_count`], then evaluated with [`Self::sp`].
    /// Results preserve the input order, and dates on or before the base date
    /// therefore return `1.0`. Values are clamped to `[0, 1]` as a final
    /// numerical safeguard.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected day-count convention cannot calculate
    /// a year fraction for any supplied date. No partial vector is returned.
    #[must_use = "computed survival probabilities should not be discarded"]
    pub fn survival_at_dates(&self, dates: &[Date]) -> crate::Result<Vec<f64>> {
        dates
            .iter()
            .map(|&date| Ok(self.sp(self.year_fraction_to(date)?).clamp(0.0, 1.0)))
            .collect()
    }

    /// Unique identifier used to register and resolve this hazard curve.
    pub fn id(&self) -> &CurveId {
        &self.id
    }
    /// Largest hazard rate the builder accepted for this curve; carried by
    /// rebuilds and the wire form.
    pub fn max_hazard_rate(&self) -> f64 {
        self.max_hazard_rate
    }

    /// Curve valuation **base date**.
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Recovery rate metadata used when mapping spreads↔hazards during bootstrap.
    pub fn recovery_rate(&self) -> f64 {
        self.recovery_rate
    }

    /// Day count convention associated with this curve's time axis.
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Get the currency of the protection leg.
    pub fn currency(&self) -> Option<Currency> {
        self.currency
    }

    /// Get the issuer name.
    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_deref()
    }

    /// Debt seniority metadata associated with this curve.
    #[inline]
    #[must_use]
    pub const fn seniority(&self) -> Option<Seniority> {
        self.seniority
    }

    /// Opaque FX policy stamp set by the curve constructor; see
    /// [`crate::market_data::term_structures::DiscountCurve::fx_policy`] for the contract.
    #[inline]
    pub fn fx_policy(&self) -> Option<&str> {
        self.fx_policy.as_deref()
    }

    /// Access the knot points (time, lambda) for inspection or modification.
    pub fn knot_points(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.knots
            .iter()
            .zip(self.lambdas.iter())
            .map(|(&t, &lambda)| (t, lambda))
    }

    /// Access the par spread points for inspection.
    pub fn par_spread_points(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.par_tenors
            .iter()
            .zip(self.par_spreads_bp.iter())
            .map(|(&t, &spread)| (t, spread))
    }
    /// Get the default interpolation method for par spreads.
    pub fn par_interp(&self) -> ParInterp {
        self.par_interp
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

    /// Helper: compute year fraction from base date to target date using the curve's day-count.
    #[inline]
    fn year_fraction_to(&self, date: Date) -> crate::Result<f64> {
        self.day_count.signed_year_fraction(
            self.base,
            date,
            crate::dates::DayCountContext::default(),
        )
    }

    /// Return an interpolated par spread in basis points for reporting.
    /// Linear interpolation in spread, with log-linear fallback when values are positive and requested.
    ///
    /// # Arguments
    ///
    /// * `t` - Year fraction from the curve base date to the quoted CDS horizon.
    /// * `method` - Interpolation of stored par-spread quotes; log-linear falls
    ///   back to linear when a quote is non-positive.
    #[must_use]
    pub fn cds_quote_bp(&self, t: f64, method: ParInterp) -> f64 {
        if self.par_tenors.len() < 2 || self.par_tenors.len() != self.par_spreads_bp.len() {
            let lambda = self.hazard_rate(t.max(0.0));
            return (lambda * (1.0 - self.recovery_rate) * 10_000.0).max(0.0);
        }

        // Log-linear falls back to linear when the spreads cannot be logged.
        if method == ParInterp::LogLinear
            && let Ok(strat) = LogLinearStrategy::from_raw(
                &self.par_tenors,
                &self.par_spreads_bp,
                ExtrapolationPolicy::FlatForward,
            )
        {
            return strat.interp(
                t,
                &self.par_tenors,
                &self.par_spreads_bp,
                ExtrapolationPolicy::FlatForward,
            );
        }
        LinearStrategy.interp(
            t,
            &self.par_tenors,
            &self.par_spreads_bp,
            ExtrapolationPolicy::FlatForward,
        )
    }
}
