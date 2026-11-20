//! Discount-curve construction, queries, and forward derivation.

use super::super::common::factor::{
    FactorClock, FactorTransform as _, FactorTransformSnapshot, dated_points_to_knots,
    eval_factor_df, eval_factor_df_on_date, eval_factor_instantaneous_forward,
    zero_rate_points_to_factors,
};
use super::super::common::{
    signed_curve_year_fraction, stable_positive_log_ratio, validate_curve_clock,
};
use super::{DiscountCurve, DiscountCurveBuilder};
use crate::math::Compounding;
use crate::math::interp::{ExtrapolationPolicy, InterpStyle, exact_f64_eq};
use crate::{
    dates::{Date, DayCount},
    types::CurveId,
};

impl DiscountCurve {
    /// Construct a flat continuously-compounded discount curve.
    ///
    /// Uses a two-knot log-linear representation `(0, 1)` and
    /// `(1, exp(-continuous_rate))` with [`ExtrapolationPolicy::Extend`] on the
    /// supplied `day_count`, preserving `DF(t) = exp(-continuous_rate * t)`
    /// across all representable times.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"USD-OIS"`).
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `continuous_rate` - Continuously-compounded zero rate as a decimal
    ///   fraction (`0.05` is 5%, `-0.01` is -1%).
    /// * `day_count` - Explicit supported curve day-count convention (cannot be
    ///   `ActActIsma`, `Act365L`, `ThirtyE360Isda`, or calendarless `Bus252`).
    ///
    /// # Errors
    ///
    /// Returns an error when `continuous_rate` is non-finite, its 1-year
    /// discount factor overflows or underflows to zero, or `day_count` is not
    /// supported as a standalone curve clock.
    pub fn flat(
        id: impl AsRef<str>,
        base_date: Date,
        continuous_rate: f64,
        day_count: DayCount,
    ) -> crate::Result<Self> {
        let one_year_df = Compounding::Continuous.df_from_rate(continuous_rate, 1.0)?;

        Self::builder(id.as_ref())
            .base_date(base_date)
            .day_count(day_count)
            .knots([(0.0, 1.0), (1.0, one_year_df)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
    }

    /// Construct a discount curve from strictly ordered zero-rate pillars.
    ///
    /// Converts each `(t, r)` pillar via `compounding`, validates strict
    /// ascending order without sorting or merging, and prepends `(0, 1)` when
    /// the first time is strictly positive. Any finite positive DF (including
    /// `DF > 1` for negative rates) is accepted.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"USD-OIS"`).
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `points` - Strictly increasing `(time_years, zero_rate)` pillars with
    ///   finite `time_years >= 0` and decimal zero rates (`0.05` is 5%).
    /// * `compounding` - Explicit compounding convention of the input rates.
    /// * `day_count` - Explicit supported curve day-count convention.
    /// * `style` - Explicit interpolation strategy.
    /// * `extrapolation` - Explicit out-of-range tail policy.
    ///
    /// # Errors
    ///
    /// Returns an error when `points` is empty, unsorted or contains duplicate
    /// times, a rate is outside `compounding`'s valid domain, `day_count` is
    /// invalid, or interpolation setup fails.
    pub fn from_zero_rates(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(f64, f64)],
        compounding: Compounding,
        day_count: DayCount,
        style: InterpStyle,
        extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        let knots = zero_rate_points_to_factors("DiscountCurve", points, compounding)?;
        Self::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .knots(knots)
            .interp(style)
            .extrapolation(extrapolation)
            .build()
    }

    /// Construct a discount curve from strictly ordered dated discount-factor
    /// pillars.
    ///
    /// Each pillar date is mapped from `base_date` using `day_count`, preserving
    /// input order (unsorted or colliding mapped times error), and `(0, 1)` is
    /// prepended when the first pillar time is strictly positive.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"USD-OIS"`).
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `points` - Strictly ascending `(date, discount_factor)` pillars with
    ///   `date >= base_date` and finite strictly positive discount factors.
    /// * `day_count` - Explicit supported curve day-count convention.
    /// * `style` - Explicit interpolation strategy.
    /// * `extrapolation` - Explicit out-of-range tail policy.
    ///
    /// # Errors
    ///
    /// Returns an error when `points` is empty, any date precedes `base_date`,
    /// mapped pillar times are not strictly increasing, `day_count` is invalid
    /// or requires an absent calendar, or any discount factor is non-positive.
    pub fn from_dates(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(Date, f64)],
        day_count: DayCount,
        style: InterpStyle,
        extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        let id: CurveId = id.into();
        let calendar = validate_curve_clock("DiscountCurve", id.as_str(), day_count, None)?;
        let knots = dated_points_to_knots(
            "DiscountCurve",
            base_date,
            points,
            day_count,
            calendar.as_ref(),
        )?;
        Self::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .knots(knots)
            .interp(style)
            .extrapolation(extrapolation)
            .build()
    }

