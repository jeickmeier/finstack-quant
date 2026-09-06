//! Curve transformations and market bumps.

use super::*;
use crate::error::InputError;
use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};

impl ForwardCurve {
    /// Create a builder pre-populated with this curve's data but a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> ForwardCurveBuilder {
        self.metadata_builder(new_id).knots(
            self.knots
                .iter()
                .copied()
                .zip(self.forwards.iter().copied()),
        )
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

    /// Apply a bump specification in-place, mutating values and rebuilding the interpolator.
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

        // Clone the value array only, not the whole curve -- the interpolator
        // rebuild below discards any cloned interpolator anyway. Nothing is
        // written to `self` until the fallible rebuild succeeds, so failure
        // atomicity is unchanged.
        let mut forwards = self.forwards.clone();
        match spec.bump_type {
            BumpType::Parallel => {
                if is_multiplicative {
                    for fwd in forwards.iter_mut() {
                        *fwd *= val;
                    }
                } else {
                    for fwd in forwards.iter_mut() {
                        *fwd += val;
                    }
                }
            }
            BumpType::TriangularKeyRate {
                prev_bucket,
                target_bucket,
                next_bucket,
            } => {
                // Reject malformed bucket grids (e.g. infinite sentinels)
                // before mutating: a non-finite neighbour yields NaN weights
                // and corrupts the curve.
                crate::market_data::term_structures::common::validate_triangular_bucket_grid(
                    prev_bucket,
                    target_bucket,
                    next_bucket,
                )?;
                for (fwd, &t) in forwards.iter_mut().zip(self.knots.iter()) {
                    let weight = crate::market_data::term_structures::common::triangular_weight(
                        t,
                        prev_bucket,
                        target_bucket,
                        next_bucket,
                    );
                    if is_multiplicative {
                        *fwd *= 1.0 + (val - 1.0) * weight;
                    } else {
                        *fwd += val * weight;
                    }
                }
            }
        }
        let interp = crate::market_data::term_structures::common::build_interp_allow_any_values(
            self.interp.style(),
            self.knots.clone(),
            forwards.clone(),
            self.interp.extrapolation(),
        )?;
        self.forwards = forwards;
        self.interp = interp;
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
    /// * `days` - Number of days to roll forward
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
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        let dt_years =
            self.day_count
                .year_fraction(self.base, new_base, DayCountContext::default())?;

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

        self.metadata_builder(self.id.clone())
            .base_date(new_base)
            .projection_grid_opt(projection_grid)
            .knots(rolled_points)
            .build()
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
