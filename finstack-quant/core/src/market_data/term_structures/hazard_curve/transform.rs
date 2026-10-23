//! Curve transformations and market bumps.

use super::*;
use crate::dates::DateExt;
use crate::market_data::bumps::{BumpSpec, BumpUnits, Bumpable};
use crate::math::units::ONE_BASIS_POINT;

impl HazardCurve {
    /// Create a copy of this curve with its `recovery_rate` metadata overridden.
    ///
    /// All hazard knots (λ), the survival interpolator, day-count, base date,
    /// par spreads and other metadata are preserved **unchanged** — only the
    /// recovery-rate field is replaced. The recovery rate is calibration/
    /// reporting metadata (consulted by the recovery-consistency guard and the
    /// spread↔hazard bootstrap mapping); it is *not* a direct input to the
    /// survival-probability or protection-leg PV math. Overriding it therefore
    /// realigns a "frozen-curve" curve with a trade carrying a different
    /// recovery without altering any priced quantity.
    ///
    /// # Errors
    ///
    /// Returns an error if `recovery_rate` is outside `[0, 1]`.
    pub fn with_recovery_rate(&self, recovery_rate: f64) -> crate::Result<HazardCurve> {
        crate::validation::validate_f64_unit_interval(recovery_rate, "recovery_rate")?;
        let mut curve = self.clone();
        curve.recovery_rate = recovery_rate;
        // The calibration provenance described the original recovery assumption.
        curve.hazard_calibration = None;
        Ok(curve)
    }