    /// Unique identifier of the curve.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Base (valuation) date of the curve.
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Day-count basis used for discount time mapping.
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Interpolation style configured for this curve.
    #[inline]
    pub fn interp_style(&self) -> InterpStyle {
        self.interp.style()
    }

    /// Extrapolation policy configured for out-of-bounds queries.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.interp.extrapolation()
    }

    /// Exact typed inputs used to bootstrap this curve, when available.
    #[inline]
    #[must_use]
    pub fn rate_calibration(&self) -> Option<&super::super::RateCalibrationRecipe> {
        self.rate_calibration.as_ref()
    }

    /// Opaque FX policy stamp set by calibration, if any.
    #[inline]
    pub fn fx_policy(&self) -> Option<&str> {
        self.fx_policy.as_deref()
    }

    /// Number of visible knot points stored on this curve.
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

    /// Continuously-compounded forward rate between `t1` and `t2`.
    ///
    /// Computes `-ln(DF(t2) / DF(t1)) / (t2 - t1)` using stable log-difference
    /// arithmetic after admitting both endpoint discount factors.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year fraction from the curve base date.
    /// * `t2` - End year fraction from the curve base date; must satisfy `t2 > t1`.
    ///
    /// # Errors
    ///
    /// Returns an error if `t1` or `t2` is non-finite, `t2 <= t1`, either
    /// endpoint discount factor fails domain or tail admission, or the quotient
    /// division overflows or underflows non-equal factors to zero.
    #[inline]
    #[must_use = "computed forward rate should not be discarded"]
    pub fn forward(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t2 <= t1 {
            return Err(crate::Error::Validation(format!(
                "forward interval must be finite with t2 > t1, got [{t1}, {t2}]"
            )));
        }
        let df1 = self.df(t1)?;
        let df2 = self.df(t2)?;
        let dt = t2 - t1;
        if !dt.is_finite() || dt <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "forward interval width must be finite and positive, got {dt}"
            )));
        }
        let neg_log_ratio = stable_positive_log_ratio(df1, df2)?;
        let rate = neg_log_ratio / dt;
        if !rate.is_finite() || (!exact_f64_eq(neg_log_ratio, 0.0) && exact_f64_eq(rate, 0.0)) {
            return Err(crate::Error::Validation(format!(
                "forward rate overflowed or underflowed over [{t1}, {t2}]"
            )));
        }
        Ok(if exact_f64_eq(rate, 0.0) { 0.0 } else { rate })
    }

    /// Original untransformed anchor date for conditioned curves, or `base_date()` otherwise.
    #[inline]
    #[must_use]
    pub fn source_base_date(&self) -> Date {
        self.transform
            .as_ref()
            .map_or(self.base, |transform| transform.source_base_date())
    }

    /// Discount factor on a specific date `date` using the curve's clock.
    ///
    /// Conditioned curves map `date` on [`source_base_date`](Self::source_base_date)
    /// using [`day_count`](Self::day_count) and [`day_count_context`](Self::day_count_context).
    ///
    /// # Arguments
    ///
    /// * `date` - Target calendar date for the discount factor query.
    ///
    /// # Errors
    ///
    /// Returns an error if day-count calculation fails or the factor at the
    /// mapped coordinate cannot be admitted under the curve's interpolation and
    /// tail policy.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_on_date(&self, date: Date) -> crate::Result<f64> {
        eval_factor_df_on_date(
            FactorClock {
                base: self.base,
                day_count: self.day_count,
                calendar: self.calendar.as_ref(),
            },
            &self.interp,
            self.transform.as_ref(),
            date,
            "DiscountCurve",
        )
    }

    /// Zero rate on `date` under the explicit `compounding` convention.
    ///
    /// Requires `date` to map strictly past `base_date` (`t > 0`) on the curve's
    /// original source clock. The horizon is the difference between the source
    /// coordinates of `date` and `base_date`. A nonpositive horizon has no
    /// finite-horizon zero rate.
    ///
    /// # Arguments
    ///
    /// * `date` - Target calendar date strictly after the curve anchor.
    /// * `compounding` - Explicit compounding convention (`Continuous`,
    ///   `Periodic(m)`, or `Simple`).
    ///
    /// # Errors
    ///
    /// Returns an error if day-count mapping fails, `t <= 0`, endpoint
    /// discount-factor admission fails, or inverse rate conversion fails.
    #[inline]
    #[must_use = "computed zero rate should not be discarded"]
    pub fn zero_rate_on_date(&self, date: Date, compounding: Compounding) -> crate::Result<f64> {
        let source_t = signed_curve_year_fraction(
            self.source_base_date(),
            date,
            self.day_count,
            self.calendar.as_ref(),
        )?;
        let t = source_t - self.transform.as_ref().map_or(0.0, |curve| curve.offset());
        if !t.is_finite() || t <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "zero_rate_on_date requires mapped time t > 0, got t={t} for date {date} (base {})",
                self.base
            )));
        }
        let df = match &self.transform {
            Some(transform) => transform.df_at_source_time(&self.interp, source_t)?,
            None => self.df(t)?,
        };
        compounding.rate_from_df(df, t)
    }

    /// Zero rate at time `t > 0` under the explicit `compounding` convention.
    ///
    /// # Arguments
    ///
    /// * `t` - Strictly positive year fraction from [`base_date`](Self::base_date).
    /// * `compounding` - Explicit compounding convention (`Continuous`,
    ///   `Periodic(m)`, or `Simple`).
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite or `t <= 0.0`, if `df(t)` cannot
    /// be admitted, or if inverse rate conversion fails.
    #[inline]
    #[must_use = "computed zero rate should not be discarded"]
    pub fn zero_rate(&self, t: f64, compounding: Compounding) -> crate::Result<f64> {
        if !t.is_finite() || t <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "zero_rate requires finite strictly positive horizon t > 0, got t={t}"
            )));
        }
        let df = self.df(t)?;
        compounding.rate_from_df(df, t)
    }

    /// Day-count context of the curve's time axis: its holiday calendar, if any.
    #[must_use]
    pub fn day_count_context(&self) -> crate::dates::DayCountContext<'static> {
        crate::dates::DayCountContext {
            calendar: self.calendar.as_ref().map(|calendar| calendar.calendar),
            ..crate::dates::DayCountContext::default()
        }
    }

    /// Canonical id of the curve's time-axis holiday calendar, if any.
    #[must_use]
    pub fn calendar_id(&self) -> Option<&str> {
        self.calendar.as_ref().map(|calendar| calendar.id.as_str())
    }

    /// Discount factor at time `t` in years from [`base_date`](Self::base_date).
    ///
    /// # Arguments
    ///
    /// * `t` - Finite curve time coordinate in years.
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite, out of range under
    /// [`ExtrapolationPolicy::Error`], or evaluates to a non-finite or
    /// non-positive value.
    #[must_use = "computed discount factor should not be discarded"]
    #[inline]
    pub fn df(&self, t: f64) -> crate::Result<f64> {
        eval_factor_df(&self.interp, self.transform.as_ref(), t, "DiscountCurve")
    }

    /// Analytic instantaneous forward rate `-D'(t) / D(t)` at time `t`.
    ///
    /// Uses the derivative of the curve's actual evaluator with one-sided
    /// limits at interpolation knots and continuous-shock breakpoints.
    ///
    /// # Arguments
    ///
    /// * `t` - Finite curve time coordinate in years.
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite, factor admission at `t` fails, or
    /// the derivative quotient overflows.
    #[inline]
    #[must_use = "computed instantaneous forward rate should not be discarded"]
    pub fn instantaneous_forward(&self, t: f64) -> crate::Result<f64> {
        eval_factor_instantaneous_forward(&self.interp, self.transform.as_ref(), t, "DiscountCurve")
    }

    /// Knot times (t) in **years** stored on this curve.
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Discount factors corresponding to each knot.
    #[inline]
    pub fn dfs(&self) -> &[f64] {
        &self.dfs
    }

    /// Create a builder for a [`DiscountCurve`] with explicit required choices.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> DiscountCurveBuilder {
        DiscountCurveBuilder {
            id: id.into(),
            base: None,
            day_count: None,
            calendar_id: None,
            points: Vec::new(),
            style: None,
            extrapolation: None,
            rate_calibration: None,
            fx_policy: None,
            transform: None,
        }
    }

    /// Create a builder pre-populated with this curve's data under a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> DiscountCurveBuilder {
        let mut builder = self
            .metadata_builder(new_id)
            .knots(self.knots.iter().copied().zip(self.dfs.iter().copied()));
        builder.transform = self.transform.clone();
        builder
    }

    /// Rebuild this curve with replacement knots while preserving structural metadata.
    ///
    /// # Arguments
    ///
    /// * `knots` - Replacement strictly increasing `(time, discount_factor)` pillars
    ///   in years from [`base_date`](Self::base_date) under [`day_count`](Self::day_count).
    ///
    /// # Errors
    ///
    /// Returns an error when replacement knots violate curve or interpolation invariants.
    pub fn rebuild_with_knots<I>(&self, knots: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.metadata_builder(self.id.clone())
            .rate_calibration_opt(None)
            .knots(knots)
            .build()
    }

    /// Builder pre-populated with this curve's metadata and no knots.
    pub(super) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> DiscountCurveBuilder {
        DiscountCurve::builder(new_id)
            .base_date(self.base)
            .day_count(self.day_count)
            .calendar_id_opt(self.calendar.as_ref().map(|calendar| calendar.id.clone()))
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .rate_calibration_opt(self.rate_calibration.clone())
            .fx_policy_opt(self.fx_policy.clone())
    }

    /// Export the retained evaluation transform snapshot, if present.
    pub(crate) fn transform_snapshot(&self) -> Option<FactorTransformSnapshot> {
        self.transform
            .as_ref()
            .map(super::evaluation::CurveTransform::to_snapshot)
    }
}
