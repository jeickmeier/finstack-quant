//! Fluent discount-curve builder and construction validation.

use super::validation::{validate_forward_rates, validate_monotonic_df};
use super::{DiscountCurve, ValidationMode};
use crate::dates::{Date, DayCount};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle, ValidationPolicy};
use crate::types::CurveId;

/// Fluent builder for [`DiscountCurve`].
///
/// Typical usage chains `base_date`, `knots`, and `interp` (optional)
/// before calling [`DiscountCurveBuilder::build`].
///
/// # Examples
/// ```rust
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use finstack_quant_core::math::interp::InterpStyle;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = DiscountCurve::builder("USD-OIS")
///     .base_date(base)
///     .knots([(0.0, 1.0), (5.0, 0.9)])
///     .interp(InterpStyle::Linear)
///     .build()
///     .expect("DiscountCurve builder should succeed");
/// assert!(curve.df(2.0) < 1.0);
/// ```
pub struct DiscountCurveBuilder {
    pub(super) id: CurveId,
    /// Valuation / base date. `None` until [`Self::base_date`] is called;
    /// [`Self::build`] requires `Some(_)` and errors on `None`.
    pub(super) base: Option<Date>,
    pub(super) day_count: DayCount,
    pub(super) calendar_id: Option<String>,
    pub(super) points: Vec<(f64, f64)>, // (t, df)
    pub(super) style: InterpStyle,
    pub(super) extrapolation: ExtrapolationPolicy,
    pub(super) min_forward_rate: Option<f64>,
    pub(super) allow_non_monotonic: bool,
    pub(super) min_forward_tenor: f64,
    pub(super) rate_calibration: Option<super::super::RateCalibrationRecipe>,
    pub(super) fx_policy: Option<String>,
    pub(super) transform: Option<super::evaluation::CurveTransform>,
}

