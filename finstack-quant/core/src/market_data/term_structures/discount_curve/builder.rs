//! Fluent discount-curve builder and construction validation.

use super::super::common::{build_interp_input_error, split_points};
use super::validation::{validate_forward_rates, validate_monotonic_df};
use super::{DiscountCurve, ValidationMode};
use crate::dates::{Date, DayCount};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
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
    pub(super) points: Vec<(f64, f64)>, // (t, df)
    pub(super) style: InterpStyle,
    pub(super) extrapolation: ExtrapolationPolicy,
    pub(super) min_forward_rate: Option<f64>,
    pub(super) allow_non_monotonic: bool,
    pub(super) min_forward_tenor: f64,
    pub(super) rate_calibration: Option<super::super::RateCalibrationRecipe>,
    pub(super) calibration_ois_cutoff_days: Option<i32>,
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
    /// The default value is [`DEFAULT_MIN_FORWARD_TENOR`](crate::market_data::term_structures::DEFAULT_MIN_FORWARD_TENOR)
    /// (~30 seconds or 1e-6 years).
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

    /// Record the OIS rate cut-off (business days) this curve was calibrated
    /// under. Pass `Some(days)` only when the bootstrap used a
    /// `CompoundedWithRateCutoff` convention; leave unset otherwise.
    pub fn calibration_ois_cutoff_days_opt(mut self, cutoff_days: Option<i32>) -> Self {
        self.calibration_ois_cutoff_days = cutoff_days;
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

    pub(super) fn apply_non_monotonic_settings(
        mut self,
        allow_non_monotonic: bool,
        min_forward_rate: Option<f64>,
    ) -> Self {
        self.allow_non_monotonic = allow_non_monotonic;
        self.min_forward_rate = min_forward_rate;
        self
    }

    /// Build the curve with minimal validation for solver use.
    ///
    /// This method skips monotonicity validation and forward rate checks, providing
    /// faster curve construction for iterative solving where the curve is temporary.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - At least 2 knot points are provided
    /// - All discount factors are positive
    /// - Knots are sorted in ascending order
    ///
    /// This is an internal optimization for calibration solvers.
    /// For general use, prefer [`Self::build`] which includes full validation.
    #[doc(hidden)]
    pub fn build_for_solver(mut self) -> crate::Result<DiscountCurve> {
        let base = self.base.ok_or(crate::error::InputError::Invalid)?;
        if self.points.first().is_some_and(|&(time, _)| time > 0.0) {
            self.points.insert(0, (0.0, 1.0));
        }
        if self.points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        if self.points.iter().any(|&(_, df)| df <= 0.0) {
            return Err(crate::error::InputError::NonPositiveValue.into());
        }

        validate_origin(&self.points)?;

        let (knots, dfs) = split_points(std::mem::take(&mut self.points));
        self.finish(base, knots, dfs)
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
    /// knots are invalid, or the configured monotonicity, forward-rate, or
    /// interpolation constraints are violated.
    pub fn build(mut self) -> crate::Result<DiscountCurve> {
        let base = self.base.ok_or(crate::error::InputError::Invalid)?;
        if !self.points.is_empty() {
            self.points.sort_by(|a, b| a.0.total_cmp(&b.0));
            let first_t = self.points[0].0;
            if first_t > 0.0 {
                self.points.insert(0, (0.0, 1.0));
            }
        }

        if self.points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }
        if self.points.iter().any(|&(_, df)| df <= 0.0) {
            return Err(crate::error::InputError::NonPositiveValue.into());
        }
        validate_origin(&self.points)?;

        let (knots_vec, dfs_vec): (Vec<f64>, Vec<f64>) =
            split_points(std::mem::take(&mut self.points));
        crate::math::interp::utils::validate_knots(&knots_vec)?;

        if !self.allow_non_monotonic {
            validate_monotonic_df(&knots_vec, &dfs_vec)?;
        }

        if let Some(min_fwd) = self.min_forward_rate {
            validate_forward_rates(&knots_vec, &dfs_vec, min_fwd)?;
        }

        self.finish(base, knots_vec, dfs_vec)
    }

    fn finish(self, base: Date, knots: Vec<f64>, dfs: Vec<f64>) -> crate::Result<DiscountCurve> {
        let knots = knots.into_boxed_slice();
        let dfs = dfs.into_boxed_slice();

        let interp = build_interp_input_error(
            self.style,
            knots.clone(),
            dfs.clone(),
            self.extrapolation,
            true,
        )?;

        let mut curve = DiscountCurve {
            id: self.id,
            base,
            day_count: self.day_count,
            knots,
            dfs,
            interp,
            transform: None,
            style: self.style,
            extrapolation: self.extrapolation,
            min_forward_rate: self.min_forward_rate,
            allow_non_monotonic: self.allow_non_monotonic,
            min_forward_tenor: self.min_forward_tenor,
            rate_calibration: self.rate_calibration,
            calibration_ois_cutoff_days: self.calibration_ois_cutoff_days,
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