    /// Create a builder with this curve's parameters, including its par
    /// spreads and calibration provenance, using a new ID.
    ///
    /// Bumps and rolls deliberately do not carry the provenance: a shocked
    /// or rolled curve no longer reproduces from the stored recipe.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> HazardCurveBuilder {
        self.metadata_builder(new_id)
            .knots(self.knot_points())
            .par_spreads(self.par_spread_points())
            .hazard_calibration_opt(self.hazard_calibration.clone())
    }

    /// Builder pre-populated with this curve's full metadata but **no** knots
    /// or par spreads. Shared by all rebuild-style operations (bumps, rolls)
    /// so that no metadata field (issuer, seniority, currency, day-count,
    /// par interpolation, survival interpolation style, fx_policy) is dropped.
    pub(crate) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> HazardCurveBuilder {
        HazardCurve::builder(new_id)
            .base_date(self.base)
            .recovery_rate(self.recovery_rate)
            .day_count(self.day_count)
            .par_interp(self.par_interp)
            .max_hazard_rate(self.max_hazard_rate)
            .issuer_opt(self.issuer.clone())
            .seniority_opt(self.seniority)
            .currency_opt(self.currency)
            .fx_policy_opt(self.fx_policy.clone())
    }

    /// Clone the curve under `id` and apply `spec` through [`Self::bump_in_place`],
    /// the single bump implementation; the `with_*` helpers and `Bumpable`
    /// are thin wrappers over this.
    pub(crate) fn bumped_with_id(
        &self,
        id: CurveId,
        spec: &BumpSpec,
        target: HazardBumpTarget,
    ) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = id;
        bumped.bump_in_place(spec, target)?;
        Ok(bumped)
    }

    /// Apply a bump specification in-place, mutating lambda values and rebuilding the interpolator.
    ///
    /// For [`HazardBumpTarget::ParSpread`] the spread shift `s` is converted
    /// to a hazard shift `s / (1 − R)` (credit triangle); for
    /// [`HazardBumpTarget::HazardRate`] the shift is applied to the
    /// intensities as given and must stay within the curve's hazard cap. A
    /// parallel bump adds it to every hazard rate; a triangular key-rate bump
    /// scales it at each knot by the bucket's triangular weight, so the
    /// bucket shifts sum to the parallel shift at every knot.
    pub(crate) fn bump_in_place(
        &mut self,
        spec: &BumpSpec,
        target: HazardBumpTarget,
    ) -> crate::Result<()> {
        let spread = spec.resolve_additive_or_error("HazardCurve")?;

        let shift = match target {
            HazardBumpTarget::ParSpread => {
                // Recovery must be within [0, 1) for the par spread ⇢ hazard
                // conversion; recovery == 1.0 would divide by zero and yield an
                // infinite shift.
                crate::validation::validate_recovery_rate(self.recovery_rate)?;
                spread / (1.0 - self.recovery_rate)
            }
            // One basis point of intensity is 1e-4, kept as the exact product.
            HazardBumpTarget::HazardRate if spec.units == BumpUnits::RateBp => {
                spec.value * ONE_BASIS_POINT
            }
            HazardBumpTarget::HazardRate => spread,
        };
        // Clone the hazard array only, not the whole curve. Early returns below
        // still leave `self` untouched, and nothing is committed until the
        // fallible interpolator build succeeds.
        let mut lambdas = self.lambdas.clone();
        for (lambda, &t) in lambdas.iter_mut().zip(self.knots.iter()) {
            let weight = spec.bump_type.weight(t);
            let shifted = *lambda + shift * weight;
            if !shifted.is_finite() || shifted < 0.0 {
                return Err(crate::InputError::UnsupportedBump {
                    reason: "non-finite or negative hazard rate after bump".to_string(),
                }
                .into());
            }
            if target == HazardBumpTarget::HazardRate && shifted > self.max_hazard_rate {
                return Err(crate::Error::Validation(format!(
                    "Hazard rate {shifted:.4} at t={t:.4}y exceeds maximum {:.4} after bump",
                    self.max_hazard_rate
                )));
            }
            *lambda = shifted;
        }
        let (interp_knots, interp_sp) = survival_pillars(&self.knots, &lambdas);
        let interp = InterpStyle::LogLinear.build_enum(
            interp_knots.into_boxed_slice(),
            interp_sp.into_boxed_slice(),
            ExtrapolationPolicy::FlatForward,
            ValidationPolicy::Strict,
        )?;
        self.lambdas = lambdas;
        // The stored par-spread quotes were calibrated to the *unbumped*
        // hazards; keeping them would make `cds_quote_bp` report stale quotes.
        // Clear them so `cds_quote_bp` falls back to the hazard-based
        // approximation λ·(1−R)·1e4, which reflects the bumped curve.
        self.par_tenors = Box::new([]);
        self.par_spreads_bp = Box::new([]);
        self.hazard_calibration = None;
        self.interp = interp;
        Ok(())
    }
    /// Create a curve with every model hazard rate shifted in basis-point units.
    ///
    /// This is the hazard-curve equivalent of a parallel `BumpSpec::parallel_bp` shock on `DiscountCurve`:
    /// a direct intensity shock that keeps the curve ID and all credit
    /// metadata; it does not replay CDS par quotes. Stored par-spread and
    /// calibration recipes are intentionally removed because they describe
    /// the pre-bump calibration, so [`Self::cds_quote_bp`] reports its
    /// hazard-based approximation for the bumped curve.
    ///
    /// # Arguments
    ///
    /// * `bump_bp` - Additive hazard-rate shift in basis points, where one
    ///   basis point is `1e-4` in decimal intensity units. Negative shifts
    ///   that would make any hazard rate negative are rejected.
    ///
    /// # Errors
    ///
    /// Returns an error when the shock is non-finite, makes a hazard rate
    /// negative, or makes it exceed the curve's hazard-rate cap.
    pub fn with_parallel_hazard_rate_bump_bp(&self, bump_bp: f64) -> crate::Result<Self> {
        self.bumped_with_id(
            self.id.clone(),
            &BumpSpec::parallel_bp(bump_bp),
            HazardBumpTarget::HazardRate,
        )
    }

    /// Create a curve with direct hazard-rate shocks at tenor segments.
    ///
    /// An exact knot match shocks that knot. Otherwise the segment containing
    /// the requested tenor is shocked; targets beyond the final knot are a
    /// no-op. A curve with a single knot has no segment to shock, so every
    /// tenor shock is applied to that knot as one parallel bump. Requests are
    /// applied in order, so repeated targets are additive.
    /// Stored par-spread and calibration recipes are intentionally removed.
    ///
    /// # Arguments
    ///
    /// * `targets_bp` - Ordered `(tenor_years, bump_bp)` shocks. Tenors are
    ///   measured from the curve base date and bumps are additive hazard-rate
    ///   basis points.
    ///
    /// # Errors
    ///
    /// Returns an error when a tenor or bump is non-finite, the curve is
    /// malformed, any requested shock produces a negative or non-finite
    /// hazard rate, or rebuilding the shocked curve fails validation.
    pub fn with_tenor_hazard_rate_bumps_bp(
        &self,
        targets_bp: &[(f64, f64)],
    ) -> crate::Result<Self> {
        let knots: Vec<f64> = self.knot_points().map(|(tenor, _)| tenor).collect();
        let mut hazard_rates: Vec<f64> = self
            .knot_points()
            .map(|(_, hazard_rate)| hazard_rate)
            .collect();
        if knots.len() < 2 {
            let total_bp = targets_bp.iter().try_fold(0.0, |total, (tenor, bump_bp)| {
                if !tenor.is_finite() || !bump_bp.is_finite() {
                    return Err(crate::InputError::UnsupportedBump {
                        reason: "hazard-rate tenor and basis-point bump must be finite".to_string(),
                    });
                }
                Ok(total + bump_bp)
            })?;
            return self.with_parallel_hazard_rate_bump_bp(total_bp);
        }

        let last_knot = knots[knots.len() - 1];
        for (tenor, bump_bp) in targets_bp {
            if !tenor.is_finite() || !bump_bp.is_finite() {
                return Err(crate::InputError::UnsupportedBump {
                    reason: "hazard-rate tenor and basis-point bump must be finite".to_string(),
                }
                .into());
            }
            if *tenor > last_knot + 1e-6 {
                continue;
            }
            let target_index = knots
                .iter()
                .position(|knot| (*knot - tenor).abs() <= 1e-6)
                .unwrap_or_else(|| {
                    knots
                        .partition_point(|&knot| knot < *tenor)
                        .min(knots.len() - 1)
                });
            let shifted = hazard_rates[target_index] + bump_bp * ONE_BASIS_POINT;
            if !shifted.is_finite() || shifted < 0.0 {
                return Err(crate::InputError::UnsupportedBump {
                    reason: "non-finite or negative hazard rate after tenor bump".to_string(),
                }
                .into());
            }
            hazard_rates[target_index] = shifted;
        }

        self.metadata_builder(self.id.clone())
            .knots(knots.into_iter().zip(hazard_rates))
            .build()
    }

    /// Roll the curve forward by a specified number of days.
    ///
    /// This creates a new curve with:
    /// - Base date advanced by `days`
    /// - Knot times shifted backwards (t' = t - dt_years)
    /// - Points with t' <= 0 are filtered out (expired)
    /// - Hazard rates are preserved (no carry/theta adjustment)
    ///
    /// # Arguments
    /// * `days` - Signed calendar-day shift; negative values move the base date backward.
    ///
    /// # Returns
    /// A new hazard curve with updated base date and shifted knots.
    ///
    /// # Errors
    ///
    /// Returns an error if the day-count calculation fails, fewer than two
    /// knot points remain after the roll, or the rebuilt curve violates a
    /// construction invariant. `days` is signed; a negative value moves the
    /// base date backward rather than rejecting the request.
    /// Returns a validation error if the rolled base date exceeds the supported calendar range.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base.add_days(days)?;
        let dt_years = self.day_count.signed_year_fraction(
            self.base,
            new_base,
            crate::dates::DayCountContext::default(),
        )?;

        // Anchor the active post-roll hazard at the new origin, then retain
        // future hazard changes. This preserves conditional survival rather
        // than re-attributing the first surviving lambda to the whole front
        // segment.
        let mut rolled_points = Vec::with_capacity(self.knots.len() + 1);
        rolled_points.push((0.0, self.hazard_rate(dt_years)));
        rolled_points.extend(crate::market_data::term_structures::common::roll_knots(
            &self.knots,
            &self.lambdas,
            dt_years,
        ));

        if rolled_points.len() < 2 {
            return Err(crate::InputError::TooFewPoints.into());
        }

        let rolled_par_points = crate::market_data::term_structures::common::roll_knots(
            &self.par_tenors,
            &self.par_spreads_bp,
            dt_years,
        );

        let mut builder = self
            .metadata_builder(self.id.clone())
            .base_date(new_base)
            .knots(rolled_points);

        if !rolled_par_points.is_empty() {
            builder = builder.par_spreads(rolled_par_points);
        }

        builder.build()
    }
}

impl Bumpable for HazardCurve {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        // `bump_in_place` interprets RateBp/Percent as **par spread** shocks
        // (converted to hazard via 1/(1 - recovery)), rejects bumps that would
        // drive a hazard rate negative, and drops the stored par-spread quotes
        // that were calibrated to the unbumped hazards.
        self.bumped_with_id(
            spec.hazard_shift_id(self.id()),
            &spec,
            HazardBumpTarget::ParSpread,
        )
    }
}