impl DiscountCurveBuilder {
    /// Override the default **base date** (valuation date).
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = Some(d);
        self
    }
    /// Choose the day-count basis for discount time mapping.
    ///
    /// # Arguments
    ///
    /// * `day_count` - Day-count convention used to convert calendar dates into year fractions.
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }

    /// Set the registered holiday calendar of the curve's time axis.
    ///
    /// Required for business-day day counts such as Bus/252, where the year
    /// fraction from the base date counts business days. The calendar belongs
    /// to the curve (for example `"brbd"` for a BRL CDI curve), so every
    /// date-based query uses it.
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
    ///   needs no calendar. Used by the serde round-trip path.
    pub fn calendar_id_opt(mut self, calendar_id: Option<String>) -> Self {
        self.calendar_id = calendar_id;
        self
    }

    /// Supply knot points `(t, df)` where *t* is the year fraction and *df*
    /// the discount factor.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }
    /// Select interpolation style for this curve.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the extrapolation policy for out-of-bounds evaluation.
    pub fn extrapolation(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation = policy;
        self
    }

    /// Select the validation policy for the curve.
    ///
    /// # Arguments
    ///
    /// * `mode` - Execution or interpolation mode selecting the documented algorithm branch.
    pub fn validation(mut self, mode: ValidationMode) -> Self {
        match mode {
            ValidationMode::MarketStandard => {
                self.allow_non_monotonic = false;
                self.min_forward_rate = Some(-0.005);
            }
            ValidationMode::NegativeRateFriendly { forward_floor } => {
                self.allow_non_monotonic = true;
                self.min_forward_rate = Some(forward_floor);
            }
            ValidationMode::Raw {
                allow_non_monotonic,
                forward_floor,
            } => {
                self.allow_non_monotonic = allow_non_monotonic;
                self.min_forward_rate = forward_floor;
            }
        }
        self
    }

    /// Set a custom minimum tenor for forward rate calculations.
    ///
    /// The forward rate calculation `f(t1, t2) = (z2*t2 - z1*t1) / (t2 - t1)` suffers
    /// from catastrophic cancellation when `(t2 - t1)` is very small. This threshold
    /// prevents such precision issues.
    ///
    /// # Default
    ///
    /// The default value is `1e-6` years (~30 seconds).
    ///
    /// # Use Cases
    ///
    /// - Set to a smaller value (e.g., `1e-8`) for high-frequency intraday operations
    /// - Set to a larger value (e.g., `1e-4`) for daily curve operations with coarse data
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// # use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let curve = DiscountCurve::builder("USD")
    ///     .base_date(date!(2025-01-01))
    ///     .knots([(0.0, 1.0), (1.0, 0.95)])
    ///     .min_forward_tenor(1e-8)  // Allow sub-second tenors
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `tenor` - Market tenor label or period length used to locate the quote or rate
    pub fn min_forward_tenor(mut self, tenor: f64) -> Self {
        self.min_forward_tenor = tenor;
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
    ///
    /// Use when the bootstrap involved an FX-sensitive assumption (XCCY basis
    /// adjustment, FX matrix triangulation, etc.) and the policy must be
    /// surfaced on downstream valuation result envelopes.
    pub fn fx_policy(mut self, policy: impl Into<String>) -> Self {
        self.fx_policy = Some(policy.into());
        self
    }

    /// Optionally stamp an FX policy. `None` is a no-op (the field stays
    /// unset). Used by the serde round-trip path.
    pub fn fx_policy_opt(mut self, policy: Option<String>) -> Self {
        self.fx_policy = policy;
        self
    }

    /// Validate input and create the [`DiscountCurve`].
    ///
    /// If the first knot time is `> 0.0`, automatically prepends `(0.0, 1.0)` to
    /// ensure the round-trip invariant `DF(0) = 1.0` (ISDA/QuantLib standard).
    /// An explicitly supplied zero-time discount factor must equal one, and
    /// negative maturity times are rejected.
    ///
    /// # Errors
    ///
    /// Returns an error when the base date is missing, fewer than two knots are
    /// supplied after zero-time anchoring, a discount factor is non-positive,
    /// knots are invalid, the configured monotonicity, forward-rate, or
    /// interpolation constraints are violated, the calendar id is unknown, or a
    /// Bus/252 curve has no calendar.
    pub fn build(mut self) -> crate::Result<DiscountCurve> {
        let base = self.base.ok_or_else(|| {
            crate::Error::Validation("DiscountCurve requires base_date".to_string())
        })?;
        if !self.points.is_empty() {
            self.points.sort_by(|a, b| a.0.total_cmp(&b.0));
            let first_t = self.points[0].0;
            if first_t > 0.0 {
                self.points.insert(0, (0.0, 1.0));
            }
        }

        if self.points.len() < 2 {
            return Err(crate::InputError::TooFewPoints.into());
        }
        if self.points.iter().any(|&(_, df)| df <= 0.0) {
            return Err(crate::InputError::NonPositiveValue.into());
        }
        validate_origin(&self.points)?;

        let (knots_vec, dfs_vec): (Vec<f64>, Vec<f64>) =
            std::mem::take(&mut self.points).into_iter().unzip();
        crate::math::interp::validate_knots(&knots_vec)?;

        if !self.allow_non_monotonic {
            validate_monotonic_df(&knots_vec, &dfs_vec)?;
        }

        if let Some(min_fwd) = self.min_forward_rate {
            validate_forward_rates(&knots_vec, &dfs_vec, min_fwd)?;
        }

        self.finish(base, knots_vec, dfs_vec)
    }

    fn finish(self, base: Date, knots: Vec<f64>, dfs: Vec<f64>) -> crate::Result<DiscountCurve> {
        let calendar = self
            .calendar_id
            .as_deref()
            .map(super::CurveCalendar::resolve)
            .transpose()?;
        if self.day_count == DayCount::Bus252 && calendar.is_none() {
            return Err(crate::Error::Validation(format!(
                "Bus/252 discount curve '{}' requires a holiday calendar",
                self.id
            )));
        }
        let knots = knots.into_boxed_slice();
        let dfs = dfs.into_boxed_slice();

        let interp = self.style.build_enum(
            knots.clone(),
            dfs.clone(),
            self.extrapolation,
            ValidationPolicy::AllowNegative,
        )?;

        let mut curve = DiscountCurve {
            id: self.id,
            base,
            day_count: self.day_count,
            calendar,
            knots,
            dfs,
            interp,
            transform: None,
            min_forward_rate: self.min_forward_rate,
            allow_non_monotonic: self.allow_non_monotonic,
            min_forward_tenor: self.min_forward_tenor,
            rate_calibration: self.rate_calibration,
            fx_policy: self.fx_policy,
        };
        if let Some(transform) = self.transform {
            curve.restore_transform(transform)?;
        }
        Ok(curve)
    }
}

fn validate_origin(points: &[(f64, f64)]) -> crate::Result<()> {
    if points
        .iter()
        .any(|&(time, _)| !time.is_finite() || time < 0.0)
    {
        return Err(crate::Error::Validation(
            "discount-curve times must be finite and non-negative".into(),
        ));
    }
    if points
        .first()
        .is_none_or(|&(time, df)| time != 0.0 || !df.total_cmp(&1.0).is_eq())
    {
        return Err(crate::Error::Validation(
            "discount curve must start at (time = 0, discount factor = 1)".into(),
        ));
    }
    Ok(())
}
