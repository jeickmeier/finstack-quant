//! Validated construction.

use super::*;

/// Fluent builder for [`HazardCurve`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::term_structures::HazardCurve;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = HazardCurve::builder("USD-CREDIT")
///     .base_date(base)
///     .recovery_rate(0.40)
///     .knots([(1.0, 0.01), (5.0, 0.015), (10.0, 0.02)])
///     .build()
///     .expect("HazardCurve builder should succeed");
/// assert!(curve.sp(5.0) < 1.0);
/// ```
pub struct HazardCurveBuilder {
    pub(super) id: CurveId,
    pub(super) base: Date,
    pub(super) points: Vec<(f64, f64)>, // (t, lambda)
    pub(super) recovery_rate: Option<f64>,
    pub(super) issuer: Option<String>,
    pub(super) seniority: Option<Seniority>,
    pub(super) currency: Option<Currency>,
    pub(super) day_count: DayCount,
    pub(super) par_points: Vec<(f64, f64)>, // (t, spread_bp)
    pub(super) par_interp: ParInterp,
    /// Survival-probability interpolation style (default LogLinear).
    pub(super) survival_interp: InterpStyle,
    /// Maximum allowed hazard rate (default 10.0).
    /// Rates above this trigger an error in `build()`.
    pub(super) max_hazard_rate: f64,
    pub(super) hazard_calibration:
        Option<crate::market_data::term_structures::HazardCalibrationRecipe>,
    pub(super) fx_policy: Option<String>,
}

