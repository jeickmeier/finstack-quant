//! Curve transformations and market bumps.

use super::*;
use crate::market_data::bumps::{BumpSpec, BumpType, Bumpable};

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
        crate::market_data::term_structures::common::validate_unit_range(
            recovery_rate,
            "recovery_rate",
        )?;
        Ok(HazardCurve {
            id: self.id.clone(),
            base: self.base,
            knots: self.knots.clone(),
            lambdas: self.lambdas.clone(),
            recovery_rate,
            issuer: self.issuer.clone(),
            seniority: self.seniority,
            currency: self.currency,
            day_count: self.day_count,
            par_tenors: self.par_tenors.clone(),
            par_spreads_bp: self.par_spreads_bp.clone(),
            par_interp: self.par_interp,
            survival_interp_style: self.survival_interp_style,
            hazard_calibration: None,
            interp: self.interp.clone(),
            fx_policy: self.fx_policy.clone(),
        })
    }

    /// Create a builder with this curve's parameters, using a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> HazardCurveBuilder {
        self.metadata_builder(new_id)
            .knots(self.knot_points())
            .par_spreads(self.par_spread_points())
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
            .interp(self.survival_interp_style)
            .issuer_opt(self.issuer.clone())
            .seniority_opt(self.seniority)
            .currency_opt(self.currency)
            .fx_policy_opt(self.fx_policy.clone())
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

    /// Apply a bump specification in-place, mutating lambda values and rebuilding the interpolator.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        use BumpType;

        spec.validate_finite()?;
        if !matches!(spec.bump_type, BumpType::Parallel) {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "HazardCurve only supports Parallel bumps, not key-rate bumps".to_string(),
            }
            .into());
        }

        // Recovery must be within [0, 1) for the par spread ⇢ hazard
        // conversion below; recovery == 1.0 would divide by zero and yield an
        // infinite shift. Mirrors the guard in `Bumpable::apply_bump`.
        let recovery = self.recovery_rate;
        if !recovery.is_finite() || !(0.0..1.0).contains(&recovery) {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "HazardCurve bump requires recovery rate in [0, 1), got {}",
                    recovery
                ),
            }
            .into());
        }
        let (spread, is_multiplicative) = spec.resolve_standard_values().ok_or_else(|| {
            crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "HazardCurve only supports Additive bumps, got {:?}/{:?}",
                    spec.mode, spec.units
                ),
            }
        })?;
        if is_multiplicative {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "HazardCurve does not support Multiplicative bumps".to_string(),
            }
            .into());
        }

        let shift = spread / (1.0 - recovery);
        // Clone the hazard array only, not the whole curve. Early returns below
        // still leave `self` untouched, and nothing is committed until the
        // fallible interpolator build succeeds.
        let mut lambdas = self.lambdas.clone();
        for lambda in lambdas.iter_mut() {
            let shifted = *lambda + shift;
            if !shifted.is_finite() || shifted < 0.0 {
                return Err(crate::error::InputError::UnsupportedBump {
                    reason: "non-finite or negative hazard rate after bump".to_string(),
                }
                .into());
            }
            *lambda = shifted;
        }
        let (interp_knots, interp_sp) = survival_pillars(&self.knots, &lambdas);
        let interp = crate::market_data::term_structures::common::build_interp(
            self.survival_interp_style,
            interp_knots.into_boxed_slice(),
            interp_sp.into_boxed_slice(),
            ExtrapolationPolicy::FlatForward,
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
    /// This is the hazard-curve equivalent of `DiscountCurve::with_parallel_bump`:
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
    /// negative, or makes it exceed the builder's maximum hazard-rate limit
    /// (10.0 unless the source curve was built with a different limit, which
    /// is not retained by this method).
    pub fn with_parallel_hazard_rate_bump_bp(&self, bump_bp: f64) -> crate::Result<Self> {
        if !bump_bp.is_finite() {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "hazard-rate basis-point bump must be finite".to_string(),
            }
            .into());
        }
        let shift = bump_bp * 1e-4;
        let mut shifted_points = Vec::with_capacity(self.knots.len());
        for (t, lambda) in self.knot_points() {
            let shifted = lambda + shift;
            if shifted < 0.0 {
                return Err(crate::error::InputError::UnsupportedBump {
                    reason: "negative hazard rate after bump".to_string(),
                }
                .into());
            }
            shifted_points.push((t, shifted));
        }
        self.metadata_builder(self.id.clone())
            .knots(shifted_points)
            .build()
    }

    /// Create a curve with direct hazard-rate shocks at tenor segments.
    ///
    /// An exact knot match shocks that knot. Otherwise the segment containing
    /// the requested tenor is shocked; targets beyond the final knot are a
    /// no-op. Requests are applied in order, so repeated targets are additive.
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
    /// malformed, or rebuilding the shocked curve fails validation.
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
                    return Err(crate::error::InputError::UnsupportedBump {
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
                return Err(crate::error::InputError::UnsupportedBump {
                    reason: "hazard-rate tenor and basis-point bump must be finite".to_string(),
                }
                .into());
            }
            if *tenor > last_knot + 1e-6 {
                continue;
            }
            let mut target_index = knots
                .iter()
                .position(|knot| (*knot - tenor).abs() <= 1e-6)
                .unwrap_or(0);
            if target_index == 0 {
                if *tenor <= knots[0] {
                    target_index = 0;
                } else if *tenor >= last_knot {
                    target_index = knots.len() - 1;
                } else if let Some(index) = (0..knots.len() - 1)
                    .find(|index| *tenor > knots[*index] && *tenor < knots[*index + 1])
                {
                    target_index = index;
                }
            }
            hazard_rates[target_index] = (hazard_rates[target_index] + bump_bp * 1e-4).max(0.0);
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
    /// * `days` - Number of days to roll forward
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
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        let dt_years =
            self.day_count
                .year_fraction(self.base, new_base, DayCountContext::default())?;

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
            return Err(crate::error::InputError::TooFewPoints.into());
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
        spec.validate_parallel("HazardCurve")?;
        spec.resolve_standard_values_or_error(
            "HazardCurve",
            "only supports Additive/{RateBp,Percent,Fraction} bumps",
        )?;

        // `bump_in_place` interprets RateBp/Percent as **par spread** shocks
        // (converted to hazard via 1/(1 - recovery)), rejects bumps that would
        // drive a hazard rate negative, and drops the stored par-spread quotes
        // that were calibrated to the unbumped hazards.
        self.bumped_with_id(spec.hazard_shift_id(self.id()), &spec)
    }
}
