//! Validated construction.

use super::*;

/// Fluent builder for [`ForwardCurve`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::term_structures::ForwardCurve;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = ForwardCurve::builder("USD_SOFR_3M", 0.25)
///     .base_date(base)
///     .knots([(1.0, 0.045), (2.0, 0.048), (5.0, 0.050)])
///     .build()
///     .expect("ForwardCurve builder should succeed");
/// assert!(curve.rate(2.0) > 0.0);
/// ```
pub struct ForwardCurveBuilder {
    pub(super) id: CurveId,
    pub(super) base: Date,
    pub(super) base_is_set: bool,
    pub(super) reset_lag: i32,
    pub(super) day_count: DayCount,
    pub(super) tenor: f64,
    pub(super) points: Vec<(f64, f64)>,
    pub(super) projection_grid: Option<Vec<f64>>,
    pub(super) style: InterpStyle,
    pub(super) min_forward_rate: Option<f64>,
    pub(super) extrapolation: ExtrapolationPolicy,
    pub(super) rate_calibration: Option<crate::market_data::term_structures::RateCalibrationRecipe>,
    pub(super) fx_policy: Option<String>,
}

impl ForwardCurveBuilder {
    /// Set the curve’s valuation **base date**.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = d;
        self.base_is_set = true;
        self
    }
    /// Override the **reset lag** (fixing → spot) in business days.
    ///
    /// # Arguments
    ///
    /// * `lag` - Non-negative fixing or publication lag under the documented tenor convention.
    pub fn reset_lag(mut self, lag: i32) -> Self {
        self.reset_lag = lag;
        self
    }

    /// Choose the **day-count** convention.
    ///
    /// # Arguments
    ///
    /// * `day_count` - Day-count convention used to convert calendar dates into year fractions.
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }
    /// Supply knot points `(t, fwd)`.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }

    /// Set contractual reset/end-date boundaries for projection DF chaining.
    pub fn projection_grid<I>(mut self, projection_grid: I) -> Self
    where
        I: IntoIterator<Item = f64>,
    {
        self.projection_grid = Some(projection_grid.into_iter().collect());
        self
    }

    /// Optionally set contractual reset/end-date projection boundaries.
    pub fn projection_grid_opt(mut self, projection_grid: Option<Vec<f64>>) -> Self {
        self.projection_grid = projection_grid;
        self
    }
    /// Select interpolation style for this forward curve.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the extrapolation policy for out-of-bounds evaluation.
    pub fn extrapolation(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation = policy;
        self
    }

    /// Enforce a minimum forward rate across the provided knot points.
    ///
    /// # Arguments
    ///
    /// * `min_rate` - Minimum permitted rate in decimal units.
    pub fn min_forward_rate(mut self, min_rate: f64) -> Self {
        self.min_forward_rate = Some(min_rate);
        self
    }

    /// Attach the typed calibration recipe used to bootstrap this curve.
    pub fn rate_calibration(
        mut self,
        calibration: crate::market_data::term_structures::RateCalibrationRecipe,
    ) -> Self {
        self.rate_calibration = Some(calibration);
        self
    }

    /// Optionally attach the typed calibration recipe used to bootstrap this curve.
    pub fn rate_calibration_opt(
        mut self,
        calibration: Option<crate::market_data::term_structures::RateCalibrationRecipe>,
    ) -> Self {
        self.rate_calibration = calibration;
        self
    }

    /// Stamp an opaque FX policy on the curve. See [`ForwardCurve::fx_policy`].
    pub fn fx_policy(mut self, policy: impl Into<String>) -> Self {
        self.fx_policy = Some(policy.into());
        self
    }

    /// Optionally stamp an FX policy; `None` is a no-op. Used by serde
    /// round-trip and curve builders that propagate metadata.
    pub fn fx_policy_opt(mut self, policy: Option<String>) -> Self {
        self.fx_policy = policy;
        self
    }

    /// Validate input and build the [`ForwardCurve`].
    ///
    /// The curve requires an explicit base date, a strictly positive finite
    /// index tenor, a non-negative reset lag, and at least two strictly
    /// increasing finite knot times. Forward rates may be negative to support
    /// negative-rate markets. Set [`Self::min_forward_rate`] when a particular
    /// application needs a lower bound.
    ///
    /// If supplied, `projection_grid` represents contractual reset/end
    /// boundaries for projection-discount-factor chaining. It must start at
    /// zero, be finite, non-negative, strictly increasing, contain at least
    /// two boundaries, and reach at least the last interpolation knot.
    ///
    /// # Errors
    ///
    /// Returns an error if any of those structural conditions fails, a forward
    /// rate is below the optional minimum, the projection grid is invalid, or
    /// the requested interpolation style cannot be built from the knots and
    /// forward values.
    pub fn build(self) -> crate::Result<ForwardCurve> {
        if !self.base_is_set {
            return Err(InputError::Invalid.into());
        }
        if !self.tenor.is_finite() || self.tenor <= 0.0 {
            return Err(InputError::Invalid.into());
        }
        if self.reset_lag < 0 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve reset_lag must be non-negative business days; got {}",
                self.reset_lag
            )));
        }
        if self.points.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }
        let (kvec, fvec): (Vec<f64>, Vec<f64>) = split_points(self.points);
        crate::math::interp::utils::validate_knots(&kvec)?;
        if let Some(min_fwd) = self.min_forward_rate {
            for (i, &f) in fvec.iter().enumerate() {
                if f < min_fwd {
                    return Err(crate::Error::Validation(format!(
                        "Forward rate below minimum at t={:.6}: fwd={:.8} < min={:.8} (index {})",
                        kvec[i], f, min_fwd, i
                    )));
                }
            }
        }
        let projection_grid = self
            .projection_grid
            .map(|grid| {
                let last_knot = *kvec.last().ok_or(InputError::TooFewPoints)?;
                if grid.len() < 2
                    || grid.iter().any(|time| !time.is_finite() || *time < 0.0)
                    || grid[0].abs() > 1e-12
                    || grid.windows(2).any(|window| window[1] <= window[0])
                    || grid.last().is_none_or(|last| *last < last_knot)
                {
                    return Err(crate::Error::Validation(format!(
                        "ForwardCurve projection_grid must start at 0, be finite, non-negative, strictly increasing, and cover the last interpolation knot ({last_knot})"
                    )));
                }
                Ok(grid.into_boxed_slice())
            })
            .transpose()?;
        let knots = kvec.into_boxed_slice();
        let forwards = fvec.into_boxed_slice();
        // Use allow_any_values to support negative forward rates
        // (common in EUR, CHF, JPY markets since 2014)
        let interp = build_interp_allow_any_values(
            self.style,
            knots.clone(),
            forwards.clone(),
            self.extrapolation,
        )?;
        Ok(ForwardCurve {
            id: self.id,
            base: self.base,
            reset_lag: self.reset_lag,
            day_count: self.day_count,
            tenor: self.tenor,
            knots,
            forwards,
            projection_grid,
            interp,
            rate_calibration: self.rate_calibration,
            fx_policy: self.fx_policy,
        })
    }
}
