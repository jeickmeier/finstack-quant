//! Curve evaluation and accessors.

use super::*;

impl ForwardCurve {
    /// Start building a forward curve for `id` with tenor `tenor_years`.
    ///
    /// **Defaults:** The builder infers day-count and reset-lag conventions from
    /// the curve ID when possible, then uses Linear interpolation with FlatForward
    /// extrapolation.
    ///
    /// **Build-vs-query basis trap:** the inferred day-count converts dated
    /// pillars to year fractions at build time and query dates back at lookup
    /// time. Because inference is substring-based, renaming the curve ID can
    /// silently change the basis (Act/360 vs Act/365F shifts every pillar time
    /// by ~1.4%) and the reset lag. Set [`ForwardCurveBuilder::day_count`] and
    /// [`ForwardCurveBuilder::reset_lag`] explicitly when conventions matter;
    /// each day-count inference is logged at `debug` level.
    ///
    /// # Arguments
    ///
    /// * `id` - Market-data identifier; its text supplies inferred index conventions unless the builder overrides them.
    /// * `tenor_years` - Positive index accrual tenor in years, such as `0.25` for three months; validated at build time.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>, tenor_years: f64) -> ForwardCurveBuilder {
        let id: CurveId = id.into();
        let defaults = infer_forward_curve_defaults(id.as_str());
        let base =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        ForwardCurveBuilder {
            id,
            base,
            base_is_set: false,
            reset_lag: defaults.reset_lag_business_days,
            day_count: defaults.day_count,
            tenor: tenor_years,
            points: Vec::new(),
            projection_grid: None,
            style: InterpStyle::Linear,
            min_forward_rate: None,
            extrapolation: ExtrapolationPolicy::FlatForward,
            rate_calibration: None,
            fx_policy: None,
        }
    }

    /// Construct a flat forward curve quoting `rate` at every maturity.
    ///
    /// The curve stores two knots, `(0, rate)` and `(1, rate)`, with linear
    /// interpolation and flat-forward extrapolation, so `rate(t) == rate` for
    /// every non-negative `t`. Day count and reset lag follow the same curve-ID
    /// inference as [`ForwardCurve::builder`].
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"USD-SOFR-3M"`).
    /// * `tenor_years` - Index tenor in years (`0.25` for a 3-month index);
    ///   must be finite and strictly positive.
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `rate` - Simple forward rate as a decimal fraction (`0.04` is 4%);
    ///   must be finite.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `rate` is non-finite or `tenor_years`
    /// is non-finite or non-positive, or when the builder rejects the curve.
    pub fn flat(
        id: impl Into<CurveId>,
        tenor_years: f64,
        base_date: Date,
        rate: f64,
    ) -> crate::Result<Self> {
        if !rate.is_finite() {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::flat rate must be finite, got {rate}"
            )));
        }
        if !tenor_years.is_finite() || tenor_years <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::flat tenor must be finite and positive, got {tenor_years}"
            )));
        }
        Self::builder(id, tenor_years)
            .base_date(base_date)
            .knots([(0.0, rate), (1.0, rate)])
            .build()
    }

    /// Forward rate starting on `date` for the curve's tenor.
    ///
    /// Converts `date` to a year fraction from the base date under the curve
    /// day count and evaluates [`ForwardCurve::rate`].
    ///
    /// # Arguments
    ///
    /// * `date` - Calendar date on or after the curve base date at which the
    ///   forward rate is observed.
    ///
    /// # Errors
    ///
    /// Propagates a failure while computing the curve day-count fraction from
    /// the base date to `date`.
    pub fn rate_on_date(&self, date: Date) -> crate::Result<f64> {
        let t = crate::market_data::term_structures::common::year_fraction_to(
            self.base,
            date,
            self.day_count,
        )?;
        Ok(self.rate(t))
    }

    /// Forward rate starting at time `t` (in years) for the curve’s tenor.
    ///
    /// # Arguments
    ///
    /// * `t` - Year-fraction time from the curve or surface base date to the query point
    #[inline]
    #[must_use]
    pub fn rate(&self, t: f64) -> f64 {
        self.interp.interp(t)
    }

    /// Simple forward rate implied by projection discount factors between `t1` and `t2`.
    ///
    /// This is the period rate coherent with [`Self::df`]:
    ///
    /// ```text
    /// rate = (df(t1) / df(t2) - 1) / (t2 - t1)
    /// ```
    ///
    /// Use this for an arbitrary term period. For an index fixing at a reset
    /// date, use [`Self::rate`]. [`Self::rate_period`] instead returns the
    /// Simpson-rule integral average used by overnight compounding sub-windows.
    ///
    /// # Errors
    ///
    /// Returns an error when either time is non-finite, `t2 <= t1`, or an
    /// implied projection discount factor cannot be calculated.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year-fraction of the forward or rate interval being queried
    /// * `t2` - End year-fraction of the forward or rate interval being queried
    #[must_use = "computed forward rate should not be discarded"]
    pub fn rate_between(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !(t1.is_finite() && t2.is_finite()) {
            return Err(InputError::Invalid.into());
        }
        if t2 <= t1 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::rate_between requires t2 > t1; got t1={t1}, t2={t2}"
            )));
        }

        let log_growth = self.projection_log_df(t1)? - self.projection_log_df(t2)?;
        let rate = log_growth.exp_m1() / (t2 - t1);
        if !rate.is_finite() {
            return Err(crate::Error::Validation(format!(
                "Invalid implied forward rate for {} over [{t1}, {t2}]: {rate}",
                self.id.as_str()
            )));
        }
        Ok(rate)
    }

    /// Reset lag in business days from fixing to spot.
    #[inline]
    pub fn reset_lag(&self) -> i32 {
        self.reset_lag
    }

    /// Day-count convention used for this index.
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Index tenor in **years** (e.g. 0.25 = 3M).
    #[inline]
    pub fn tenor(&self) -> f64 {
        self.tenor
    }

    /// Raw knot times used to bootstrap the curve.
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Raw simple forward rates at each knot.
    #[inline]
    pub fn forwards(&self) -> &[f64] {
        &self.forwards
    }

    /// Contractual reset/end-date boundaries used for projection DFs, when present.
    ///
    /// This grid is independent of interpolation knots. `None` means the curve
    /// uses fixed numeric-tenor stepping from zero.
    #[inline]
    pub fn projection_grid(&self) -> Option<&[f64]> {
        self.projection_grid.as_deref()
    }

    /// Curve identifier.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }
    /// Valuation **base date**.
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Interpolation style used by this curve.
    #[inline]
    pub fn interp_style(&self) -> InterpStyle {
        self.interp.style()
    }

    /// Extrapolation policy used by this curve.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.interp.extrapolation()
    }

    /// Exact typed conventions and quotes used to calibrate this curve.
    #[inline]
    pub fn rate_calibration(
        &self,
    ) -> Option<&crate::market_data::term_structures::RateCalibrationRecipe> {
        self.rate_calibration.as_ref()
    }

    /// Opaque FX policy stamp set by the curve constructor; see
    /// [`crate::market_data::term_structures::DiscountCurve::fx_policy`] for the contract.
    #[inline]
    pub fn fx_policy(&self) -> Option<&str> {
        self.fx_policy.as_deref()
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

    /// Simpson-rule integral average rate over `[t1, t2]`.
    ///
    /// This is appropriate for averaging short overnight observation
    /// sub-windows, not for deriving the simple term forward over an arbitrary
    /// projection interval. Use [`Self::rate_between`] for the latter.
    ///
    /// # NaN contract
    ///
    /// Returns [`f64::NAN`] (rather than an error) if `t2 < t1` — misordered
    /// arguments are a caller bug, and changing the signature to `Result`
    /// would be too breaking. A `debug_assert` fires in debug builds and a
    /// `tracing` warning is emitted in release builds; callers must be
    /// prepared for NaN to propagate if they pass misordered times.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year-fraction of the forward or rate interval being queried
    /// * `t2` - End year-fraction of the forward or rate interval being queried
    #[inline]
    #[must_use]
    pub fn rate_period(&self, t1: f64, t2: f64) -> f64 {
        debug_assert!(
            t2 >= t1,
            "ForwardCurve::rate_period requires t1 <= t2 (got t1={t1}, t2={t2})"
        );
        if t2 < t1 {
            tracing::warn!(
                curve_id = %self.id,
                t1 = t1,
                t2 = t2,
                "ForwardCurve::rate_period called with t2 < t1; returning NaN. \
                 This is likely a caller bug — time arguments should satisfy t1 <= t2.",
            );
            return f64::NAN;
        }
        // Market-standard interpretation: average forward over the interval.
        //
        // We approximate the integral average of the interpolated forward curve:
        //   avg = (1 / (t2 - t1)) * ∫_{t1}^{t2} f(t) dt
        //
        // Use fixed-segment Simpson's rule for determinism (no adaptive stepping).
        // This is materially better than endpoint averaging for curved/interpolated shapes.
        let dt = t2 - t1;
        if dt <= 1e-12 {
            return self.rate(t1);
        }

        let n: usize = if dt > 20.0 {
            32
        } else if dt > 5.0 {
            16
        } else {
            8
        };
        simpson_rule(|t| self.rate(t), t1, t2, n).map_or(f64::NAN, |integral| integral / dt)
    }

    /// Logarithm of the implied projection discount factor from zero to `t`.
    fn projection_log_df(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() {
            return Err(InputError::Invalid.into());
        }
        if t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve df(t) requires t >= 0; got t={t}"
            )));
        }
        if t == 0.0 {
            return Ok(0.0);
        }

        let tau = self.tenor;
        if !tau.is_finite() || tau <= 0.0 {
            return Err(InputError::Invalid.into());
        }

        let mut log_df = 0.0_f64;
        let mut cur = 0.0_f64;
        let advance = |start: f64, end: f64, log_df: &mut f64| -> crate::Result<()> {
            let dt = end - start;
            if dt <= 0.0 {
                return Ok(());
            }
            let forward = self.rate(start);
            let accrual = forward * dt;
            let denom = 1.0 + accrual;
            if !denom.is_finite() || denom <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "Invalid implied projection DF step for {}: t={start:.6} -> {end:.6}, forward={forward:.6}, denom={denom:.6}",
                    self.id.as_str(),
                )));
            }
            *log_df -= accrual.ln_1p();
            Ok(())
        };

        if let Some(grid) = &self.projection_grid {
            for &boundary in grid.iter().skip(1) {
                if cur >= t {
                    break;
                }
                let nxt = boundary.min(t);
                advance(cur, nxt, &mut log_df)?;
                cur = nxt;
            }
        }

        while cur < t {
            let nxt = (cur + tau).min(t);
            if nxt <= cur {
                return Err(crate::Error::Validation(format!(
                    "ForwardCurve projection step made no progress at t={cur} toward {t}"
                )));
            }
            advance(cur, nxt, &mut log_df)?;
            cur = nxt;
        }
        Ok(log_df)
    }

    /// Implied **projection discount factor** from `0` to `t` (years).
    ///
    /// This is a convenience for Bloomberg-style curve inspection where a projection curve
    /// is displayed with both forward rates and an implied discount factor curve.
    ///
    /// With an explicit [`Self::projection_grid`], projection discount factors
    /// chain the contractual reset/end intervals independently of interpolation
    /// knots:
    ///
    /// ```text
    /// DF(0) = 1
    /// DF(reset_end) = DF(reset_start) / (1 + F(reset_start) * dt)
    /// ```
    ///
    /// This preserves fixed-tenor quote meaning when calendar adjustment makes
    /// a contractual period differ from the numeric tenor (for example, a 3M
    /// Act/360 period spanning 91 or 92 days). Without an explicit grid, the
    /// fixed numeric-tenor stepping from zero is used.
    ///
    /// Notes
    /// -----
    /// - This is **not** a discount curve used for PV discounting; it is an *implied projection DF*.
    /// - Explicit contractual intervals come from `projection_grid`.
    /// - Curves without a grid, and times beyond an explicit grid, step by `tenor_years`.
    /// - This is a simple-rate chaining helper, not an overnight compounded-in-arrears engine.
    ///
    /// # Errors
    ///
    /// Returns an error when `t` is non-finite or negative, the stored tenor
    /// is invalid, a projection step cannot produce a finite positive discount
    /// factor, or the final exponentiation is non-finite or non-positive.
    ///
    /// # Arguments
    ///
    /// * `t` - Year-fraction time from the curve or surface base date to the query point
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df(&self, t: f64) -> crate::Result<f64> {
        let df = self.projection_log_df(t)?.exp();
        if !df.is_finite() || df <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid implied projection DF for {} at t={t}: {df}",
                self.id.as_str()
            )));
        }
        Ok(df)
    }

    /// Implied projection discount factor on a calendar date using the curve's day-count.
    ///
    /// # Errors
    ///
    /// Returns an error if year fraction or discount factor calculation fails.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_on_date_curve(&self, date: Date) -> crate::Result<f64> {
        let t = if date == self.base {
            0.0
        } else {
            self.day_count
                .year_fraction(self.base, date, DayCountContext::default())?
        };
        self.df(t)
    }
}
