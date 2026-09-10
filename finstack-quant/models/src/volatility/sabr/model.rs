//! SABR model, smile, parameter, and calibration support.
//!
use super::parameters::SabrParameters;
use finstack_quant_core::{Error, Result};

/// Snap β to the exact endpoints 0 or 1 when it is within this tolerance.
///
/// Used by every code path that branches on "β = 0" (normal-SABR) or "β = 1"
/// (lognormal-SABR) so that a raw parameter like `0.00005` lands on the same
/// branch in both `implied_volatility` (smile pricing) and `atm_volatility`
/// (ATM-only). Previously these two paths used 1e-4 vs 1e-12 respectively,
/// which let raw β=5e-5 take the normal branch when smiling and the general
/// branch when ATM-pricing the same parameter set.
pub(crate) const BETA_SNAP_TOL: f64 = 1e-4;

/// Quoting convention of the implied volatility produced by Hagan's SABR
/// expansion.
///
/// The β≈0 (normal-SABR) branch of `implied_volatility` outputs a **normal
/// (Bachelier)** vol in absolute rate units; every other β outputs a
/// **lognormal (Black)** vol. Storing a Bachelier vol in a Black surface (or
/// vice versa) is a silent unit error, so callers must branch on this tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SabrVolType {
    /// Normal (Bachelier) volatility in absolute rate units (β≈0 branch).
    Normal,
    /// Lognormal (Black) volatility (β > 0 branches, including β=1).
    Black,
}

/// SABR model wrapping validated [`SabrParameters`].
pub struct SabrModel {
    params: SabrParameters,
}

impl SabrModel {
    /// Wrap validated SABR parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - Already-validated SABR parameters (α, β, ρ, ν, optional
    ///   shift) used by every subsequent expansion call.
    pub fn new(params: SabrParameters) -> Self {
        Self { params }
    }

    /// Quoting convention of the vols this model's Hagan expansion produces.
    ///
    /// Determined by the same β-snap tolerance the pricing branches use:
    /// β within `BETA_SNAP_TOL` of 0 routes to the normal-SABR branch and
    /// yields [`SabrVolType::Normal`]; everything else yields
    /// [`SabrVolType::Black`].
    pub fn vol_type(&self) -> SabrVolType {
        if self.params.beta < BETA_SNAP_TOL {
            SabrVolType::Normal
        } else {
            SabrVolType::Black
        }
    }

    /// Implied volatility together with its quoting convention.
    ///
    /// Identical to [`Self::implied_volatility`] but returns the
    /// [`SabrVolType`] tag alongside the number so callers cannot silently
    /// store a Bachelier vol (β≈0 branch) in a Black-vol surface.
    #[must_use = "computed volatility should be used"]
    pub fn implied_volatility_with_type(
        &self,
        forward: f64,
        strike: f64,
        time_to_expiry: f64,
    ) -> Result<(f64, SabrVolType)> {
        Ok((
            self.implied_volatility(forward, strike, time_to_expiry)?,
            self.vol_type(),
        ))
    }

