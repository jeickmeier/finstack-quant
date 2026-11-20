//! Forward-curve construction, positive-factor evaluation, and interval rate queries.

use super::super::DiscountCurve;
use super::super::common::factor::{
    dated_points_to_knots, df_between_admitted_factors,
    instantaneous_forward as factor_instantaneous_forward, simple_rate_from_factors,
    zero_rate_points_to_factors,
};
use super::super::common::{signed_curve_year_fraction, validate_curve_clock};
use super::{ForwardCurve, ForwardCurveBuilder};
use crate::dates::{Date, DayCount, Tenor};
use crate::math::Compounding;
use crate::math::interp::{ExtrapolationPolicy, InterpStyle, exact_f64_eq};
use crate::types::CurveId;

impl ForwardCurve {
    /// Create a builder for a [`ForwardCurve`] with explicit required choices.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> ForwardCurveBuilder {
        ForwardCurveBuilder {
            id: id.into(),
            base: None,
            day_count: None,
            calendar_id: None,
            index_tenor: None,
            points: Vec::new(),
            style: None,
            extrapolation: None,
            rate_calibration: None,
            fx_policy: None,
            transform: None,
        }
    }

    /// Construct a flat continuously-compounded projection factor curve.
    ///
    /// Uses a two-knot log-linear representation `(0, 1)` and
    /// `(1, exp(-continuous_rate))` with [`ExtrapolationPolicy::Extend`] on the
    /// supplied `day_count`, preserving `P_x(t) = exp(-continuous_rate * t)`
    /// across all representable times.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"USD-SOFR-3M"`).
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `continuous_rate` - Continuously-compounded source rate as a decimal
    ///   fraction (`0.04` is 4%, `-0.005` is -50 bps).
    /// * `day_count` - Explicit supported curve day-count convention (cannot be
    ///   `ActActIsma`, `Act365L`, `ThirtyE360Isda`, or calendarless `Bus252`).
    ///
    /// # Errors
    ///
    /// Returns an error when `continuous_rate` is non-finite, its 1-year factor
    /// overflows or underflows to zero, or `day_count` is unsupported.
    pub fn flat(
        id: impl Into<CurveId>,
        base_date: Date,
        continuous_rate: f64,
        day_count: DayCount,
    ) -> crate::Result<Self> {
        let one_year_factor = Compounding::Continuous.df_from_rate(continuous_rate, 1.0)?;

        Self::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .knots([(0.0, 1.0), (1.0, one_year_factor)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
    }

    /// Construct a forward curve from strictly ordered zero-rate pillars.
    ///
    /// Converts each `(t, r)` pillar into a positive projection factor under
    /// `compounding`, validating strict ascending order without sorting or
    /// merging and prepending `(0, 1)` when missing.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"EUR-EURIBOR-3M"`).
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `points` - Strictly ascending `(time_years, zero_rate)` pillars with
    ///   finite `time_years >= 0` and decimal zero rates (`0.04` is 4%).
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
        let knots = zero_rate_points_to_factors("ForwardCurve", points, compounding)?;
        Self::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .knots(knots)
            .interp(style)
            .extrapolation(extrapolation)
            .build()
    }

    /// Construct a forward curve from strictly ordered dated factor pillars.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier.
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `points` - Strictly ascending `(date, factor)` pillars with
    ///   `date >= base_date` and finite strictly positive projection factors.
    /// * `day_count` - Explicit supported curve day-count convention.
    /// * `style` - Explicit interpolation strategy.
    /// * `extrapolation` - Explicit out-of-range tail policy.
    ///
    /// # Errors
    ///
    /// Returns an error when `points` is empty, any date precedes `base_date`,
    /// mapped pillar times are not strictly increasing, `day_count` is invalid
    /// or requires an absent calendar, or any factor is non-positive.
    pub fn from_dates(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(Date, f64)],
        day_count: DayCount,
        style: InterpStyle,
        extrapolation: ExtrapolationPolicy,
    ) -> crate::Result<Self> {
        let id: CurveId = id.into();
        let calendar = validate_curve_clock("ForwardCurve", id.as_str(), day_count, None)?;
        let knots = dated_points_to_knots(
            "ForwardCurve",
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

    /// Promote a complete [`DiscountCurve`] evaluator directly into a
    /// projection [`ForwardCurve`] without resampling or refitting pillars.
    ///
    /// Preserves the complete source interpolator, day-count clock, holiday
    /// calendar, tail policy, FX policy stamp, and any retained conditioning or
    /// continuous shock transform while resetting `rate_calibration` to `None`.
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier for the resulting forward projection curve.
    /// * `discount` - Source [`DiscountCurve`] whose positive-factor evaluator
    ///   is cloned losslessly.
    /// * `index_tenor` - Optional index accrual tenor metadata associated with
    ///   the projection role.
    #[must_use]
    pub fn from_discount_curve(
        id: impl Into<CurveId>,
        discount: &DiscountCurve,
        index_tenor: Option<Tenor>,
    ) -> Self {
        Self {
            id: id.into(),
            base: discount.base,
            day_count: discount.day_count,
            calendar: discount.calendar.clone(),
            index_tenor,
            knots: discount.knots.clone(),
            factors: discount.dfs.clone(),
            interp: discount.interp.clone(),
            transform: discount
                .transform_snapshot()
                .map(super::evaluation::CurveTransform::from_snapshot),
            rate_calibration: None,
            fx_policy: discount.fx_policy.clone(),
        }
    }

    /// Unique identifier of the curve.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Valuation **base date** anchoring `t = 0`.
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Original untransformed anchor date for conditioned curves, or `base_date()` otherwise.
    #[inline]
    #[must_use]
    pub fn source_base_date(&self) -> Date {
        self.transform
            .as_ref()
            .map_or(self.base, |transform| transform.source_base_date())
    }

    /// Day-count basis used for projection factor time mapping.
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Canonical id of the curve's time-axis holiday calendar, if any.
    #[must_use]
    pub fn calendar_id(&self) -> Option<&str> {
        self.calendar.as_ref().map(|calendar| calendar.id.as_str())
    }

    /// Day-count context of the curve's time axis: its holiday calendar, if any.
    #[must_use]
    pub fn day_count_context(&self) -> crate::dates::DayCountContext<'static> {
        crate::dates::DayCountContext {
            calendar: self.calendar.as_ref().map(|calendar| calendar.calendar),
            ..crate::dates::DayCountContext::default()
        }
    }

    /// Optional contractual index accrual tenor metadata associated with this curve.
    #[inline]
    pub fn index_tenor(&self) -> Option<Tenor> {
        self.index_tenor
    }

    /// Knot times in **years** from [`base_date`](Self::base_date).
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Normalized positive projection factors at each knot.
    #[inline]
    pub fn factors(&self) -> &[f64] {
        &self.factors
    }

    /// Paired `(time, factor)` pillars stored on this curve.
    #[must_use]
    pub fn knot_points(&self) -> Vec<(f64, f64)> {
        self.knots
            .iter()
            .copied()
            .zip(self.factors.iter().copied())
            .collect()
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

    /// Exact typed recipe used to bootstrap this curve, when available.
    #[inline]
    #[must_use]
    pub fn rate_calibration(
        &self,
    ) -> Option<&crate::market_data::term_structures::RateCalibrationRecipe> {
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

    /// Projection factor `P_x(t)` at time `t` in years from [`base_date`](Self::base_date).
    ///
    /// Supports negative finite query coordinates `t < 0` according to the
    /// configured [`ExtrapolationPolicy`] (`Flat`, `Extend`, or `Error`).
    ///
    /// # Arguments
    ///
    /// * `t` - Finite curve time coordinate in years.
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite, out of bounds under
    /// [`ExtrapolationPolicy::Error`], or evaluates to a non-finite or
    /// non-positive value.
    #[must_use = "computed projection factor should not be discarded"]
    #[inline]
    pub fn df(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::df requires finite time t, got {t}"
            )));
        }
        if let Some(transform) = &self.transform {
            return transform.df(&self.interp, t);
        }
        let val = self.interp.interp(t)?;
        if !val.is_finite() || val <= 0.0 {
            return Err(crate::InputError::NonPositiveValue.into());
        }
        Ok(val)
    }

    /// Projection factor on a specific date using the curve's signed clock.
    ///
    /// Conditioned curves map `date` on [`source_base_date`](Self::source_base_date)
    /// using [`day_count`](Self::day_count) and [`day_count_context`](Self::day_count_context).
    ///
    /// # Arguments
    ///
    /// * `date` - Target calendar date for the projection factor query.
    ///
    /// # Errors
    ///
    /// Returns an error if day-count calculation fails or the factor at the
    /// mapped coordinate cannot be admitted under the curve's tail policy.
    #[inline]
    #[must_use = "computed projection factor should not be discarded"]
    pub fn df_on_date(&self, date: Date) -> crate::Result<f64> {
        if let Some(transform) = &self.transform {
            let source_t = signed_curve_year_fraction(
                transform.source_base_date(),
                date,
                self.day_count,
                self.calendar.as_ref(),
            )?;
            return transform.df_at_source_time(&self.interp, source_t);
        }
        let t =
            signed_curve_year_fraction(self.base, date, self.day_count, self.calendar.as_ref())?;
        self.df(t)
    }

    /// Relative projection factor `P_x(to) / P_x(from)` between two calendar dates.
    ///
    /// Admits both endpoint factors before evaluating the quotient.
    ///
    /// # Arguments
    ///
    /// * `from` - Start calendar date.
    /// * `to` - End calendar date.
    ///
    /// # Errors
    ///
    /// Returns an error if either endpoint factor cannot be admitted or the
    /// relative factor overflows or underflows to zero.
    #[inline]
    #[must_use = "computed relative projection factor should not be discarded"]
    pub fn df_between_dates(&self, from: Date, to: Date) -> crate::Result<f64> {
        let p_from = self.df_on_date(from)?;
        let p_to = self.df_on_date(to)?;
        df_between_admitted_factors(p_from, p_to, from, to)
    }

    /// Simple forward rate over curve-time interval `[t1, t2]` annualized on
    /// explicit contractual accrual year fraction `accrual_year_fraction`.
    ///
    /// Evaluates `(exp(ln(P_x(t1) / P_x(t2))) - 1) / accrual_year_fraction`
    /// using stable positive-factor log-ratio arithmetic after admitting both
    /// endpoint factors.
    ///
    /// # Arguments
    ///
    /// * `t1` - Finite start year fraction from [`base_date`](Self::base_date).
    /// * `t2` - Finite end year fraction strictly greater than `t1`.
    /// * `accrual_year_fraction` - Explicit strictly positive finite accrual
    ///   year fraction of the index interval (must not depend on the curve
    ///   clock when the index convention differs).
    ///
    /// # Errors
    ///
    /// Returns an error when `t1` or `t2` is non-finite, `t2 <= t1`,
    /// `accrual_year_fraction` is non-finite or `<= 0`, either endpoint factor
    /// fails admission, or the rate overflows, underflows non-equal factors to
    /// zero, or hits the non-positive simple gross domain.
    #[must_use = "computed forward rate should not be discarded"]
    pub fn rate_between(&self, t1: f64, t2: f64, accrual_year_fraction: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t2 <= t1 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::rate_between requires finite t1 < t2, got [{t1}, {t2}]"
            )));
        }
        let p1 = self.df(t1)?;
        let p2 = self.df(t2)?;
        simple_rate_from_factors(p1, p2, accrual_year_fraction, "ForwardCurve::rate_between")
    }

    /// Simple forward rate over date interval `[start, end]` annualized on
    /// explicit contractual accrual year fraction `accrual_year_fraction`.
    ///
    /// Both dates are mapped on the curve's signed clock from
    /// [`source_base_date`](Self::source_base_date) without clamping pre-base
    /// resets to zero (allowing spot-based curves to project pre-spot resets
    /// under their tail policy). Both endpoint factors are admitted before any
    /// equal-coordinate shortcut; distinct dates that map to the same curve
    /// coordinate (such as Mar 30 to Mar 31 under `30/360`) return `0.0` once
    /// factor admission succeeds.
    ///
    /// # Arguments
    ///
    /// * `start` - Period start date (`start < end`).
    /// * `end` - Exclusive period end date strictly after `start`.
    /// * `accrual_year_fraction` - Explicit strictly positive finite year
    ///   fraction for `[start, end]` under the contractual index convention.
    ///
    /// # Errors
    ///
    /// Returns an error when `end <= start`, mapped coordinates reverse order,
    /// `accrual_year_fraction` is non-finite or `<= 0`, either endpoint factor
    /// cannot be admitted under the curve's clock/tail policy, or simple-rate
    /// computation overflows, underflows to zero, or hits the non-positive
    /// gross domain.
    #[must_use = "computed forward rate should not be discarded"]
    pub fn rate_between_dates(
        &self,
        start: Date,
        end: Date,
        accrual_year_fraction: f64,
    ) -> crate::Result<f64> {
        if end <= start {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::rate_between_dates requires end > start; got start={start}, end={end}"
            )));
        }
        if !accrual_year_fraction.is_finite() || accrual_year_fraction <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::rate_between_dates requires finite positive accrual_year_fraction, got {accrual_year_fraction}"
            )));
        }
        let anchor_base = self.source_base_date();
        let t_start =
            signed_curve_year_fraction(anchor_base, start, self.day_count, self.calendar.as_ref())?;
        let t_end =
            signed_curve_year_fraction(anchor_base, end, self.day_count, self.calendar.as_ref())?;
        if !t_start.is_finite() || !t_end.is_finite() || t_end < t_start {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::rate_between_dates requires finite non-decreasing mapped coordinates for [{start}, {end}], got [{t_start}, {t_end}]"
            )));
        }
        let (p1, p2) = if let Some(transform) = &self.transform {
            (
                transform.df_at_source_time(&self.interp, t_start)?,
                transform.df_at_source_time(&self.interp, t_end)?,
            )
        } else {
            (self.df(t_start)?, self.df(t_end)?)
        };
        if exact_f64_eq(t_start, t_end) {
            return Ok(0.0);
        }
        simple_rate_from_factors(
            p1,
            p2,
            accrual_year_fraction,
            "ForwardCurve::rate_between_dates",
        )
    }

    /// Analytic instantaneous projection forward `-P_x'(t) / P_x(t)` at time `t`.
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
        if let Some(transform) = &self.transform {
            return transform.instantaneous_forward(&self.interp, t);
        }
        factor_instantaneous_forward(&self.interp, None, t, "ForwardCurve")
    }
}
