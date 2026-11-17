//! Curve transformations and direct parameter shocks for [`HazardCurve`].

use super::super::common::signed_curve_year_fraction;
use super::builder::surviving_source_knots;
use super::*;
use crate::market_data::bumps::{BumpSpec, Bumpable};
use crate::math::units::ONE_BASIS_POINT;

impl HazardCurve {
    /// Return a copy of this curve with its `recovery_rate` metadata replaced.
    ///
    /// Hazard intensities, source clock, and survival/default probabilities remain
    /// unchanged. An exact unchanged `recovery_rate` preserves calibration
    /// provenance; a distinct recovery clears `hazard_calibration` because the
    /// stored recipe no longer certifies a calibration at the new recovery.
    ///
    /// # Arguments
    ///
    /// * `recovery_rate` - Replacement recovery rate in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `recovery_rate` is non-finite or outside `[0, 1]`.
    pub fn with_recovery_rate(&self, recovery_rate: f64) -> crate::Result<HazardCurve> {
        crate::validation::validate_f64_unit_interval(recovery_rate, "recovery_rate")?;
        let mut curve = self.clone();
        if !exact_f64_eq(recovery_rate, self.recovery_rate) {
            curve.recovery_rate = recovery_rate;
            curve.hazard_calibration = None;
        }
        Ok(curve)
    }

