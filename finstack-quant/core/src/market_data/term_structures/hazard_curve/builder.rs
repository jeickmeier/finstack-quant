//! Validated construction for [`HazardCurve`].

use super::super::common::{signed_curve_year_fraction, validate_curve_clock};
use super::*;
use crate::InputError;
use crate::currency::Currency;
use crate::dates::{Date, DayCount};
use crate::math::interp::exact_f64_eq;
use crate::types::CurveId;

/// Fluent builder for [`HazardCurve`].
///
/// Requires explicit `base_date`, `day_count` (plus `calendar_id` for `Bus/252`),
/// `recovery_rate`, and strictly positive strictly ordered `knots` before
/// calling [`HazardCurveBuilder::build`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Date, DayCount};
/// use finstack_quant_core::market_data::term_structures::HazardCurve;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = HazardCurve::builder("USD-CREDIT")
///     .base_date(base)
///     .day_count(DayCount::Act365F)
///     .recovery_rate(0.40)
///     .knots([(1.0, 0.01), (5.0, 0.015), (10.0, 0.02)])
///     .build()
///     .expect("HazardCurve builder should succeed");
/// assert!(curve.sp(5.0).expect("valid survival") < 1.0);
/// ```
pub struct HazardCurveBuilder {
    pub(super) id: CurveId,
    pub(super) base: Option<Date>,
    pub(super) points: Vec<(f64, f64)>,
    pub(super) recovery_rate: Option<f64>,
    pub(super) issuer: Option<String>,
    pub(super) seniority: Option<Seniority>,
    pub(super) currency: Option<Currency>,
    pub(super) day_count: Option<DayCount>,
    pub(super) calendar_id: Option<String>,
    pub(super) hazard_calibration:
        Option<crate::market_data::term_structures::HazardCalibrationRecipe>,
    pub(super) fx_policy: Option<String>,
    pub(super) source: Option<HazardConditionSource>,
}

