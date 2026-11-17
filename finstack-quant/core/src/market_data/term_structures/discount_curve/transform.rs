//! Discount-curve bumping and forward-conditioning transformations.

use super::super::common::factor::{roll_positive_factor_points, verify_triangular_breakpoints};
use super::super::common::signed_curve_year_fraction;
use super::DiscountCurve;
use crate::dates::Date;
use crate::market_data::bumps::{BumpSpec, Bumpable};
use crate::math::interp::exact_f64_eq;

impl DiscountCurve {
    /// Apply a bump specification in-place, retaining the original interpolation.
    ///
    /// Additive bumps are **continuously compounded zero-space** shocks
    /// (`DF *= exp(-delta_r * t)`), not quote re-bootstraps. Triangular shocks act
    /// continuously in maturity, including between and beyond stored pillars.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        let val = spec.resolve_additive_or_error("DiscountCurve")?;
        if exact_f64_eq(val, 0.0) {
            return Ok(());
        }
        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| super::evaluation::CurveTransform::from_curve(self));
        transform.bump(val, spec.bump_type)?;

        let mut dfs = Vec::with_capacity(self.knots.len());
        for &time in self.knots.iter() {
            dfs.push(transform.df(&self.interp, time)?);
        }

        // Check in-coverage shape breakpoints for triangular key-rate shocks.
        let min_src = self
            .transform
            .as_ref()
            .map_or(self.knots[0], |tr| tr.source_first_knot());
        let max_src = self.transform.as_ref().map_or_else(
            || self.knots[self.knots.len() - 1],
            |tr| tr.source_points()[tr.source_points().len() - 1].0,
        );
        verify_triangular_breakpoints(
            spec,
            &self.interp,
            min_src,
            max_src,
            transform.offset(),
            |src_t| transform.df_at_source_time(&self.interp, src_t),
        )?;

        self.dfs = dfs.into_boxed_slice();
        self.transform = Some(transform);
        self.rate_calibration = None;
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
        let src_base = self.source_base_date();
        if reference_date < src_base {
            return Err(crate::Error::Validation(format!(
                "DiscountCurve::condition requires reference_date ({reference_date}) on or after source base date ({src_base})"
            )));
        }
        let _ = signed_curve_year_fraction(
            src_base,
            reference_date,
            self.day_count,
            self.calendar.as_ref(),
        )?;
        let c_w = self.df_on_date(reference_date)?;
        if !c_w.is_finite() || c_w <= 0.0 {
            return Err(crate::InputError::NonPositiveValue.into());
        }
        if reference_date == self.base {
            return Ok(self.clone());
        }

        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| super::evaluation::CurveTransform::from_curve(self));
        transform.condition(
            &self.interp,
            reference_date,
            self.day_count,
            self.day_count_context(),
        )?;
        let new_offset = transform.offset();

        let (knots, dfs) = roll_positive_factor_points(
            transform.source_points(),
            &self.interp,
            new_offset,
            |source_t| transform.df_at_source_time(&self.interp, source_t),
        )?;

        let mut conditioned = self.clone();
        conditioned.base = reference_date;
        conditioned.knots = knots.into_boxed_slice();
        conditioned.dfs = dfs.into_boxed_slice();
        conditioned.transform = Some(transform);
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
