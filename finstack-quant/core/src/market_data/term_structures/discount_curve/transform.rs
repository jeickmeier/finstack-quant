//! Discount-curve bumping and roll-forward transformations.

use super::super::common::{roll_knots, year_fraction_to_with_ctx};
use super::DiscountCurve;
use crate::dates::DateExt;
use crate::market_data::bumps::{BumpSpec, BumpType, Bumpable};

impl DiscountCurve {
    /// Apply a bump specification in-place, retaining the original interpolation.
    ///
    /// Additive bumps are **continuously compounded zero-space** shocks
    /// (`DF *= exp(−δr · t)`), not quote re-bootstraps. Triangular shocks act
    /// continuously in maturity, including between and beyond stored pillars.
    ///
    /// # Performance
    ///
    /// Merges shocks into a flat piecewise-quadratic log-DF adjustment. Repeated
    /// shocks on the same grid do not create nested curves or extra breakpoints.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        let (val, is_multiplicative) = spec.resolve_standard_values_or_error(
            "DiscountCurve",
            "only supports Additive/{RateBp,Percent,Fraction} bumps",
        )?;
        if is_multiplicative {
            return Err(crate::InputError::UnsupportedBump {
                reason: "DiscountCurve does not support Multiplicative bumps".to_string(),
            }
            .into());
        }
        if val == 0.0 {
            return Ok(());
        }
        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| super::evaluation::CurveTransform::from_curve(self));
        transform.bump(val, spec.bump_type)?;
        let dfs: Box<[f64]> = self
            .knots
            .iter()
            .map(|&time| transform.df(&self.interp, time))
            .collect();
        if dfs.iter().any(|df| !df.is_finite() || *df <= 0.0) {
            return Err(crate::Error::Validation(
                "discount-curve shock produced invalid discount factors".into(),
            ));
        }
        // A sparse curve may have no pillar inside the shock support. Validate
        // its peak too, so an overflowing key-rate shock cannot hide between nodes.
        if let BumpType::TriangularKeyRate { target_bucket, .. } = spec.bump_type {
            let peak_df = transform.df(&self.interp, target_bucket);
            if !peak_df.is_finite() || peak_df <= 0.0 {
                return Err(crate::Error::Validation(
                    "discount-curve shock produced an invalid peak discount factor".into(),
                ));
            }
        }
        self.dfs = dfs;
        self.transform = Some(transform);
        // Stress curves retain mathematical validation on JSON round-trips.
        // The original construction policy remains unchanged on `self`'s
        // source curve when using the public cloning bump methods.
        self.allow_non_monotonic = true;
        self.min_forward_rate = None;
        Ok(())
    }

    /// Roll the curve forward by a specified number of days, realizing forwards.
    ///
    /// This creates a new curve with:
    /// - Base date advanced by `days`
    /// - Knot times shifted backwards (t' = t - dt_years)
    /// - Points with t' <= 0 are filtered out (expired)
    /// - Discount factors renormalized by the realized forward:
    ///   `DF_new(t - dt) = DF_old(t) / DF_old(dt)`
    ///
    /// These are **realized-forward** semantics (per the 2026-06-09 core quant
    /// review): forwards realize as the curve rolls, so a flat curve stays
    /// flat under roll, and a roll-then-reprice theta captures both carry and
    /// roll-down. The relationship to present values is
    /// `PV(rolled curve, T - dt) = PV(old curve, T) / DF_old(dt)` — i.e. the
    /// rolled PV is the forward value of the old PV to the new base date.
    /// This aligns the discount curve with the hazard, forward, inflation,
    /// and price/vol-index curve rolls, which already realize forwards.
    ///
    /// # Arguments
    /// * `days` - Signed calendar-day shift; negative values move the base date backward.
    ///
    /// # Returns
    /// A new discount curve with updated base date and renormalized knots.
    ///
    /// # Errors
    /// Returns an error if fewer than 2 knot points remain after filtering
    /// expired points, or if `DF_old(dt)` is not positive and finite (which
    /// can only happen if extrapolation past the last knot misbehaves).
    ///
    /// # Examples
    /// ```
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = DiscountCurve::builder("USD_OIS")
    ///     .base_date(base_date)
    ///     .knots(vec![(0.5, 0.99), (1.0, 0.98), (2.0, 0.96), (5.0, 0.90)])
    ///     .build()
    ///     ?;
    ///
    /// // Roll past the 0.5Y knot so that point expires (182 days is still
    /// // short of 0.5 Act/365F years).
    /// let rolled = curve.roll_forward(200)?;
    /// assert!(rolled.knots().len() < curve.knots().len());
    /// # Ok(())
    /// # }
    /// ```
    /// Returns a validation error if the rolled base date exceeds the supported calendar range.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        if days == 0 {
            return Ok(self.clone());
        }
        let new_base = self.base.add_days(days)?;
        let dt_years = year_fraction_to_with_ctx(
            self.base,
            new_base,
            self.day_count,
            self.day_count_context(),
        )?;

        // Realized-forward renormalization: divide every rolled DF by the
        // old curve's DF at the roll horizon, interpolated in the curve's own
        // time basis (the same `dt_years` the knots are shifted by).
        let df_dt = self.df(dt_years);
        if !df_dt.is_finite() || df_dt <= 0.0 {
            return Err(crate::InputError::NonPositiveValue.into());
        }

        let rolled_points: Vec<(f64, f64)> = roll_knots(&self.knots, &self.dfs, dt_years)
            .into_iter()
            .map(|(t, df)| (t, df / df_dt))
            .collect();

        // Knots inside (0, dt] are dropped by `roll_knots` (expired).
        // `build()` re-prepends a (0.0, 1.0) knot, which is now exactly
        // correct: DF_new(0) = DF_old(dt) / DF_old(dt) = 1. One live knot is
        // therefore enough.
        if rolled_points.is_empty() {
            return Err(crate::InputError::TooFewPoints.into());
        }
        if rolled_points
            .iter()
            .any(|&(_, df)| !df.is_finite() || df <= 0.0)
        {
            return Err(crate::Error::Validation(
                "discount-curve roll produced invalid discount factors".into(),
            ));
        }

        // Preserve the source interpolation and shift its evaluation origin.
        // Reconstructing a smooth interpolator from the remaining pillars
        // changes its boundary derivative and introduces artificial roll P&L.
        let mut rolled = self.clone();
        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| super::evaluation::CurveTransform::from_curve(self));
        transform.roll(&self.interp, dt_years)?;
        let mut points = Vec::with_capacity(rolled_points.len() + 1);
        points.push((0.0, 1.0));
        points.extend(rolled_points);
        let (knots, dfs): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
        if !self.allow_non_monotonic {
            super::validation::validate_monotonic_df(&knots, &dfs)?;
        }
        if let Some(floor) = self.min_forward_rate {
            super::validation::validate_forward_rates(&knots, &dfs, floor)?;
        }
        rolled.base = new_base;
        rolled.knots = knots.into_boxed_slice();
        rolled.dfs = dfs.into_boxed_slice();
        rolled.transform = Some(transform);
        Ok(rolled)
    }
}

impl Bumpable for DiscountCurve {
    /// Apply a continuously compounded zero-space shock.
    ///
    /// Additive `RateBp` / `Percent` / `Fraction` bumps shift zeros via
    /// `DF_bumped(t) = DF(t) · exp(−δr · t)` (and the triangular-weighted
    /// analogue for key-rate bumps). This does **not** re-bootstrap from
    /// stored [`crate::market_data::term_structures::RateCalibrationRecipe`] quotes.
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = spec.standard_bump_id(self.id());
        bumped.bump_in_place(&spec)?;
        Ok(bumped)
    }
}