    /// Hagan et al. (2002) implied-vol expansion with Obloj (2008) z/χ
    /// correction and optional shifted SABR for negative rates.
    ///
    /// # Arguments
    ///
    /// * `forward` - Forward price or rate used by the volatility or pricing model
    /// * `strike` - Option strike in the surface's quote units (absolute or relative)
    /// * `time_to_expiry` - Time to expiry used by the algorithm, subject to the enclosing type invariants and documented units.
    #[must_use = "computed volatility should be used"]
    #[inline]
    pub fn implied_volatility(
        &self,
        forward: f64,
        strike: f64,
        time_to_expiry: f64,
    ) -> Result<f64> {
        self.validate_inputs(forward, strike, time_to_expiry)?;

        if self.params.beta < BETA_SNAP_TOL {
            let vol = super::expansion::normal_beta_zero(
                self.params.alpha,
                self.params.nu,
                self.params.rho,
                forward,
                strike,
                time_to_expiry,
            );
            return if vol.is_finite() && vol > 0.0 {
                Ok(vol)
            } else {
                Err(Error::Validation(
                    "normal SABR produced invalid volatility".to_owned(),
                ))
            };
        }

        let (effective_forward, effective_strike) = match self.params.shift {
            Some(shift) => (forward + shift, strike + shift),
            None => (forward, strike),
        };

        let alpha = self.params.alpha;
        let nu = self.params.nu;
        let rho = self.params.rho;

        // Snap β to 0 or 1 with the same `BETA_SNAP_TOL` used by
        // `atm_volatility` / `vol_type`, so smile and ATM paths take the same
        // branch for a given raw β.
        let raw_beta = self.params.beta;
        let beta_is_zero = raw_beta < BETA_SNAP_TOL;
        let beta_is_one = !beta_is_zero && (1.0 - raw_beta).abs() < BETA_SNAP_TOL;
        let beta = if beta_is_zero {
            0.0
        } else if beta_is_one {
            1.0
        } else {
            raw_beta
        };

        let f_mid = (effective_forward * effective_strike).sqrt();
        let f_mid_beta = if beta_is_zero {
            1.0
        } else {
            f_mid.powf(1.0 - beta)
        };

        // There is intentionally NO `nu ≈ 0` short-circuit. As ν→0 the general
        // Hagan formula degenerates correctly on its own — z→0, and z/χ(z) → 1
        // (see `calculate_chi_robust` and the z-guard at the ATM check below).
        // Short-circuiting to `atm_volatility` for every strike would fabricate
        // a perfectly FLAT smile, which is wrong for β≠1: the SABR/CEV smile
        // is non-flat because `factor1` depends on the strike through `f_mid`.
        // Reference: Hagan et al. (2002) eq. 2.17a.
        let z = if beta_is_one {
            (nu / alpha) * (effective_forward / effective_strike).ln()
        } else if beta_is_zero {
            (nu / alpha) * (effective_forward - effective_strike)
        } else {
            (nu / alpha) * (effective_forward.powf(1.0 - beta) - effective_strike.powf(1.0 - beta))
                / (1.0 - beta)
        };

        // ATM detection uses relative moneyness only. The previous
        // `z.abs() < 1e-8` clause was small both for genuine ATM (F≈K) AND
        // whenever ν→0, so it short-circuited every strike to the flat ATM vol
        // in the pure-CEV limit. The ν→0 limit is handled by the general
        // formula (z/χ(z)→1).
        let relative_diff = (effective_forward - effective_strike).abs()
            / effective_forward.abs().max(effective_strike.abs());
        if relative_diff < 1e-8 {
            return self.atm_volatility(effective_forward, time_to_expiry);
        }

        let log_moneyness = (effective_forward / effective_strike).ln();

        // Obloj (2008) correction: use geometric-mean-based z for the z/χ(z)
        // ratio. Hagan et al. (2002) uses
        // z = (ν/α)(F^{1-β} - K^{1-β})/(1-β), which introduces O(ε²) errors
        // for long maturities and high vol-of-vol. The corrected formula uses
        // z = (ν/α) * f_mid^{1-β} * ln(F/K) for 0 < β < 1. For β=0 (normal)
        // and β=1 (lognormal), the original formula is already exact.
        // arXiv:0708.0998v2
        let z_corrected = if beta_is_one || beta_is_zero {
            z
        } else {
            (nu / alpha) * f_mid.powf(1.0 - beta) * log_moneyness
        };

        let x = self.calculate_chi_robust(z_corrected)?;

        let factor1 = if f_mid_beta.abs() < 1e-14 {
            alpha
        } else {
            let correction_term = if beta_is_zero {
                1.0
            } else {
                1.0 + (1.0 - beta).powi(2) / 24.0 * log_moneyness.powi(2)
                    + (1.0 - beta).powi(4) / 1920.0 * log_moneyness.powi(4)
            };
            alpha / (f_mid_beta * correction_term)
        };

        // z/χ(z) → 1 holds only as z → 0, not for an arbitrary tiny χ.
        // Fabricating `1.0` whenever χ underflowed silently produced a wrong
        // factor for genuinely-pathological χ. Small |z| uses the Taylor
        // ratio; otherwise divide exactly; if χ underflowed while z is not
        // small, error out rather than guess.
        let factor2 = self.z_over_chi(z_corrected, x)?;

        // β=0 outputs *normal* (Bachelier) implied vol — `factor1 = α`
        // (because `f_mid_beta = 1`), not the Black-vol prefactor `α/F^(1-β)`.
        // The β=0 normal-SABR time correction is just `(2-3ρ²)/24·ν²`
        // (Hagan/Obloj normal-vol expansion); it has NO `α²/(24·f_mid²)`
        // leverage term. That term belongs to the lognormal/Black σ_B
        // expansion (Hagan eq. 2.17a) — applying it here would break the exact
        // `β=0, ν=0 ⇒ σ_N = α` Bachelier identity.
        let time_correction = if beta_is_zero {
            (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2)
        } else {
            (1.0 - beta).powi(2) / 24.0 * alpha.powi(2) / f_mid.powf(2.0 * (1.0 - beta))
                + 0.25 * rho * beta * nu * alpha / f_mid_beta
                + (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2)
        };

        let factor3 = 1.0 + time_to_expiry * time_correction;

        let volatility = factor1 * factor2 * factor3;

        if volatility <= 0.0 || !volatility.is_finite() {
            return Err(Error::Validation(format!(
                "SABR produced invalid volatility={:.6} for forward={:.6}, strike={:.6}, T={:.4}. \
                 Check parameter values.",
                volatility, forward, strike, time_to_expiry
            )));
        }

        Ok(volatility)
    }

