//! Discount-curve bumping and forward-conditioning transformations.

use super::super::common::factor::{
    FactorBumpState, FactorClock, FactorConditionView, bump_factor_curve_in_place,
    condition_factor_curve,
};
use super::DiscountCurve;
use crate::dates::Date;
use crate::market_data::bumps::{BumpSpec, Bumpable};

impl DiscountCurve {
    /// Apply a bump specification in-place, retaining the original interpolation.
    ///
    /// Additive bumps are **continuously compounded zero-space** shocks
    /// (`DF *= exp(-delta_r * t)`), not quote re-bootstraps. Triangular shocks act
    /// continuously in maturity, including between and beyond stored pillars.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        let state = FactorBumpState {
            base: self.base,
            knots: &self.knots,
            values: &mut self.dfs,
            interp: &self.interp,
            transform: &mut self.transform,
        };
        if bump_factor_curve_in_place("DiscountCurve", state, spec)? {
            self.rate_calibration = None;
        }
        Ok(())
    }

    /// Condition the curve forward or backward to `reference_date`, realizing forwards.
    ///
    /// Re-anchors the curve at `reference_date` using the original source time
    /// coordinate measured from [`source_base_date`](Self::source_base_date)
    /// while preserving source interpolation, tails, and accumulated shocks.
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
    /// ```
    /// use finstack_quant_core::dates::{Date, DayCount};
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = DiscountCurve::builder("USD_OIS")
    ///     .base_date(base_date)
    ///     .day_count(DayCount::Act365F)
    ///     .knots(vec![(0.5, 0.99), (1.0, 0.98), (2.0, 0.96), (5.0, 0.90)])
    ///     .interp(InterpStyle::MonotoneConvex)
    ///     .extrapolation(ExtrapolationPolicy::Extend)
    ///     .build()?;
    ///
    /// let conditioned = curve.condition(date!(2025 - 07 - 20))?;
    /// assert!(conditioned.knots().len() < curve.knots().len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn condition(&self, reference_date: Date) -> crate::Result<Self> {
        let Some(next) = condition_factor_curve(
            "DiscountCurve",
            FactorConditionView {
                clock: FactorClock {
                    base: self.base,
                    day_count: self.day_count,
                    calendar: self.calendar.as_ref(),
                },
                knots: &self.knots,
                values: &self.dfs,
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
        conditioned.dfs = next.values;
        conditioned.transform = Some(next.transform);
        conditioned.rate_calibration = None;
        Ok(conditioned)
    }
}

impl Bumpable for DiscountCurve {
    /// Apply a continuously compounded zero-space shock.
    ///
    /// Additive `RateBp` / `Percent` / `Fraction` bumps shift zeros via
    /// `DF_bumped(t) = DF(t) * exp(-delta_r * t)` (and the triangular-weighted
    /// analogue for key-rate bumps). This does not re-bootstrap from
    /// stored [`crate::market_data::term_structures::RateCalibrationRecipe`] quotes.
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = spec.standard_bump_id(self.id());
        bumped.bump_in_place(&spec)?;
        Ok(bumped)
    }
}
