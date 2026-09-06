//! Curve evaluation and accessors.

use super::*;

impl HazardCurve {
    /// Start building a hazard curve with identifier `id`.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> HazardCurveBuilder {
        let base =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        HazardCurveBuilder {
            id: id.into(),
            base,
            points: Vec::new(),
            recovery_rate: None,
            issuer: None,
            seniority: None,
            currency: None,
            day_count: DayCount::Act365F,
            par_points: Vec::new(),
            par_interp: ParInterp::Linear,
            survival_interp: InterpStyle::LogLinear,
            max_hazard_rate: 10.0,
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
            if t <= 1e-9 {
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
            let lambda = -(sp / prev_sp).ln() / (t - prev_t);
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

    /// Survival probability S(t) up to time `t` (in **years**).
    ///
    /// # Arguments
    ///
    /// * `t` - Year-fraction time from the curve or surface base date to the query point
    #[must_use]
    pub fn sp(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 1.0;
        }
        if let Some(&last_t) = self.knots.last() {
            if t > last_t {
                let survival_at_last = self.interp.interp(last_t);
                let tail_hazard = self.lambdas[self.lambdas.len() - 1];
                return survival_at_last * (-tail_hazard * (t - last_t)).exp();
            }
        }
        self.interp.interp(t)
    }

    /// Default probability between `t1` and `t2`.
    ///
    /// Returns `S(t1) - S(t2)`, the probability of default occurring
    /// in the interval `[t1, t2]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `t2 < t1`.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year-fraction of the forward or rate interval being queried
    /// * `t2` - End year-fraction of the forward or rate interval being queried
    #[must_use = "computed default probability should not be discarded"]
    pub fn default_prob(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if t2 < t1 {
            return Err(crate::Error::Validation(format!(
                "default_prob requires t2 >= t1 (t1={t1}, t2={t2})"
            )));
        }
        let sp1 = self.sp(t1);
        let sp2 = self.sp(t2);
        Ok(sp1 - sp2)
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
    /// `DiscountCurve::df_on_date_curve` and `ForwardCurve::df_on_date_curve`.
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
        let base = self.base_date();
        let day_count = self.day_count();
        let mut survival = Vec::with_capacity(dates.len());

        for &date in dates {
            let t = day_count.year_fraction(base, date, DayCountContext::default())?;
            let sp = self.sp(t).clamp(0.0, 1.0);
            survival.push(sp);
        }

        Ok(survival)
    }

    /// Unique identifier used to register and resolve this hazard curve.
    pub fn id(&self) -> &CurveId {
        &self.id
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
        crate::market_data::term_structures::common::year_fraction_to(
            self.base,
            date,
            self.day_count,
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

        match method {
            ParInterp::Linear => {
                let strat = LinearStrategy;
                strat.interp(
                    t,
                    &self.par_tenors,
                    &self.par_spreads_bp,
                    ExtrapolationPolicy::FlatForward,
                )
            }
            ParInterp::LogLinear => {
                if let Ok(strat) = LogLinearStrategy::from_raw(
                    &self.par_tenors,
                    &self.par_spreads_bp,
                    ExtrapolationPolicy::FlatForward,
                ) {
                    strat.interp(
                        t,
                        &self.par_tenors,
                        &self.par_spreads_bp,
                        ExtrapolationPolicy::FlatForward,
                    )
                } else {
                    let strat = LinearStrategy;
                    strat.interp(
                        t,
                        &self.par_tenors,
                        &self.par_spreads_bp,
                        ExtrapolationPolicy::FlatForward,
                    )
                }
            }
        }
    }
}

impl Survival for HazardCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }

    #[inline]
    fn sp(&self, t: f64) -> f64 {
        self.sp(t)
    }

    #[inline]
    fn base_date(&self) -> Option<Date> {
        Some(self.base_date())
    }

    #[inline]
    fn day_count(&self) -> DayCount {
        self.day_count()
    }
}

/// Convert piecewise-constant hazard knots `(tᵢ, λᵢ)` into survival-probability
/// interpolation pillars `(tᵢ, S(tᵢ))` anchored at `(0, 1)`.
///
/// This is the **single canonical λ-attribution convention** used by both the
/// builder (`HazardCurveBuilder::build`) and in-place rebuilds
/// (`HazardCurve::rebuild_interp`, the `MarketContext::bump` / CS01 path):
///
/// - **Ending-segment attribution**: λᵢ applies to the segment *ending* at tᵢ
///   (segment `(tᵢ₋₁, tᵢ]` accrues `λᵢ·(tᵢ − tᵢ₋₁)`; segment `(0, t₁]` uses λ₁).
///   A redundant t≈0 knot is ignored for the integral; its lambda is not
///   applied to `(0, t₁]`.
///
/// Sharing this function guarantees that bumping a curve in place with a
/// zero-size bump is an exact no-op (no silent re-attribution of base hazards
/// into spurious CS01 components).
pub(super) fn survival_pillars(knots: &[f64], lambdas: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut interp_knots = Vec::with_capacity(knots.len() + 1);
    let mut interp_sp = Vec::with_capacity(knots.len() + 1);
    interp_knots.push(0.0);
    interp_sp.push(1.0);

    let mut accum = 0.0;
    let mut prev_t = 0.0;

    for (&t, &lambda) in knots.iter().zip(lambdas.iter()) {
        if t <= 1e-9 {
            continue;
        }
        accum += lambda * (t - prev_t);
        interp_knots.push(t);
        interp_sp.push((-accum).exp());
        prev_t = t;
    }

    (interp_knots, interp_sp)
}