    /// ATM implied volatility (Hagan et al. (2002) eq. 2.18).
    #[inline]
    pub(crate) fn atm_volatility(&self, forward: f64, time_to_expiry: f64) -> Result<f64> {
        let alpha = self.params.alpha;
        let nu = self.params.nu;
        let rho = self.params.rho;
        // Same β snap as `implied_volatility`. Snapping only β≈0 (and not
        // β≈1) would price exact-ATM with prefactor α/F^(1−β) while
        // neighbouring strikes use snapped β=1 and α — an ATM smile
        // discontinuity of ~F^(−(1−β)).
        let raw_beta = self.params.beta;
        let beta_is_zero = raw_beta < BETA_SNAP_TOL;
        let beta_is_one = !beta_is_zero && (1.0 - raw_beta).abs() < BETA_SNAP_TOL;
        let beta = if beta_is_zero {
            0.0
        } else if beta_is_one {
            1.0
        } else {
            raw_beta
        };

        if alpha.abs() < 1e-14 {
            return Ok(0.0);
        }

        // Hagan et al. (2002) eq. 2.18:
        //   σ_ATM = α / F^(1-β) · [1 + ((1-β)²α²/24/F^(2(1-β)) + ρβνα/4/F^(1-β) + (2-3ρ²)ν²/24) · T]
        let vol = if beta_is_zero {
            // Normal SABR (β=0): output is *normal* (Bachelier) vol with
            // prefactor α (not α/F). The β=0 time correction is
            // `(2-3ρ²)ν²/24` only — the α²/(24·F²) leverage term belongs to
            // the lognormal/Black σ_B expansion and would break
            // `β=0, ν=0 ⇒ σ_N = α`.
            alpha * (1.0 + time_to_expiry * (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2))
        } else {
            // General β ∈ (0,1], including β=1.
            // At β=1: F^(1-β)=1, (1-β)²=0, so alpha_term→0 and rho_term→ρνα/4.
            // At β=0.5: (1-β)²/24·α²/F^(2(1-β)) = α²/(96·F). A previous
            // beta_is_half shortcut used α²/(24·F) — a 4× error.
            let f_beta = if forward.abs() < 1e-14 {
                1e-14_f64.powf(1.0 - beta)
            } else {
                forward.powf(1.0 - beta)
            };

            let alpha_term =
                (1.0 - beta).powi(2) / 24.0 * alpha.powi(2) / forward.powf(2.0 * (1.0 - beta));
            let rho_term = 0.25 * rho * beta * nu * alpha / f_beta;
            let nu_term = (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2);

            let time_correction = alpha_term + rho_term + nu_term;

            alpha / f_beta * (1.0 + time_to_expiry * time_correction)
        };

        if vol <= 0.0 || !vol.is_finite() {
            return Err(Error::Validation(format!(
                "SABR ATM volatility calculation produced invalid result={:.6} for forward={:.6}, T={:.4}",
                vol, forward, time_to_expiry
            )));
        }

        Ok(vol)
    }

    /// χ(z) for the SABR formula, blending a fourth-order series near z = 0
    /// with the exact logarithm so Greeks stay continuous near ATM.
    ///
    /// - Series: χ(z) ≈ z + (ρ/2)z² + ((3ρ² - 1)/6)z³ + (ρ(5ρ² - 3)/8)z⁴ + O(z⁵)
    /// - Smoothstep blend on `|z| ∈ [1e-5, 1e-3]`
    /// - Closed-form limits at ρ → ±1
    #[inline]
    pub(crate) fn calculate_chi_robust(&self, z: f64) -> Result<f64> {
        chi(z, self.params.rho)
    }

