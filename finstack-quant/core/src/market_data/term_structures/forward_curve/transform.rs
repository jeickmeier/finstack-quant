//! Curve transformations and market bumps.

use super::*;
use crate::dates::DateExt;
use crate::error::InputError;
use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};

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

    /// Create a new curve with a triangular key-rate bump using explicit bucket neighbors.
    ///
    /// This is the market-standard key-rate DV01 implementation (per Tuckman/Fabozzi)
    /// where the triangular weight is defined by the **bucket grid**, not curve knots.
    /// This ensures that the sum of all bucketed DV01s equals the parallel DV01.
    ///
    /// # Mathematical Foundation
    ///
    /// The triangular weight function for bucket at `target` with neighbors `prev` and `next`:
    /// - w(t) = 0                                    if t ≤ prev
    /// - w(t) = (t - prev) / (target - prev)        if prev < t ≤ target
    /// - w(t) = (next - t) / (next - target)        if target < t < next
    /// - w(t) = 0                                    if t ≥ next
    ///
    /// The forward rate is then bumped: `rate_bumped = rate + bump * weight`
    ///
    /// # Key Property: Unity Partition
    ///
    /// For any time t, the sum of all bucket weights equals 1.0:
    /// `Σᵢ wᵢ(t) = 1.0`
    ///
    /// This ensures: **sum of bucketed DV01 = parallel DV01**
    ///
    /// # Arguments
    /// * `prev_bucket` - Previous bucket time in years (`None` for the first bucket)
    /// * `target_bucket` - Target bucket time in years (peak of the triangle)
    /// * `next_bucket` - Next bucket time in years (`None` for the last bucket;
    ///   never pass `f64::INFINITY` — non-finite bounds are rejected)
    /// * `bp` - Bump size in basis points (100bp = 1%)
    ///
    /// # Returns
    /// A new forward curve with the triangular key-rate bump applied.
    ///
    /// # Errors
    /// Returns an error if the bumped curve violates validation constraints.
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
    ///     .knots(vec![(1.0, 0.045), (2.0, 0.048), (5.0, 0.050), (10.0, 0.052)])
    ///     .build()
    ///     ?;
    ///
    /// // Apply 10bp bump at 5Y interior bucket with neighbours at 3Y and 7Y
    /// let bumped = curve.with_triangular_key_rate_bump_neighbors(
    ///     Some(3.0), 5.0, Some(7.0), 10.0,
    /// )?;
    /// # let _ = bumped;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// `prev_bucket = None` makes this the first bucket (flat-left half
    /// triangle); `next_bucket = None` makes this the last bucket
    /// (flat-right half triangle). These preserve the unity-partition
    /// invariant at the wings.
    pub fn with_triangular_key_rate_bump_neighbors(
        &self,
        prev_bucket: Option<f64>,
        target_bucket: f64,
        next_bucket: Option<f64>,
        bp: f64,
    ) -> crate::Result<Self> {
        use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits};

        self.bumped_with_id(
            crate::market_data::bumps::id_bump_bp(self.id.as_str(), bp),
            &BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::RateBp,
                value: bp,
                bump_type: BumpType::TriangularKeyRate {
                    prev_bucket,
                    target_bucket,
                    next_bucket,
                },
            },
        )
    }

    /// Clone the curve under `id` and apply `spec` through [`Self::bump_in_place`],
    /// the single bump implementation; the `with_*` helpers and `Bumpable`
    /// are thin wrappers over this.
    pub(crate) fn bumped_with_id(&self, id: CurveId, spec: &BumpSpec) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = id;
        bumped.bump_in_place(spec)?;
        Ok(bumped)
    }

    /// Apply a continuous rate shock while preserving the source interpolator.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        use BumpType;

        spec.validate_finite()?;
        let (val, is_multiplicative) = spec.resolve_standard_values().ok_or_else(|| {
            crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "ForwardCurve bump requires Additive or Multiplicative values, got {:?}/{:?}",
                    spec.mode, spec.units
                ),
            }
        })?;

        if let BumpType::TriangularKeyRate {
            prev_bucket,
            target_bucket,
            next_bucket,
        } = spec.bump_type
        {
            crate::market_data::term_structures::common::validate_triangular_bucket_grid(
                prev_bucket,
                target_bucket,
                next_bucket,
            )?;
            if is_multiplicative {
                return Err(crate::error::InputError::UnsupportedBump {
                    reason: "ForwardCurve key-rate shocks require additive rate units".into(),
                }
                .into());
            }
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

    /// Create a new curve with a parallel rate bump in basis points.
    ///
    /// Adds `bp / 10,000` to every stored simple forward rate, such that
    /// `1.0` is a one-basis-point shock. The result retains the date, tenor,
    /// reset-lag, interpolation, extrapolation, projection-grid, calibration,
    /// and FX-policy metadata, but receives a derived identifier.
    ///
    /// This is a direct fitted-curve shock: attached calibration quotes and
    /// recipes are metadata and are not re-bootstrapped.
    ///
    /// # Errors
    ///
    /// Returns an error if the rebuilt curve cannot validate its knots or
    /// construct its interpolation. In particular, callers should supply a
    /// finite bump and should not assume this method enforces a floor on
    /// negative forward rates; negative rates are permitted by the curve type.
    pub fn with_parallel_bump(&self, bp: f64) -> crate::Result<Self> {
        self.bumped_with_id(
            crate::market_data::bumps::id_bump_bp(self.id.as_str(), bp),
            &BumpSpec::parallel_bp(bp),
        )
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
            return Err(crate::error::InputError::TooFewPoints.into());
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
        match spec.bump_type {
            BumpType::Parallel => {
                spec.resolve_standard_values_or_error(
                    "ForwardCurve parallel bump requires",
                    "Additive/{RateBp,Percent,Fraction} or Multiplicative/Factor",
                )?;
                self.bumped_with_id(spec.standard_bump_id(self.id()), &spec)
            }
            BumpType::TriangularKeyRate {
                prev_bucket,
                target_bucket,
                next_bucket,
            } => {
                if spec.mode == BumpMode::Additive && spec.units == BumpUnits::RateBp {
                    self.with_triangular_key_rate_bump_neighbors(
                        prev_bucket,
                        target_bucket,
                        next_bucket,
                        spec.value,
                    )
                } else {
                    Err(InputError::UnsupportedBump {
                        reason: format!(
                            "ForwardCurve key-rate bump requires Additive/RateBp, got {:?}/{:?}",
                            spec.mode, spec.units
                        ),
                    }
                    .into())
                }
            }
        }
    }
}
