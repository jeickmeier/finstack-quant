//! Fluent forward-curve builder and construction validation.

use super::super::common::factor::{FactorBuildInput, build_factor_curve};
use super::ForwardCurve;
use crate::dates::{Date, DayCount, Tenor};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::types::CurveId;

/// Fluent builder for [`ForwardCurve`].
///
/// Requires explicit `base_date`, `day_count` (plus `calendar_id` for `Bus/252`),
/// `knots`, `interp`, and `extrapolation` before calling [`ForwardCurveBuilder::build`].
///
/// # Examples
/// ```rust
/// use finstack_quant_core::dates::{Date, DayCount};
/// use finstack_quant_core::market_data::term_structures::ForwardCurve;
/// use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = ForwardCurve::builder("USD-SOFR-3M")
///     .base_date(base)
///     .day_count(DayCount::Act360)
///     .knots([(0.0, 1.0), (5.0, 0.82)])
///     .interp(InterpStyle::LogLinear)
///     .extrapolation(ExtrapolationPolicy::Extend)
///     .build()
///     .expect("ForwardCurve builder should succeed");
/// assert!(curve.df(2.0).expect("valid factor") < 1.0);
/// ```
pub struct ForwardCurveBuilder {
    pub(super) id: CurveId,
    pub(super) base: Option<Date>,
    pub(super) day_count: Option<DayCount>,
    pub(super) calendar_id: Option<String>,
    pub(super) index_tenor: Option<Tenor>,
    pub(super) points: Vec<(f64, f64)>,
    pub(super) style: Option<InterpStyle>,
    pub(super) extrapolation: Option<ExtrapolationPolicy>,
    pub(super) rate_calibration: Option<super::super::RateCalibrationRecipe>,
    pub(super) fx_policy: Option<String>,
    pub(super) transform: Option<super::evaluation::CurveTransform>,
}

impl ForwardCurveBuilder {
    /// Set the **base date** (valuation anchor date).
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = Some(d);
        self
    }

    /// Choose the day-count basis for projection factor time mapping.
    ///
    /// # Arguments
    ///
    /// * `day_count` - Supported complete day-count convention used to convert
    ///   calendar dates into curve year fractions.
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = Some(day_count);
        self
    }

    /// Set the registered holiday calendar of the curve's time axis.
    ///
    /// Required for business-day day counts such as `Bus/252`.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Registered calendar id, or a `+`-joined union such as
    ///   `"usny+target2"`; resolved and canonicalized at [`Self::build`].
    pub fn calendar_id(mut self, calendar_id: impl Into<String>) -> Self {
        self.calendar_id = Some(calendar_id.into());
        self
    }

    /// Optionally set the time-axis holiday calendar; `None` clears it.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Registered calendar id, or `None` for a day count that
    ///   needs no calendar.
    pub fn calendar_id_opt(mut self, calendar_id: Option<String>) -> Self {
        self.calendar_id = calendar_id;
        self
    }

    /// Associate an explicit index accrual tenor metadata value with this curve.
    ///
    /// # Arguments
    ///
    /// * `tenor` - Contractual index tenor (for example [`Tenor::quarterly`]).
    pub fn index_tenor(mut self, tenor: Tenor) -> Self {
        self.index_tenor = Some(tenor);
        self
    }

    /// Optionally associate an explicit index accrual tenor; `None` clears it.
    ///
    /// # Arguments
    ///
    /// * `tenor` - Optional index tenor metadata.
    pub fn index_tenor_opt(mut self, tenor: Option<Tenor>) -> Self {
        self.index_tenor = tenor;
        self
    }

    /// Supply strictly ordered knot points `(t, factor)` where `t >= 0` is the
    /// year fraction and `factor > 0` is the normalized projection factor.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }

    /// Select interpolation style for this curve.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Set the extrapolation policy for out-of-bounds evaluation.
    pub fn extrapolation(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation = Some(policy);
        self
    }

    /// Attach the typed calibration recipe used to bootstrap this curve.
    pub fn rate_calibration(mut self, calibration: super::super::RateCalibrationRecipe) -> Self {
        self.rate_calibration = Some(calibration);
        self
    }

    /// Optionally attach the typed calibration recipe used to bootstrap this curve.
    pub fn rate_calibration_opt(
        mut self,
        calibration: Option<super::super::RateCalibrationRecipe>,
    ) -> Self {
        self.rate_calibration = calibration;
        self
    }

    /// Stamp an opaque FX policy on the curve.
    pub fn fx_policy(mut self, policy: impl Into<String>) -> Self {
        self.fx_policy = Some(policy.into());
        self
    }

    /// Optionally stamp an FX policy. `None` leaves the field unset.
    pub fn fx_policy_opt(mut self, policy: Option<String>) -> Self {
        self.fx_policy = policy;
        self
    }

    /// Validate input and create the [`ForwardCurve`].
    ///
    /// When the first supplied knot has `t > 0.0`, prepends `(0.0, 1.0)` as
    /// normalization anchor; when `t = 0.0` is supplied, requires exact
    /// `factor == 1.0`. At least one positive-time pillar is required.
    ///
    /// # Errors
    ///
    /// Returns an error when any required field (`base_date`, `day_count`,
    /// `interp`, `extrapolation`) is missing, the day-count clock is unsupported
    /// or lacks its `Bus/252` calendar, pillars are not strictly increasing or
    /// contain non-finite / non-positive factors, or fewer than two points remain
    /// after normalization.
    pub fn build(self) -> crate::Result<ForwardCurve> {
        let built = build_factor_curve(
            "ForwardCurve",
            FactorBuildInput {
                id: self.id.as_str(),
                base: self.base,
                day_count: self.day_count,
                calendar_id: self.calendar_id.as_deref(),
                points: self.points,
                style: self.style,
                extrapolation: self.extrapolation,
                transform: self.transform,
            },
        )?;

        Ok(ForwardCurve {
            id: self.id,
            base: built.base,
            day_count: built.day_count,
            calendar: built.calendar,
            index_tenor: self.index_tenor,
            knots: built.knots,
            factors: built.values,
            interp: built.interp,
            transform: built.transform,
            rate_calibration: self.rate_calibration,
            fx_policy: self.fx_policy,
        })
    }
}