    /// `z / χ(z)` correction using a Taylor ratio for small `|z|`.
    ///
    /// `chi` must already be `χ(z)` from [`Self::calculate_chi_robust`].
    ///
    /// - For `|z| < 1e-5` (the series region of `calculate_chi_robust`):
    ///   `z/χ(z) = 1 − (ρ/2)z + ((2−3ρ²)/12)z² + (ρ(5−6ρ²)/24)z³ + O(z⁴)`,
    ///   which avoids 0/0 cancellation.
    /// - Otherwise exact division `z/χ(z)`.
    /// - If `|χ| < 1e-14` while `|z|` is not small, return a validation error
    ///   rather than fabricating `1.0`.
    #[inline]
    pub(crate) fn z_over_chi(&self, z: f64, chi: f64) -> Result<f64> {
        const Z_SERIES: f64 = 1e-5;

        if z.abs() < Z_SERIES {
            let rho = self.params.rho;
            let z2 = z * z;
            let z3 = z2 * z;
            // z/χ(z) Taylor coefficients from χ(z) = z + (ρ/2)z²
            // + ((3ρ²−1)/6)z³ + (ρ(5ρ²−3)/8)z⁴ via 1/(1+u) inversion.
            return Ok(1.0 - 0.5 * rho * z
                + (2.0 - 3.0 * rho * rho) / 12.0 * z2
                + rho * (5.0 - 6.0 * rho * rho) / 24.0 * z3);
        }

        if chi.abs() < 1e-14 {
            return Err(Error::Validation(format!(
                "SABR z/χ(z) correction is undefined: χ(z)={:.3e} underflowed while \
                 z={:.6e} is not small. Check parameter values (ρ={:.6}).",
                chi, z, self.params.rho
            )));
        }

        Ok(z / chi)
    }

    /// Validated SABR parameters used by this model.
    pub fn parameters(&self) -> &SabrParameters {
        &self.params
    }

    /// Replace the model's SABR parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - Replacement validated SABR parameters.
    pub fn set_parameters(&mut self, params: SabrParameters) {
        self.params = params;
    }

    /// Whether normal beta=0 dynamics or a displacement permit negative rates.
    pub fn supports_negative_rates(&self) -> bool {
        self.params.beta < BETA_SNAP_TOL || self.params.shift.is_some()
    }

    /// Forward and strike after applying the optional displacement shift.
    ///
    /// # Arguments
    ///
    /// * `forward` - Unshifted forward rate or price.
    /// * `strike` - Unshifted strike in the same units as `forward`.
    pub fn effective_rates(&self, forward: f64, strike: f64) -> (f64, f64) {
        if let Some(shift) = self.params.shift {
            (forward + shift, strike + shift)
        } else {
            (forward, strike)
        }
    }

    /// Reject non-positive expiry and rates that fall outside the model's
    /// (possibly shifted) domain.
    ///
    /// # Arguments
    ///
    /// * `forward` - Unshifted forward rate or price to be expanded.
    /// * `strike` - Unshifted strike in the same units as `forward`.
    /// * `time_to_expiry` - Expiry in years; must be strictly positive.
    pub fn validate_inputs(&self, forward: f64, strike: f64, time_to_expiry: f64) -> Result<()> {
        if !forward.is_finite()
            || !strike.is_finite()
            || !time_to_expiry.is_finite()
            || time_to_expiry <= 0.0
        {
            return Err(Error::Validation(format!(
                "SABR time_to_expiry must be positive, got: {:.6}",
                time_to_expiry
            )));
        }

        if self.params.shift.is_none() {
            // Normal SABR (β≈0) permits non-positive forwards and strikes.
            let beta_is_zero = self.params.beta < BETA_SNAP_TOL;
            if !beta_is_zero && (forward <= 0.0 || strike <= 0.0) {
                return Err(Error::Validation(format!(
                    "Standard SABR with beta={:.4} requires positive rates. \
                     Got forward={:.6}, strike={:.6}. \
                     Use shifted SABR (or beta=0 normal SABR) for negative rates.",
                    self.params.beta, forward, strike
                )));
            }
        } else if let Some(shift) = self.params.shift {
            if self.params.beta >= BETA_SNAP_TOL
                && (forward + shift <= 0.0 || strike + shift <= 0.0)
            {
                return Err(Error::Validation(format!(
                    "Shifted SABR: effective rates must be positive. \
                     Got forward+shift={:.6}, strike+shift={:.6} (shift={:.6})",
                    forward + shift,
                    strike + shift,
                    shift
                )));
            }
        }

        Ok(())
    }
}