impl HazardCurveBuilder {
    /// Set the **base date** for the curve.
    ///
    /// # Arguments
    ///
    /// * `d` - Valuation anchor date corresponding to `t = 0`.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = Some(d);
        self
    }

    /// Set issuer metadata.
    ///
    /// # Arguments
    ///
    /// * `name` - Obligor or reference entity name stored as curve metadata.
    pub fn issuer(mut self, name: impl Into<String>) -> Self {
        self.issuer = Some(name.into());
        self
    }

    /// Set seniority metadata.
    ///
    /// # Arguments
    ///
    /// * `s` - Debt seniority classification associated with the curve.
    pub fn seniority(mut self, s: Seniority) -> Self {
        self.seniority = Some(s);
        self
    }

    /// Set currency metadata.
    ///
    /// # Arguments
    ///
    /// * `ccy` - Currency of the underlying credit obligation or protection leg.
    pub fn currency(mut self, ccy: Currency) -> Self {
        self.currency = Some(ccy);
        self
    }

    /// Set day-count convention for the curve time axis.
    ///
    /// # Arguments
    ///
    /// * `day_count` - Explicit supported curve day-count convention (cannot be
    ///   `ActActIsma`, `Act365L`, `ThirtyE360Isda`, or calendarless `Bus252`).
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = Some(day_count);
        self
    }

    /// Set the registered holiday calendar of the curve's time axis.
    ///
    /// Required for business-day day counts such as `Bus252`.
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
    /// * `calendar_id` - Registered calendar id, or `None` for day counts that
    ///   need no holiday calendar.
    pub fn calendar_id_opt(mut self, calendar_id: Option<String>) -> Self {
        self.calendar_id = calendar_id;
        self
    }

    /// Set recovery rate metadata.
    ///
    /// # Arguments
    ///
    /// * `r` - Assumed recovery on default as a decimal fraction in `[0, 1]`.
    pub fn recovery_rate(mut self, r: f64) -> Self {
        self.recovery_rate = Some(r);
        self
    }

    /// Supply strictly ordered positive ending knot points `(t, λ)`.
    ///
    /// Each `t > 0` ends the piecewise-constant interval `(tᵢ₋₁, tᵢ]` carrying
    /// finite annual default intensity `λ >= 0` (decimal per source-clock year).
    ///
    /// # Arguments
    ///
    /// * `pts` - Strictly ascending `(t, lambda)` pairs with `t > 0` and
    ///   `lambda >= 0`.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
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

    /// Optionally set issuer metadata (`None` clears it).
    pub(crate) fn issuer_opt(mut self, name: Option<impl Into<String>>) -> Self {
        self.issuer = name.map(Into::into);
        self
    }

    /// Optionally set seniority metadata (`None` clears it).
    pub(crate) fn seniority_opt(mut self, s: Option<Seniority>) -> Self {
        self.seniority = s;
        self
    }

    /// Optionally set currency metadata (`None` clears it).
    pub(crate) fn currency_opt(mut self, ccy: Option<Currency>) -> Self {
        self.currency = ccy;
        self
    }

    /// Stamp an opaque FX policy on the curve.
    ///
    /// # Arguments
    ///
    /// * `policy` - FX policy metadata identifier preserved across rebuilds.
    pub fn fx_policy(mut self, policy: impl Into<String>) -> Self {
        self.fx_policy = Some(policy.into());
        self
    }

    /// Optionally stamp an FX policy; `None` clears it.
    ///
    /// # Arguments
    ///
    /// * `policy` - Optional FX policy metadata identifier.
    pub fn fx_policy_opt(mut self, policy: Option<String>) -> Self {
        self.fx_policy = policy;
        self
    }

    /// Validate inputs and construct the [`HazardCurve`].
    ///
    /// # Errors
    ///
    /// Returns an error if `base_date`, `day_count`, or `recovery_rate` is
    /// missing, `recovery_rate` is outside `[0, 1]`, the day-count clock is
    /// unsupported or missing its `Bus252` calendar, `knots` is empty, any
    /// knot time is non-finite, `<= 0.0`, or not strictly increasing as supplied,
    /// any intensity is non-finite or negative, or retained conditioned-source
    /// invariants fail.
    pub fn build(self) -> crate::Result<HazardCurve> {
        let base = self.base.ok_or_else(|| {
            crate::Error::Validation("HazardCurve requires base_date".to_string())
        })?;
        let day_count = self.day_count.ok_or_else(|| {
            crate::Error::Validation("HazardCurve requires day_count".to_string())
        })?;
        let recovery_rate = self.recovery_rate.ok_or_else(|| {
            crate::Error::Validation(
                "HazardCurve requires an explicit recovery_rate in [0, 1]".to_string(),
            )
        })?;
        crate::validation::validate_f64_unit_interval(recovery_rate, "recovery_rate")?;

        let calendar = validate_curve_clock(
            "HazardCurve",
            self.id.as_str(),
            day_count,
            self.calendar_id.as_deref(),
        )?;

        if self.points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        let mut kvec = Vec::with_capacity(self.points.len());
        let mut lvec = Vec::with_capacity(self.points.len());
        for (idx, &(t, lambda)) in self.points.iter().enumerate() {
            if !t.is_finite() || t <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve knot time must be finite and strictly positive (t > 0), got t={t}"
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
            if idx > 0 && t <= self.points[idx - 1].0 {
                return Err(InputError::NonMonotonicKnots.into());
            }
            kvec.push(t);
            lvec.push(lambda);
        }

        let mut source = self.source;
        if let Some(src) = &mut source {
            if base < src.base_date {
                return Err(crate::Error::Validation(format!(
                    "HazardCurve base_date ({base}) cannot precede source base_date ({})",
                    src.base_date
                )));
            }
            if src.knot_points.is_empty() {
                return Err(InputError::TooFewPoints.into());
            }
            for (idx, &(t, lambda)) in src.knot_points.iter().enumerate() {
                if !t.is_finite() || t <= 0.0 || !lambda.is_finite() || lambda < 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "HazardCurve source knot ({t}, {lambda}) must have finite t > 0 and lambda >= 0"
                    )));
                }
                if idx > 0 && t <= src.knot_points[idx - 1].0 {
                    return Err(InputError::NonMonotonicKnots.into());
                }
            }
            src.offset =
                signed_curve_year_fraction(src.base_date, base, day_count, calendar.as_ref())?;
            if !src.offset.is_finite() || src.offset < 0.0 {
                return Err(crate::Error::Validation(format!(
                    "hazard curve condition offset must be finite and non-negative, got {}",
                    src.offset
                )));
            }
            let expected_points = surviving_source_knots(&src.knot_points, src.offset);
            if kvec.len() != expected_points.len()
                || kvec
                    .iter()
                    .zip(lvec.iter())
                    .zip(expected_points.iter())
                    .any(|((&kt, &kl), &(et, el))| !exact_f64_eq(kt, et) || !exact_f64_eq(kl, el))
            {
                return Err(crate::Error::Validation(
                    "conditioned hazard curve pillars do not match retained source curve"
                        .to_string(),
                ));
            }
        }

        Ok(HazardCurve {
            id: self.id,
            base,
            knots: kvec.into_boxed_slice(),
            lambdas: lvec.into_boxed_slice(),
            recovery_rate,
            issuer: self.issuer,
            seniority: self.seniority,
            currency: self.currency,
            day_count,
            calendar,
            hazard_calibration: self.hazard_calibration,
            fx_policy: self.fx_policy,
            source,
        })
    }
}

/// Compute conditioned surviving `(rem, lambda)` pillars relative to `offset`.
pub(super) fn surviving_source_knots(source_knots: &[(f64, f64)], offset: f64) -> Vec<(f64, f64)> {
    let mut surviving: Vec<(f64, f64)> = source_knots
        .iter()
        .filter_map(|&(source_t, source_lambda)| {
            if source_t > offset {
                let rem = source_t - offset;
                (rem.is_finite() && rem > 0.0).then_some((rem, source_lambda))
            } else {
                None
            }
        })
        .collect();
    if surviving.is_empty() {
        let tail_lambda = source_knots[source_knots.len() - 1].1;
        surviving.push((1.0, tail_lambda));
    }
    surviving
}