impl HazardCurveBuilder {
    /// Set the **base date** for the curve.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = d;
        self
    }
    /// Set issuer metadata.
    pub fn issuer(mut self, name: impl Into<String>) -> Self {
        self.issuer = Some(name.into());
        self
    }
    /// Set seniority metadata.
    pub fn seniority(mut self, s: Seniority) -> Self {
        self.seniority = Some(s);
        self
    }
    /// Set currency metadata.
    pub fn currency(mut self, ccy: Currency) -> Self {
        self.currency = Some(ccy);
        self
    }
    /// Set day-count convention for the curve time axis.
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }
    /// Set recovery rate metadata.
    pub fn recovery_rate(mut self, r: f64) -> Self {
        self.recovery_rate = Some(r);
        self
    }
    /// Supply knot points `(t, λ)` where λ is the hazard rate.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }
    /// Store the market par spreads used for bootstrap for reporting.
    pub fn par_spreads<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.par_points.extend(pts);
        self
    }
    /// Set the interpolation method for par spreads.
    pub fn par_interp(mut self, method: ParInterp) -> Self {
        self.par_interp = method;
        self
    }

    /// Set the interpolation style for survival probabilities between
    /// pillars. The default [`InterpStyle::LogLinear`] is the market
    /// standard and the only supported style: it preserves consistency with
    /// the stored piecewise-constant hazard rates.
    ///
    /// # Arguments
    ///
    /// * `style` - Survival interpolation; must be [`InterpStyle::LogLinear`].
    ///   Other styles are rejected by [`build`](Self::build).
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.survival_interp = style;
        self
    }
    /// Attach the exact inputs used to calibrate this curve.
    ///
    /// # Arguments
    ///
    /// * `calibration` - Original hazard parameters, typed CDS quotes, and
    ///   solver policy required for deterministic quote-shock replay.
    pub fn hazard_calibration(
        mut self,
        calibration: crate::market_data::term_structures::HazardCalibrationRecipe,
    ) -> Self {
        self.hazard_calibration = Some(calibration);
        self
    }

    /// Optionally attach exact calibration replay inputs.
    ///
    /// # Arguments
    ///
    /// * `calibration` - Replay inputs to retain, or `None` for a curve that
    ///   was not produced by the calibration engine.
    pub fn hazard_calibration_opt(
        mut self,
        calibration: Option<crate::market_data::term_structures::HazardCalibrationRecipe>,
    ) -> Self {
        self.hazard_calibration = calibration;
        self
    }

    /// Set the maximum allowed hazard rate.
    ///
    /// During `build()`, any hazard rate exceeding this value triggers an error.
    /// The default is `10.0` (implies >99.995% 1Y default probability).
    pub fn max_hazard_rate(mut self, max: f64) -> Self {
        self.max_hazard_rate = max;
        self
    }

    /// Optionally set issuer metadata (no-op if `None`).
    pub fn issuer_opt(mut self, name: Option<impl Into<String>>) -> Self {
        self.issuer = name.map(Into::into);
        self
    }

    /// Optionally set seniority metadata (no-op if `None`).
    pub fn seniority_opt(mut self, s: Option<Seniority>) -> Self {
        self.seniority = s;
        self
    }

    /// Optionally set currency metadata (no-op if `None`).
    pub fn currency_opt(mut self, ccy: Option<Currency>) -> Self {
        self.currency = ccy;
        self
    }

    /// Stamp an opaque FX policy on the curve. See [`HazardCurve::fx_policy`].
    pub fn fx_policy(mut self, policy: impl Into<String>) -> Self {
        self.fx_policy = Some(policy.into());
        self
    }

    /// Optionally stamp an FX policy; `None` is a no-op. Used by the serde
    /// round-trip path and by curve builders propagating metadata.
    pub fn fx_policy_opt(mut self, policy: Option<String>) -> Self {
        self.fx_policy = policy;
        self
    }

    /// Validate input and build the [`HazardCurve`].
    ///
    /// # Validation
    ///
    /// - Base date must be explicitly set (not the default 1970-01-01)
    /// - At least one knot point required
    /// - All hazard rates must be non-negative and finite
    /// - Hazard rates > `max_hazard_rate` (default 10.0) trigger an error
    /// - Recovery rate must be supplied explicitly and lie in [0, 1]
    /// - Knot times must be strictly increasing after sorting by time
    /// - Stored par-spread tenors must be finite and non-negative, and spreads
    ///   must be finite; they are retained for reporting rather than used to
    ///   re-bootstrap hazards
    ///
    /// The only supported survival interpolation is log-linear, which corresponds to
    /// piecewise-constant hazards between pillars. A zero-time knot is allowed;
    /// its hazard applies from the base date onward.
    ///
    /// # Errors
    ///
    /// Returns an error when the base date was not explicitly set, no knots
    /// are supplied, a time, rate, recovery rate, or stored par spread is
    /// invalid, knot times are duplicated, or a hazard rate exceeds the
    /// configured maximum, or survival interpolation is not log-linear.
    /// Input points are sorted by time before the curve is
    /// constructed; callers need not pre-sort them.
    pub fn build(self) -> crate::Result<HazardCurve> {
        if self.survival_interp != InterpStyle::LogLinear {
            return Err(crate::Error::Validation(
                "HazardCurve requires log-linear survival interpolation for piecewise-constant hazards".to_string(),
            ));
        }
        // Require explicit base_date to avoid accidentally anchoring to 1970-01-01
        let default_base =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        if self.base == default_base {
            return Err(InputError::Invalid.into());
        }
        if self.points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let recovery_rate = self.recovery_rate.ok_or_else(|| {
            crate::Error::Validation(
                "HazardCurve requires an explicit recovery_rate in [0, 1]".to_string(),
            )
        })?;

        // Validate knot times and hazard rates: times must be finite/non-negative;
        // rates non-negative and finite; a zero-time anchor is allowed, but all
        // subsequent knots must increase strictly.
        for &(t, lambda) in &self.points {
            if !t.is_finite() || t < 0.0 {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve knot time must be finite and non-negative, got t={t}"
                )));
            }
            if lambda < 0.0 {
                return Err(InputError::NegativeValue.into());
            }
            if !lambda.is_finite() {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve hazard rate must be finite, got lambda={lambda} at t={t}"
                )));
            }
            // Sanity check: λ exceeding max_hazard_rate is almost certainly a
            // data error (units confusion, etc.).  Default limit is 10.0 which
            // implies >99.995% 1Y default probability.
            if lambda > self.max_hazard_rate {
                return Err(crate::Error::Validation(format!(
                    "Hazard rate {lambda:.4} at t={t:.4}y exceeds maximum {:.4}. \
                     Use .allow_high_hazard_rates() or .max_hazard_rate() to override.",
                    self.max_hazard_rate
                )));
            }
        }

        crate::market_data::term_structures::common::validate_unit_range(
            recovery_rate,
            "recovery_rate",
        )?;

        let mut points = self.points;
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (kvec, lvec): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
        if kvec.len() > 1 {
            for i in 1..kvec.len() {
                if kvec[i] <= kvec[i - 1] {
                    return Err(InputError::Invalid.into());
                }
            }
        }
        let mut par_pts = self.par_points;
        for &(t, spread) in &par_pts {
            if !t.is_finite() || t < 0.0 || !spread.is_finite() {
                return Err(InputError::Invalid.into());
            }
        }
        par_pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (p_ten, p_spd): (Vec<f64>, Vec<f64>) = par_pts.into_iter().unzip();
        if let Some(recipe) = &self.hazard_calibration {
            recipe.validate()?;
        }

        // Convert hazard rates to survival probabilities for interpolation
        // using the single canonical λ-attribution convention shared with
        // `rebuild_interp` (see `survival_pillars`).
        let (interp_kvec, interp_svec) = survival_pillars(&kvec, &lvec);

        // Build interpolator over survival probabilities. The default
        // LogLinear style implies a piecewise-constant hazard rate.
        // Extrapolate with FlatForward (constant hazard rate at tail).
        let interp = crate::market_data::term_structures::common::build_interp(
            self.survival_interp,
            interp_kvec.into_boxed_slice(),
            interp_svec.into_boxed_slice(),
            ExtrapolationPolicy::FlatForward,
        )?;

        Ok(HazardCurve {
            id: self.id,
            base: self.base,
            knots: kvec.into_boxed_slice(),
            lambdas: lvec.into_boxed_slice(),
            recovery_rate,
            issuer: self.issuer,
            seniority: self.seniority,
            currency: self.currency,
            day_count: self.day_count,
            par_tenors: p_ten.into_boxed_slice(),
            par_spreads_bp: p_spd.into_boxed_slice(),
            par_interp: self.par_interp,
            survival_interp_style: self.survival_interp,
            hazard_calibration: self.hazard_calibration,
            interp,
            fx_policy: self.fx_policy,
        })
    }
}
