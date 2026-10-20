//! Curve transformations and market bumps.

use super::*;
use crate::dates::DateExt;
use crate::market_data::bumps::{BumpSpec, BumpType, Bumpable};

impl ForwardCurve {
    /// Create a builder pre-populated with this curve's data but a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> ForwardCurveBuilder {
        let mut builder = self.metadata_builder(new_id).knots(
            self.knots
                .iter()
                .copied()
                .zip(self.forwards.iter().copied()),
        );
        builder.transform = self.transform.clone();
        builder
    }

    /// Rebuild this curve with replacement knots while preserving all metadata.
    ///
    /// This retains the id, tenor, reset lag, day count, interpolation,
    /// extrapolation, projection grid, calibration provenance and FX policy.
    /// Replacement pillars define a new interpolation; previous roll and shock
    /// transformations are not reapplied to them.
    ///
    /// When the replacement pillars reach past the end of the projection grid,
    /// the grid is extended in index-tenor steps until it covers the last
    /// pillar. Those are the boundaries projection-discount-factor chaining
    /// already uses beyond the grid, so the extension changes no projected
    /// rate.
    ///
    /// # Arguments
    ///
    /// * `knots` - Replacement `(time, forward_rate)` pillars: times in year
    ///   fractions from the base date under the curve's day count, rates as
    ///   decimals (0.05 = 5%).
    ///
    /// # Errors
    ///
    /// Returns an error when the replacement knots violate the curve's
    /// validation or interpolation constraints.
    pub fn rebuild_with_knots<I>(&self, knots: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        let knots: Vec<(f64, f64)> = knots.into_iter().collect();
        let last_knot = knots.iter().map(|&(time, _)| time).fold(0.0, f64::max);
        let projection_grid = self.projection_grid.as_deref().map(|grid| {
            let mut grid = grid.to_vec();
            while let Some(&end) = grid.last() {
                if end >= last_knot {
                    break;
                }
                grid.push(end + self.tenor);
            }
            grid
        });
        self.metadata_builder(self.id.clone())
            .projection_grid_opt(projection_grid)
            .knots(knots)
            .build()
    }

    /// Builder pre-populated with this curve's full metadata but **no** knots.
    /// Shared by all rebuild-style operations (bumps, rolls) so that no
    /// metadata field (reset lag, day-count, interpolation, extrapolation,
    /// rate_calibration, fx_policy) is dropped.
    pub(crate) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> ForwardCurveBuilder {
        ForwardCurve::builder(new_id, self.tenor)
            .base_date(self.base)
            .reset_lag(self.reset_lag)
            .day_count(self.day_count)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .projection_grid_opt(self.projection_grid.as_deref().map(<[f64]>::to_vec))
            .rate_calibration_opt(self.rate_calibration.clone())
            .fx_policy_opt(self.fx_policy.clone())
    }

    /// Apply a continuous rate shock while preserving the source interpolator.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        spec.validate()?;
        let (val, is_multiplicative) =
            spec.resolve_standard_values()
                .ok_or_else(|| crate::InputError::UnsupportedBump {
                    reason: format!(
                    "ForwardCurve bump requires Additive or Multiplicative values, got {:?}/{:?}",
                    spec.mode, spec.units
                ),
                })?;

        if is_multiplicative && matches!(spec.bump_type, BumpType::TriangularKeyRate { .. }) {
            return Err(crate::InputError::UnsupportedBump {
                reason: "ForwardCurve key-rate shocks require additive rate units".into(),
            }
            .into());
        }
        if (!is_multiplicative && val == 0.0) || (is_multiplicative && val.total_cmp(&1.0).is_eq())
        {
            return Ok(());
        }
        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| super::evaluation::CurveTransform::from_curve(self));
        transform.bump(val, is_multiplicative, spec.bump_type)?;
        let forwards: Box<[f64]> = self
            .knots
            .iter()
            .map(|&time| transform.rate(&self.interp, time))
            .collect();
        if forwards.iter().any(|value| !value.is_finite()) {
            return Err(crate::Error::Validation(
                "forward-curve shock produced non-finite rates".into(),
            ));
        }
        if let BumpType::TriangularKeyRate { target_bucket, .. } = spec.bump_type {
            if !transform.rate(&self.interp, target_bucket).is_finite() {
                return Err(crate::Error::Validation(
                    "forward-curve shock produced a non-finite peak rate".into(),
                ));
            }
        }
        self.forwards = forwards;
        self.transform = Some(transform);
        Ok(())
    }

    /// Roll the curve forward by a specified number of days.
    ///
    /// This creates a new curve with:
    /// - Base date advanced by `days`
    /// - Knot times shifted backwards (t' = t - dt_years)
    /// - Points with t' <= 0 are filtered out (expired)
    /// - Forward rates are preserved (no carry/theta adjustment)
    ///
    /// This is the "constant curves" or "pure roll-down" scenario where forward
    /// rates at each calendar date remain the same, but maturity times are
    /// re-measured from the new base date.
    ///
    /// # Arguments
    /// * `days` - Signed calendar-day shift; negative values move the base date backward.
    ///
    /// # Returns
    /// A new forward curve with updated base date and shifted knots.
    ///
    /// # Errors
    /// Returns an error if fewer than 2 knot points remain after filtering expired points.
    ///
    /// # Examples
    /// ```
    /// use finstack_quant_core::market_data::term_structures::ForwardCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = ForwardCurve::builder("USD_SOFR_3M", 0.25)
    ///     .base_date(base_date)
    ///     .knots(vec![(0.5, 0.045), (1.0, 0.048), (2.0, 0.050), (5.0, 0.052)])
    ///     .build()
    ///     ?;
    ///
    /// // Roll 6 months forward.
    /// let rolled = curve.roll_forward(182)?;
    /// assert_eq!(rolled.base_date(), date!(2025 - 07 - 02));
    ///
    /// // The rolled curve is re-anchored at t = 0, and every surviving knot
    /// // has moved half a year closer to the new base date.
    /// assert!(rolled.knots()[0].abs() < 1e-12);
    /// assert!(rolled.knots().last().is_some_and(|&t| t < 5.0));
    /// # Ok(())
    /// # }
    /// ```
    /// Returns a validation error if the rolled base date exceeds the supported calendar range.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        if days == 0 {
            return Ok(self.clone());
        }
        let new_base = self.base.add_days(days)?;
        let dt_years = super::super::common::year_fraction_to(self.base, new_base, self.day_count)?;

        // Preserve the live forward at the new origin. Merely shifting and
        // dropping expired knots loses the interpolation segment containing
        // `dt_years` and can materially change the rolled curve at t=0.
        let mut rolled_points = Vec::with_capacity(self.knots.len() + 1);
        rolled_points.push((0.0, self.rate(dt_years)));
        rolled_points.extend(roll_knots(&self.knots, &self.forwards, dt_years));

        if rolled_points.len() < 2 {
            return Err(crate::InputError::TooFewPoints.into());
        }

        let projection_grid = self.projection_grid.as_ref().map(|grid| {
            let mut rolled = Vec::with_capacity(grid.len());
            rolled.push(0.0);
            rolled.extend(
                grid.iter()
                    .copied()
                    .filter(|time| *time > dt_years)
                    .map(|time| time - dt_years),
            );
            rolled
        });

        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| super::evaluation::CurveTransform::from_curve(self));
        transform.offset += dt_years;
        transform.validate()?;
        let mut builder = self
            .metadata_builder(self.id.clone())
            .base_date(new_base)
            .projection_grid_opt(projection_grid)
            .knots(rolled_points);
        builder.transform = Some(transform);
        builder.build()
    }
}

impl Bumpable for ForwardCurve {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = spec.standard_bump_id(self.id());
        bumped.bump_in_place(&spec)?;
        Ok(bumped)
    }
}