/// Stable Hagan chi function shared by normal and lognormal expansions.
pub(super) fn chi(z: f64, rho: f64) -> Result<f64> {
    // Fourth-order Taylor series around z = 0:
    // χ(z) = ln((√(1 - 2ρz + z²) + z - ρ)/(1 - ρ))
    //
    // With g(z) = √(1 - 2ρz + z²) = 1 - ρz + (1-ρ²)z²/2 + ρ(1-ρ²)z³/2
    //             + (1-ρ²)(5ρ²-1)z⁴/8 + O(z⁵),
    // (g + z - ρ)/(1 - ρ) = 1 + z + (1+ρ)z²/2 + ρ(1+ρ)z³/2 + (1+ρ)(5ρ²-1)z⁴/8,
    // and ln(1 + w) = w - w²/2 + w³/3 - w⁴/4 gives:
    // χ(z) = z + (ρ/2)z² + ((3ρ² - 1)/6)z³ + (ρ(5ρ² - 3)/8)z⁴ + O(z⁵)
    // (ρ=0 check: χ(z) = asinh(z) = z - z³/6 + O(z⁵))
    let series_chi = |z_val: f64| -> f64 {
        let z2 = z_val * z_val;
        let z3 = z2 * z_val;
        let z4 = z2 * z2;
        let c2 = rho / 2.0;
        let c3 = (3.0 * rho * rho - 1.0) / 6.0;
        let c4 = rho * (5.0 * rho * rho - 3.0) / 8.0;
        z_val + c2 * z2 + c3 * z3 + c4 * z4
    };

    let exact_chi = |z_val: f64| -> Result<f64> {
        let discriminant = 1.0 - 2.0 * rho * z_val + z_val * z_val;

        if discriminant < 0.0 {
            return Err(Error::Validation(format!(
                "SABR chi function: negative discriminant {} for z={:.6}, rho={:.6}",
                discriminant, z_val, rho
            )));
        }

        let sqrt_disc = discriminant.sqrt();

        if (1.0 - rho).abs() < 1e-10 {
            // ρ → 1: discriminant is (1−z)², so for z<1 the limit is
            // χ(z) = −ln(1−z). The formula diverges at z ≥ 1 (SABR density
            // degenerates).
            if z_val >= 1.0 {
                return Err(Error::Validation(format!(
                    "SABR chi function: rho≈1 with z={z_val:.6} ≥ 1 — \
                         Hagan expansion is undefined in this limit"
                )));
            }
            return Ok(-(1.0 - z_val).ln());
        }
        if (1.0 + rho).abs() < 1e-10 {
            // ρ → −1: discriminant is (1+z)², so for z > −1 the limit is
            // χ(z) = ln(1+z). At z ≤ −1 the log argument is non-positive
            // and the Hagan expansion is undefined.
            if z_val <= -1.0 {
                return Err(Error::Validation(format!(
                    "SABR chi function: rho≈-1 with z={z_val:.6} ≤ -1 — \
                         Hagan expansion is undefined in this limit"
                )));
            }
            return Ok((sqrt_disc + z_val + 1.0).ln() - (2.0_f64).ln());
        }

        let numerator = sqrt_disc + z_val - rho;
        let denominator = 1.0 - rho;

        if numerator <= 0.0 {
            return Err(Error::Validation(format!(
                "SABR chi function: non-positive log argument {} for z={:.6}, rho={:.6}",
                numerator, z_val, rho
            )));
        }

        Ok((numerator / denominator).ln())
    };

    let abs_z = z.abs();
    let z_low = 1e-5;
    let z_high = 1e-3;

    if abs_z < z_low {
        Ok(series_chi(z))
    } else if abs_z > z_high {
        exact_chi(z)
    } else {
        let t = (abs_z - z_low) / (z_high - z_low);
        let blend = t * t * (3.0 - 2.0 * t); // Hermite smoothstep
        let series_val = series_chi(z);
        let exact_val = exact_chi(z)?;
        Ok((1.0 - blend) * series_val + blend * exact_val)
    }
}