    /// Create a builder pre-populated with this curve's state under `new_id`.
    ///
    /// # Arguments
    ///
    /// * `new_id` - Identifier assigned to the rebuilt curve.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> HazardCurveBuilder {
        let mut builder = self
            .metadata_builder(new_id)
            .knots(self.knot_points())
            .hazard_calibration_opt(self.hazard_calibration.clone());
        builder.source = self.source.clone();
        builder
    }

    /// Builder pre-populated with this curve's structural metadata and no knots.
    pub(crate) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> HazardCurveBuilder {
        HazardCurve::builder(new_id)
            .base_date(self.base)
            .day_count(self.day_count)
            .calendar_id_opt(self.calendar.as_ref().map(|cal| cal.id.clone()))
            .recovery_rate(self.recovery_rate)
            .issuer_opt(self.issuer.clone())
            .seniority_opt(self.seniority)
            .currency_opt(self.currency)
            .fx_policy_opt(self.fx_policy.clone())
    }

    /// Clone under `id` and apply direct intensity parameter shock `spec`.
    pub(crate) fn bumped_with_id(&self, id: CurveId, spec: &BumpSpec) -> crate::Result<Self> {
        let mut bumped = self.clone();
        bumped.id = id;
        bumped.bump_in_place(spec)?;
        Ok(bumped)
    }

    /// Apply a direct intensity parameter shock in place.
    ///
    /// Supports additive `RateBp`, `Percent`, and `Fraction` shocks in annual
    /// decimal intensity units (`RateBp` = `1e-4`, `Percent` = `1e-2`, `Fraction`
    /// = `1.0`). Triangular shocks sample bucket weights at the stored ending
    /// parameters on the original source clock without inserting breakpoints.
    /// Zero valid shifts preserve existing state and `hazard_calibration`.
    pub(crate) fn bump_in_place(&mut self, spec: &BumpSpec) -> crate::Result<()> {
        let shift = spec.resolve_additive_or_error("HazardCurve")?;
        if exact_f64_eq(shift, 0.0) {
            return Ok(());
        }

        if let Some(src) = &self.source {
            let mut next_src_knots = Vec::with_capacity(src.knot_points.len());
            for &(source_t, source_lambda) in &src.knot_points {
                let weight = spec.bump_type.weight(source_t);
                let shifted = source_lambda + shift * weight;
                if !shifted.is_finite() || shifted < 0.0 {
                    return Err(crate::InputError::UnsupportedBump {
                        reason: "non-finite or negative hazard rate after bump".to_string(),
                    }
                    .into());
                }
                next_src_knots.push((source_t, shifted));
            }
            let surviving = surviving_source_knots(&next_src_knots, src.offset);
            if surviving.len() != self.knots.len()
                || surviving
                    .iter()
                    .zip(self.knots.iter())
                    .any(|(&(t, _), &k)| !exact_f64_eq(t, k))
            {
                return Err(crate::Error::Validation(
                    "conditioned hazard bump knot structure diverged from source".to_string(),
                ));
            }
            let next_lambdas: Vec<f64> = surviving.into_iter().map(|(_, lambda)| lambda).collect();
            self.lambdas = next_lambdas.into_boxed_slice();
            if let Some(src_mut) = &mut self.source {
                src_mut.knot_points = next_src_knots;
            }
            self.hazard_calibration = None;
            return Ok(());
        }

        let mut next_lambdas = Vec::with_capacity(self.lambdas.len());
        for (&t, &lambda) in self.knots.iter().zip(self.lambdas.iter()) {
            let weight = spec.bump_type.weight(t);
            let shifted = lambda + shift * weight;
            if !shifted.is_finite() || shifted < 0.0 {
                return Err(crate::InputError::UnsupportedBump {
                    reason: "non-finite or negative hazard rate after bump".to_string(),
                }
                .into());
            }
            next_lambdas.push(shifted);
        }
        self.lambdas = next_lambdas.into_boxed_slice();
        self.hazard_calibration = None;
        Ok(())
    }

    /// Create a curve with every model hazard intensity shifted additively in basis points.
    ///
    /// Adds `bump_bp * 1e-4` directly to every intensity parameter; does not
    /// convert par CDS spreads by `1 / (1 - R)` or recalibrate quotes. An
    /// exact `0.0` bump preserves `hazard_calibration` and metadata; any nonzero
    /// shift clears `hazard_calibration`.
    ///
    /// # Arguments
    ///
    /// * `bump_bp` - Additive intensity shift in basis points (`1 bp = 1e-4`
    ///   annual decimal intensity).
    ///
    /// # Errors
    ///
    /// Returns an error if `bump_bp` is non-finite, nonzero `bump_bp` underflows
    /// `bump_bp * 1e-4` to zero, or any resulting intensity becomes negative or
    /// non-finite.
    pub fn with_parallel_hazard_rate_bump_bp(&self, bump_bp: f64) -> crate::Result<Self> {
        self.bumped_with_id(self.id.clone(), &BumpSpec::parallel_bp(bump_bp))
    }

    /// Create a curve with direct additive hazard-rate shocks at exact knot tenors.
    ///
    /// Each entry in `targets_bp` must identify an exact existing positive
    /// ending knot `tᵢ` on this curve without duplicates. All requests and
    /// resulting non-negative intensities are validated atomically before any
    /// curve state is published. Empty or all-zero valid requests preserve
    /// `hazard_calibration`.
    ///
    /// # Arguments
    ///
    /// * `targets_bp` - Unique `(tenor_years, bump_bp)` pairs targeting exact
    ///   existing positive knots of `self`.
    ///
    /// # Errors
    ///
    /// Returns an error if any tenor or bump is non-finite, any tenor is `<= 0`
    /// or fails to match an exact existing knot on `self`, duplicate tenors are
    /// supplied, nonzero `bump_bp` underflows `1e-4` normalization to zero, or
    /// any resulting intensity becomes negative or non-finite.
    pub fn with_tenor_hazard_rate_bumps_bp(
        &self,
        targets_bp: &[(f64, f64)],
    ) -> crate::Result<Self> {
        let mut next_lambdas = self.lambdas.to_vec();
        let mut seen = vec![false; self.knots.len()];
        let mut any_nonzero = false;

        for &(tenor, bump_bp) in targets_bp {
            if !tenor.is_finite() || tenor <= 0.0 || !bump_bp.is_finite() {
                return Err(crate::InputError::UnsupportedBump {
                    reason: "hazard-rate tenor and basis-point bump must be finite with tenor > 0"
                        .to_string(),
                }
                .into());
            }
            let Some(idx) = self.knots.iter().position(|&k| exact_f64_eq(k, tenor)) else {
                return Err(crate::InputError::UnsupportedBump {
                    reason: format!("hazard-rate bump tenor {tenor} did not match any exact knot"),
                }
                .into());
            };
            if seen[idx] {
                return Err(crate::InputError::UnsupportedBump {
                    reason: format!("duplicate hazard-rate bump tenor {tenor}"),
                }
                .into());
            }
            seen[idx] = true;

            let shift = bump_bp * ONE_BASIS_POINT;
            if !shift.is_finite() || (!exact_f64_eq(bump_bp, 0.0) && exact_f64_eq(shift, 0.0)) {
                return Err(crate::InputError::UnsupportedBump {
                    reason: format!(
                        "hazard-rate bump {bump_bp}bp overflowed or underflowed scaling"
                    ),
                }
                .into());
            }
            if !exact_f64_eq(shift, 0.0) {
                any_nonzero = true;
            }

            let shifted = next_lambdas[idx] + shift;
            if !shifted.is_finite() || shifted < 0.0 {
                return Err(crate::InputError::UnsupportedBump {
                    reason: "non-finite or negative hazard rate after bump".to_string(),
                }
                .into());
            }
            next_lambdas[idx] = shifted;
        }

        if !any_nonzero {
            return Ok(self.clone());
        }

        let mut next = self.clone();
        if let Some(src) = &mut next.source {
            let surviving = surviving_source_knots(&src.knot_points, src.offset);
            let matched_live = surviving.len() == next.knots.len()
                && surviving
                    .iter()
                    .zip(next.knots.iter())
                    .all(|(&(t, _), &k)| exact_f64_eq(t, k));
            if !matched_live {
                return Err(crate::Error::Validation(
                    "conditioned hazard curve pillars inconsistent with source".to_string(),
                ));
            }
            for point in &mut src.knot_points {
                if point.0 > src.offset {
                    let rem = point.0 - src.offset;
                    if let Some(live_idx) = next.knots.iter().position(|&k| exact_f64_eq(k, rem)) {
                        point.1 = next_lambdas[live_idx];
                    }
                }
            }
            if src
                .knot_points
                .iter()
                .all(|&(source_t, _)| source_t <= src.offset)
                && let Some(last) = src.knot_points.last_mut()
                && !next_lambdas.is_empty()
            {
                last.1 = next_lambdas[0];
            }
        }
        next.lambdas = next_lambdas.into_boxed_slice();
        next.hazard_calibration = None;
        Ok(next)
    }

    /// Condition the curve to `reference_date >= source_base_date()` on the original timeline.
    ///
    /// Retains the original unconditioned schedule and clock so dated survival
    /// and conditional default queries integrate directly from `reference_date`.
    ///
    /// # Arguments
    ///
    /// * `reference_date` - Target valuation anchor date on or after
    ///   [`source_base_date`](Self::source_base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `reference_date < source_base_date()`, day-count
    /// calculation fails, or building the conditioned curve fails.
    pub fn condition(&self, reference_date: Date) -> crate::Result<Self> {
        let src_base = self.source_base_date();
        if reference_date < src_base {
            return Err(crate::Error::Validation(format!(
                "HazardCurve::condition requires reference_date ({reference_date}) on or after source base date ({src_base})"
            )));
        }
        let offset = signed_curve_year_fraction(
            src_base,
            reference_date,
            self.day_count,
            self.calendar.as_ref(),
        )?;
        if !offset.is_finite() || offset < 0.0 {
            return Err(crate::Error::Validation(format!(
                "HazardCurve::condition produced invalid offset {offset}"
            )));
        }
        if reference_date == self.base {
            return Ok(self.clone());
        }
        let src_knots: Vec<(f64, f64)> = self
            .source
            .as_ref()
            .map(|src| src.knot_points.clone())
            .unwrap_or_else(|| self.knot_points().collect());
        let surviving = surviving_source_knots(&src_knots, offset);

        let mut builder = self
            .metadata_builder(self.id.clone())
            .base_date(reference_date)
            .knots(surviving);
        builder.source = Some(HazardConditionSource {
            base_date: src_base,
            knot_points: src_knots,
            offset,
        });
        builder.build()
    }
}

impl Bumpable for HazardCurve {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        self.bumped_with_id(spec.hazard_shift_id(self.id()), &spec)
    }
}
