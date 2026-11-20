//! Forward-curve bumping and forward/backward conditioning transformations.

use super::super::common::factor::{
    FactorBumpState, FactorClock, FactorConditionView, bump_factor_curve_in_place,
    condition_factor_curve,
};
use super::{ForwardCurve, ForwardCurveBuilder};
use crate::dates::Date;
use crate::market_data::bumps::{BumpSpec, Bumpable};
use crate::types::CurveId;

impl ForwardCurve {
    /// Create a builder pre-populated with this curve's data under a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> ForwardCurveBuilder {
        let mut builder = self
            .metadata_builder(new_id)
            .knots(self.knots.iter().copied().zip(self.factors.iter().copied()));
        builder.transform = self.transform.clone();
        builder
    }

    /// Rebuild this curve with replacement knots while preserving structural metadata.
    ///
    /// # Arguments
    ///
    /// * `knots` - Replacement strictly increasing `(time, factor)` pillars in years
    ///   from [`base_date`](Self::base_date) under [`day_count`](Self::day_count).
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
    pub(crate) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> ForwardCurveBuilder {
        ForwardCurve::builder(new_id)
            .base_date(self.base)
            .day_count(self.day_count)
            .calendar_id_opt(self.calendar.as_ref().map(|calendar| calendar.id.clone()))
            .index_tenor_opt(self.index_tenor)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .rate_calibration_opt(self.rate_calibration.clone())
            .fx_policy_opt(self.fx_policy.clone())
    }

    /// Apply a bump specification in-place, retaining the original interpolation.
    ///
    /// Additive bumps are **continuously compounded zero-space** shocks
    /// (`P_b(t) = P(t) * exp(-delta_r * t * w(t))`), not quote re-bootstraps.
    /// Triangular shocks act continuously in maturity, including between and
    /// beyond stored pillars. Multiplicative/factor shocks are rejected.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        let state = FactorBumpState {
            base: self.base,
            knots: &self.knots,
            values: &mut self.factors,
            interp: &self.interp,
            transform: &mut self.transform,
        };
        if bump_factor_curve_in_place("ForwardCurve", state, spec)? {
            self.rate_calibration = None;
        }
        Ok(())
    }

    /// Condition the curve forward or backward to `reference_date`.
    ///
    /// Re-anchors the curve at `reference_date` using the original source time
    /// coordinate measured from [`source_base_date`](Self::source_base_date)
    /// while preserving source interpolation, tails, index tenor, and
    /// accumulated shocks.
    ///
    /// # Arguments
    ///
    /// * `reference_date` - Target valuation anchor date on or after
    ///   [`source_base_date`](Self::source_base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `reference_date < source_base_date()`, computing the
    /// year fraction to `reference_date` fails, or the current factor at
    /// `reference_date` cannot be admitted.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::DayCount;
    /// use finstack_quant_core::market_data::term_structures::ForwardCurve;
    /// use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = ForwardCurve::builder("USD_SOFR_3M")
    ///     .base_date(base_date)
    ///     .day_count(DayCount::Act360)
    ///     .knots([(0.5, 0.98), (1.0, 0.96), (2.0, 0.92), (5.0, 0.80)])
    ///     .interp(InterpStyle::MonotoneConvex)
    ///     .extrapolation(ExtrapolationPolicy::Extend)
    ///     .build()?;
    ///
    /// let conditioned = curve.condition(date!(2025 - 07 - 02))?;
    /// assert_eq!(conditioned.base_date(), date!(2025 - 07 - 02));
    /// assert!(conditioned.knots().len() < curve.knots().len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn condition(&self, reference_date: Date) -> crate::Result<Self> {
        let Some(next) = condition_factor_curve(
            "ForwardCurve",
            FactorConditionView {
                clock: FactorClock {
                    base: self.base,
                    day_count: self.day_count,
                    calendar: self.calendar.as_ref(),
                },
                knots: &self.knots,
                values: &self.factors,
                interp: &self.interp,
                transform: self.transform.as_ref(),
            },
            reference_date,
        )?
        else {
            return Ok(self.clone());
        };

        let mut conditioned = self.clone();
        conditioned.base = reference_date;
        conditioned.knots = next.knots;
        conditioned.factors = next.values;
        conditioned.transform = Some(next.transform);
        conditioned.rate_calibration = None;
        Ok(conditioned)
    }
}

impl Bumpable for ForwardCurve {
    /// Apply a continuously compounded zero-space shock.
    ///
    /// Additive `RateBp` / `Percent` / `Fraction` bumps shift factors via
    /// `P_bumped(t) = P(t) * exp(-delta_r * t * w(t))`.
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = spec.standard_bump_id(self.id());
        bumped.bump_in_place(&spec)?;
        Ok(bumped)
    }
}
