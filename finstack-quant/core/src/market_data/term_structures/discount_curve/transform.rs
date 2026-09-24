//! Discount-curve bumping and roll-forward transformations.

use super::super::common::roll_knots;
use super::DiscountCurve;
use crate::dates::DayCountContext;
use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};

impl DiscountCurve {
    /// Clone the curve under `id` and apply `spec` through [`Self::bump_in_place`],
    /// the single bump implementation; the `with_*` helpers and `Bumpable`
    /// are thin wrappers over this.
    pub(crate) fn bumped_with_id(
        &self,
        id: crate::types::CurveId,
        spec: &BumpSpec,
    ) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = id;
        bumped.bump_in_place(spec)?;
        Ok(bumped)
    }

    /// Apply a bump specification in-place, mutating values and rebuilding the interpolator.
    ///
    /// Additive bumps are **continuously compounded zero-space** shocks
    /// (`DF *= exp(−δr · t)`), not quote re-bootstraps. This avoids allocating
    /// intermediate `Vec<(f64, f64)>`, skips ID generation, and avoids
    /// sorting unchanged knots while rechecking discount-factor validation.
    ///
    /// # Performance
    ///
    /// Clones the value array and the interpolator's consumed knot/value inputs,
    /// but avoids cloning the full curve and its calibration recipe.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        spec.validate_finite()?;
        let (val, is_multiplicative) = spec.resolve_standard_values().ok_or_else(|| {
            crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "DiscountCurve only supports Additive/{{RateBp,Percent,Fraction}} bumps, got {:?}/{:?}",
                    spec.mode, spec.units
                ),
            }
        })?;
        if is_multiplicative {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "DiscountCurve does not support Multiplicative bumps".to_string(),
            }
            .into());
        }
        let bump_rate = val;

        // Continuously compounded zero-space shock: DF *= exp(-δr t).
        // Clone only values; assign after the fallible interpolator build to
        // preserve failure atomicity.
        let mut dfs = self.dfs.clone();
        match spec.bump_type {
            BumpType::Parallel => {
                for (df, &t) in dfs.iter_mut().zip(self.knots.iter()) {
                    *df *= (-bump_rate * t).exp();
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
                super::super::common::validate_triangular_bucket_grid(
                    prev_bucket,
                    target_bucket,
                    next_bucket,
                )?;
                for (df, &t) in dfs.iter_mut().zip(self.knots.iter()) {
                    let weight = super::super::common::triangular_weight(
                        t,
                        prev_bucket,
                        target_bucket,
                        next_bucket,
                    );
                    *df *= (-bump_rate * weight * t).exp();
                }
            }
        }
        if dfs.iter().any(|df| !df.is_finite()) {
            return Err(crate::Error::Validation(
                "discount-curve shock produced non-finite discount factors".into(),
            ));
        }
        if dfs.iter().any(|df| *df <= 0.0) {
            return Err(crate::error::InputError::NonPositiveValue.into());
        }
        let interp = super::super::common::build_interp_input_error(
            self.style,
            self.knots.clone(),
            dfs.clone(),
            self.extrapolation,
            true,
        )?;
        self.dfs = dfs;
        self.interp = interp;
        // Stress curves retain mathematical validation on JSON round-trips.
        // The original construction policy remains unchanged on `self`'s
        // source curve when using the public cloning bump methods.
        self.allow_non_monotonic = true;
        self.min_forward_rate = None;
        Ok(())
    }

    /// Create a new curve with a parallel rate bump applied in basis points (fallible).
    ///
    /// Uses `DF_bumped(t) = DF(t) · exp(−δr · t)` with `δr = bp / 10_000`.
    /// This is a **continuously compounded zero-space** parallel shock of the
    /// already-built curve, not a re-bootstrap of the original market quotes.
    ///
    /// # Arguments
    ///
    /// * `bp` - Finite additive continuously compounded zero-rate shock in
    ///   basis points; -100 lowers rates by one percentage point.
    ///
    /// # Errors
    ///
    /// Returns an error for non-finite shocks, non-finite/non-positive discount
    /// factors, or invalid interpolation. Negative rates are permitted under
    /// stress independently of the source curve's construction policy. The
    /// shocked curve serializes with permissive rate-policy settings.
    pub fn with_parallel_bump(&self, bp: f64) -> crate::Result<Self> {
        self.bumped_with_id(
            crate::market_data::bumps::id_bump_bp(self.id.as_str(), bp),
            &BumpSpec::parallel_bp(bp),
        )
    }

    /// Create a new curve with a triangular key-rate bump using explicit bucket neighbors.
    ///
    /// This is the market-standard key-rate DV01 implementation (per Tuckman/Fabozzi)
    /// where the triangular weight is defined by the **bucket grid**, not curve knots.
    /// This ensures that the sum of all bucketed DV01s equals the parallel DV01.
    ///
    /// # Mathematical Foundation
    ///
    /// For a zero rate bump δr applied with triangular weight w(t):
    /// ```text
    /// DF_bumped(t) = DF(t) × exp(-w(t) × δr × t)
    /// ```
    ///
    /// The triangular weight function for an **interior** bucket at `target`
    /// with neighbours `prev = Some(p)` and `next = Some(n)`:
    /// - w(t) = 0                                    if t ≤ p
    /// - w(t) = (t − p) / (target − p)               if p < t ≤ target
    /// - w(t) = (n − t) / (n − target)               if target < t < n
    /// - w(t) = 0                                    if t ≥ n
    ///
    /// For the **first bucket** (`prev = None`) the rising edge is replaced
    /// by a flat 1.0 for `t ≤ target`; for the **last bucket**
    /// (`next = None`) the falling edge is replaced by a flat 1.0 for
    /// `t > target`.
    ///
    /// # Key Property: Unity Partition
    ///
    /// When `prev = None` is used for the first bucket and `next = None`
    /// for the last bucket, the weights of the full bucket set sum to 1.0
    /// at any time t covered by any bucket:
    /// `Σᵢ wᵢ(t) = 1.0`
    ///
    /// This ensures: **sum of bucketed DV01 = parallel DV01**.
    ///
    /// # Arguments
    /// * `prev_bucket` - Previous bucket time in years; `None` for the first bucket
    /// * `target_bucket` - Target bucket time in years (peak of the triangle)
    /// * `next_bucket` - Next bucket time in years; `None` for the last bucket
    /// * `bp` - Bump size in basis points (100bp = 1%)
    ///
    /// # Returns
    /// A new discount curve with the triangular key-rate bump applied.
    ///
    /// # Errors
    /// Returns an error for invalid bucket coordinates, non-finite shocks,
    /// non-finite/non-positive discount factors, or invalid interpolation.
    /// Negative rates are allowed independently of construction-policy floors.
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
    ///     .knots(vec![(1.0, 0.98), (2.0, 0.96), (5.0, 0.90), (10.0, 0.80)])
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
    pub fn with_triangular_key_rate_bump_neighbors(
        &self,
        prev_bucket: Option<f64>,
        target_bucket: f64,
        next_bucket: Option<f64>,
        bp: f64,
    ) -> crate::Result<Self> {
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
    /// * `days` - Number of days to roll forward
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
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        let dt_years =
            self.day_count
                .year_fraction(self.base, new_base, DayCountContext::default())?;

        // Realized-forward renormalization: divide every rolled DF by the
        // old curve's DF at the roll horizon, interpolated in the curve's own
        // time basis (the same `dt_years` the knots are shifted by).
        let df_dt = self.df(dt_years);
        if !df_dt.is_finite() || df_dt <= 0.0 {
            return Err(crate::error::InputError::NonPositiveValue.into());
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
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        // Thread the full metadata (including fx_policy) and override the base.
        self.metadata_builder(self.id.clone())
            .base_date(new_base)
            .knots(rolled_points)
            .build()
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
        let (val, is_multiplicative) = spec.resolve_standard_values_or_error(
            "DiscountCurve",
            "only supports Additive/{RateBp,Percent,Fraction} bumps",
        )?;

        if is_multiplicative {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "DiscountCurve does not support Multiplicative bumps".to_string(),
            }
            .into());
        }

        // Internal methods take basis points; `val` is a decimal rate.
        let bp = val * 10_000.0;

        match spec.bump_type {
            BumpType::Parallel => self.with_parallel_bump(bp),
            BumpType::TriangularKeyRate {
                prev_bucket,
                target_bucket,
                next_bucket,
            } => self.with_triangular_key_rate_bump_neighbors(
                prev_bucket,
                target_bucket,
                next_bucket,
                bp,
            ),
        }
    }
}
