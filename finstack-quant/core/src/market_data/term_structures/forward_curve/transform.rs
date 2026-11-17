//! Forward-curve bumping and forward/backward conditioning transformations.

use super::super::common::factor::{roll_positive_factor_points, verify_triangular_breakpoints};
use super::super::common::signed_curve_year_fraction;
use super::{ForwardCurve, ForwardCurveBuilder};
use crate::dates::Date;
use crate::market_data::bumps::{BumpSpec, Bumpable};
use crate::math::interp::exact_f64_eq;
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
        let val = spec.resolve_additive_or_error("ForwardCurve")?;
        if exact_f64_eq(val, 0.0) {
            return Ok(());
        }
        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| super::evaluation::CurveTransform::from_curve(self));
        transform.bump(val, spec.bump_type)?;

        let mut factors = Vec::with_capacity(self.knots.len());
        for &time in self.knots.iter() {
            factors.push(transform.df(&self.interp, time)?);
        }

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

        self.factors = factors.into_boxed_slice();
        self.transform = Some(transform);
        self.rate_calibration = None;
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
        let src_base = self.source_base_date();
        if reference_date < src_base {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::condition requires reference_date ({reference_date}) on or after source base date ({src_base})"
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

        let (knots, factors) = roll_positive_factor_points(
            transform.source_points(),
            &self.interp,
            new_offset,
            |source_t| transform.df_at_source_time(&self.interp, source_t),
        )?;

        let mut conditioned = self.clone();
        conditioned.base = reference_date;
        conditioned.knots = knots.into_boxed_slice();
        conditioned.factors = factors.into_boxed_slice();
        conditioned.transform = Some(transform);
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
