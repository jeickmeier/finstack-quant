//! SABR model: Hagan expansions for the lognormal and normal implied volatility.
//!
//! Implements the SABR (Stochastic Alpha Beta Rho) model, the market standard
//! for swaption and cap/floor volatility smile modeling. Uses the Hagan et al.
//! (2002) analytical approximation for implied volatility.
//!
//! # Mathematical Foundation
//!
//! The SABR model describes the joint dynamics of a forward rate and its
//! stochastic volatility:
//!
//! ```text
//! dF = σ * F^β * dW₁
//! dσ = ν * σ * dW₂
//! E[dW₁ * dW₂] = ρ * dt
//!
//! where:
//!   F = forward rate
//!   σ = instantaneous volatility (alpha at t=0)
//!   β = CEV exponent (controls backbone; 0=normal, 1=lognormal)
//!   ν = vol-of-vol (controls smile curvature)
//!   ρ = correlation (controls skew direction)
//! ```
//!
//! # Parameters
//!
//! | Parameter | Symbol | Typical Range | Market Role |
//! |-----------|--------|---------------|-------------|
//! | Alpha (α) | `alpha` | 0.01–0.50 | ATM volatility level |
//! | Beta (β) | `beta` | 0.0–1.0 | Backbone/CEV exponent |
//! | Rho (ρ) | `rho` | (-1, 1) | Skew direction |
//! | Nu (ν) | `nu` | 0.01–1.50 | Smile curvature (vol-of-vol) |
//!
//! # Common Calibration Choices
//!
//! - **β = 0.5** (CMS market convention): Square-root dynamics
//! - **β = 0.0** (Normal SABR): Used for negative rate environments
//! - **β = 1.0** (Lognormal SABR): Traditional lognormal dynamics
//!
//! # Approximation Accuracy
//!
//! The Hagan approximation is accurate for:
//! - Options with expiry T < 10Y (shorter is better)
//! - Strikes not too far from ATM (within 2-3 standard deviations)
//! - Moderate vol-of-vol (ν < 1.5)
//!
//! For very long-dated options or deep OTM strikes, consider exact PDE solutions.
//!
//! # References
//!
//! - Hagan, P. S., Kumar, D., Lesniewski, A. S., & Woodward, D. E. (2002).
//!   "Managing Smile Risk." *Wilmott Magazine*, September 2002, 84-108. `docs/REFERENCES.md#hagan-2002-sabr`
//!
//! - Obloj, J. (2008). "Fine-tune your smile: Correction to Hagan et al."
//!   *Wilmott Magazine*, May 2008.
//! - West, G. (2005). "Calibration of the SABR Model in Illiquid Markets."
//!   *Applied Mathematical Finance*, 12(4), 371-385. `docs/REFERENCES.md#hagan-2002-sabr`
//! - QuantLib SABR implementation: `ql/termstructures/volatility/sabr.cpp` `docs/REFERENCES.md#hagan-2002-sabr`
use super::parameters::SabrParameters;
use crate::volatility::conventions::VolatilityConvention;
use finstack_quant_core::{Error, Result};

/// Snap β to the exact endpoints 0 or 1 when it is within this tolerance.
///
/// Used by every code path that branches on "β = 0" (normal-SABR) or "β = 1"
/// (lognormal-SABR) so that a raw parameter like `0.00005` lands on the same
/// branch for smile pricing, the ATM limit, and the quoting convention.
pub(crate) const BETA_SNAP_TOL: f64 = 1e-4;

impl SabrParameters {
    /// Quoting convention of the vols [`Self::implied_volatility`] produces.
    ///
    /// The β≈0 (normal-SABR) branch outputs a **normal (Bachelier)** vol in
    /// absolute rate units, displaced or not; every other β outputs a
    /// **lognormal (Black)** vol, displaced by `shift` when one is set.
    /// Storing a Bachelier vol in a Black surface (or vice versa) is a silent
    /// unit error, so callers must branch on this tag. Determined by the same
    /// β-snap tolerance the pricing branches use.
    #[must_use]
    pub fn quote_convention(&self) -> VolatilityConvention {
        if self.beta < BETA_SNAP_TOL {
            VolatilityConvention::Normal
        } else if let Some(shift) = self.shift {
            VolatilityConvention::ShiftedLognormal { shift }
        } else {
            VolatilityConvention::Lognormal
        }
    }

    /// Hagan et al. (2002) implied-vol expansion with Obloj (2008) z/χ
    /// correction and optional shifted SABR for negative rates.
    ///
    /// Selects the branch from β: a normal (Bachelier) vol for β≈0 and a
    /// lognormal (Black) vol otherwise, as reported by
    /// [`Self::quote_convention`]. [`Self::implied_vol_lognormal`] and
    /// [`Self::implied_vol_normal`] are the separate fixed-convention
    /// expansions used by SABR cubes.
    ///
    /// # Arguments
    ///
    /// * `forward` - Forward price or rate used by the volatility or pricing model
    /// * `strike` - Option strike in the surface's quote units (absolute or relative)
    /// * `time_to_expiry` - Time to expiry in years; must be strictly positive.
    #[must_use = "computed volatility should be used"]
    #[inline]
    pub fn implied_volatility(
        &self,
        forward: f64,
        strike: f64,
        time_to_expiry: f64,
    ) -> Result<f64> {
        self.validate_inputs(forward, strike, time_to_expiry)?;

        if self.beta < BETA_SNAP_TOL {
            let vol = normal_beta_zero(
                self.alpha,
                self.nu,
                self.rho,
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

        self.lognormal_expansion(forward, strike, time_to_expiry)
    }

    /// Hagan et al. (2002) lognormal (Black-76) expansion with Obloj (2008) z/χ
    /// correction and optional displacement shift for any `β ∈ [0, 1]`.
    #[inline]
    pub(super) fn lognormal_expansion(
        &self,
        forward: f64,
        strike: f64,
        time_to_expiry: f64,
    ) -> Result<f64> {
        let (effective_forward, effective_strike) = match self.shift {
            Some(shift) => (forward + shift, strike + shift),
            None => (forward, strike),
        };

        if !effective_forward.is_finite()
            || !effective_strike.is_finite()
            || !time_to_expiry.is_finite()
            || effective_forward <= 0.0
            || effective_strike <= 0.0
            || time_to_expiry <= 0.0
        {
            return Err(Error::Validation(format!(
                "SABR lognormal expansion requires positive effective forward/strike and expiry; \
                 got forward={forward:.6}, strike={strike:.6}, T={time_to_expiry:.4}"
            )));
        }

        let alpha = self.alpha;
        let nu = self.nu;
        let rho = self.rho;

        // Snap β to 0 or 1 with the same `BETA_SNAP_TOL` used by
        // `implied_volatility` / `quote_convention`, so smile and ATM paths take
        // the same branch for a given raw β.
        let raw_beta = self.beta;
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
        let f_mid_beta = f_mid.powf(1.0 - beta);

        // ATM detection uses relative moneyness only. The previous
        // `z.abs() < 1e-8` clause was small both for genuine ATM (F≈K) AND
        // whenever ν→0, so it short-circuited every strike to the flat ATM vol
        // in the pure-CEV limit. The ν→0 limit is handled by the general
        // formula (z/χ(z)→1).
        let relative_diff = (effective_forward - effective_strike).abs()
            / effective_forward.abs().max(effective_strike.abs());
        if relative_diff < 1e-8 {
            return self.atm_lognormal(effective_forward, time_to_expiry, beta);
        }

        let log_moneyness = (effective_forward / effective_strike).ln();

        // Obloj (2008) correction: use geometric-mean-based z for the z/χ(z)
        // ratio: z = (ν/α) * f_mid^{1-β} * ln(F/K).
        let z_corrected = if beta_is_one {
            (nu / alpha) * log_moneyness
        } else {
            (nu / alpha) * f_mid_beta * log_moneyness
        };

        let x = self.calculate_chi_robust(z_corrected)?;

        let factor1 = if f_mid_beta.abs() < 1e-14 {
            alpha
        } else {
            let correction_term = 1.0
                + (1.0 - beta).powi(2) / 24.0 * log_moneyness.powi(2)
                + (1.0 - beta).powi(4) / 1920.0 * log_moneyness.powi(4);
            alpha / (f_mid_beta * correction_term)
        };

        let factor2 = self.z_over_chi(z_corrected, x)?;

        let time_correction = (1.0 - beta).powi(2) / 24.0 * alpha.powi(2)
            / f_mid.powf(2.0 * (1.0 - beta))
            + 0.25 * rho * beta * nu * alpha / f_mid_beta
            + (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2);

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

    /// Lognormal ATM implied volatility (Hagan et al. (2002) eq. 2.18).
    #[inline]
    fn atm_lognormal(&self, forward: f64, time_to_expiry: f64, beta: f64) -> Result<f64> {
        let alpha = self.alpha;
        let nu = self.nu;
        let rho = self.rho;

        if alpha.abs() < 1e-14 {
            return Ok(0.0);
        }

        let f_beta = if forward.abs() < 1e-14 {
            1e-14_f64.powf(1.0 - beta)
        } else {
            forward.powf(1.0 - beta)
        };

        let alpha_term = (1.0 - beta).powi(2) / 24.0 * alpha.powi(2) / (f_beta * f_beta);
        let rho_term = 0.25 * rho * beta * nu * alpha / f_beta;
        let nu_term = (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2);

        let time_correction = alpha_term + rho_term + nu_term;

        let vol = alpha / f_beta * (1.0 + time_to_expiry * time_correction);

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
        chi(z, self.rho)
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
            let rho = self.rho;
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
                chi, z, self.rho
            )));
        }

        Ok(z / chi)
    }

    /// Whether normal beta=0 dynamics or a displacement permit negative rates.
    pub fn supports_negative_rates(&self) -> bool {
        self.beta < BETA_SNAP_TOL || self.shift.is_some()
    }

    /// Forward and strike after applying the optional displacement shift.
    ///
    /// # Arguments
    ///
    /// * `forward` - Unshifted forward rate or price.
    /// * `strike` - Unshifted strike in the same units as `forward`.
    pub fn effective_rates(&self, forward: f64, strike: f64) -> (f64, f64) {
        if let Some(shift) = self.shift {
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
        if !forward.is_finite() || !strike.is_finite() {
            return Err(Error::Validation(format!(
                "SABR forward and strike must be finite, got: forward={forward}, strike={strike}"
            )));
        }
        if !time_to_expiry.is_finite() || time_to_expiry <= 0.0 {
            return Err(Error::Validation(format!(
                "SABR time_to_expiry must be positive, got: {:.6}",
                time_to_expiry
            )));
        }

        if self.shift.is_none() {
            // Normal SABR (β≈0) permits non-positive forwards and strikes.
            let beta_is_zero = self.beta < BETA_SNAP_TOL;
            if !beta_is_zero && (forward <= 0.0 || strike <= 0.0) {
                return Err(Error::Validation(format!(
                    "Standard SABR with beta={:.4} requires positive rates. \
                     Got forward={:.6}, strike={:.6}. \
                     Use shifted SABR (or beta=0 normal SABR) for negative rates.",
                    self.beta, forward, strike
                )));
            }
        } else if let Some(shift) = self.shift {
            if self.beta >= BETA_SNAP_TOL && (forward + shift <= 0.0 || strike + shift <= 0.0) {
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
    if !z.is_finite() || !rho.is_finite() {
        return Err(Error::Validation(format!(
            "SABR chi function requires finite z and rho, got z={z}, rho={rho}"
        )));
    }
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

impl SabrParameters {
    /// Normal (Bachelier) implied volatility using Hagan's approximation.
    ///
    /// Returns the normal/Bachelier implied volatility. This is useful for
    /// negative rate environments (EUR, CHF, JPY post-2014).
    ///
    /// # Arguments
    ///
    /// * `f` - Forward rate (may be negative)
    /// * `k` - Strike rate (may be negative)
    /// * `t` - Time to expiry in years
    ///
    /// # Returns
    ///
    /// Normal/Bachelier implied volatility, or `f64::NAN` for non-positive
    /// expiry, a χ(z) breakdown, or cross-zero inputs (`f·k ≤ 0` after any
    /// configured shift) with `beta > 0` — the CEV backbone is not
    /// shift-invariant, so such quotes require an explicit
    /// [`with_shift`](Self::with_shift). β = 0 (normal SABR) is shift-invariant
    /// and prices cross-zero quotes directly. Guard with `is_finite()` or use
    /// the checked [`implied_vol_normal`](Self::implied_vol_normal)
    /// on pricing paths.
    fn implied_vol_normal_unchecked(&self, f: f64, k: f64, t: f64) -> f64 {
        let alpha = self.alpha;
        let beta = if self.beta < BETA_SNAP_TOL {
            0.0
        } else {
            self.beta
        };
        let rho = self.rho;
        let nu = self.nu;

        let (f, k) = if let Some(s) = self.shift {
            (f + s, k + s)
        } else {
            (f, k)
        };

        if t <= 0.0 {
            return f64::NAN;
        }
        if beta > 0.0 && (f <= 0.0 || k <= 0.0) {
            return f64::NAN;
        }

        if beta == 0.0 {
            return normal_beta_zero(alpha, nu, rho, f, k, t);
        }

        // No special-case for ν → 0: as in the lognormal expansion, the general
        // formula is continuous in ν (z → 0, z/χ(z) → 1) and retains the full
        // (1 + [...]T) correction in the CEV limit.

        // ATM case
        if (f - k).abs() < 1e-12 * f.abs().max(1e-10) {
            return self.atm_vol_normal(f, t);
        }

        let fk = f * k;
        let one_minus_beta = 1.0 - beta;

        let fk_mid = fk.powf(one_minus_beta / 2.0);
        let log_fk = (f / k).ln();

        // z = (ν/α) * (FK)^((1-β)/2) * ln(F/K)
        let z = (nu / alpha) * fk_mid * log_fk;
        let chi_z = chi(z, rho).unwrap_or(f64::NAN);

        let z_over_chi = self.z_over_chi(z, chi_z).unwrap_or(f64::NAN);

        // Normal vol (Hagan 2002, eq. 2.17b):
        //   σ_N = α·(FK)^(β/2) · [numerator series] / [denominator series]
        //         · (z/χ(z)) · [1 + correction·T]
        let fk_beta_half = fk.powf(beta / 2.0);

        let omb2 = one_minus_beta * one_minus_beta;
        let log_fk_sq = log_fk * log_fk;

        // Numerator series (β-independent):
        //   1 + (1/24)ln²(F/K) + (1/1920)ln⁴(F/K)
        let numer_series = 1.0 + log_fk_sq / 24.0 + log_fk_sq * log_fk_sq / 1920.0;
        // Denominator series:
        //   1 + ((1-β)²/24)ln²(F/K) + ((1-β)⁴/1920)ln⁴(F/K)
        let denom_series =
            1.0 + omb2 / 24.0 * log_fk_sq + omb2 * omb2 / 1920.0 * log_fk_sq * log_fk_sq;

        // First-order time correction. The leading term uses the normal-SABR
        // coefficient −β(2−β)/24 (eq. 2.17b), NOT the lognormal (1−β)²/24.
        let fk_omb = fk.powf(one_minus_beta);
        let correction = 1.0
            + (-beta * (2.0 - beta) / 24.0 * alpha * alpha / fk_omb
                + 0.25 * rho * beta * nu * alpha / fk_mid
                + (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu)
                * t;

        alpha * fk_beta_half * numer_series / denom_series * z_over_chi * correction
    }

    /// Lognormal SABR implied volatility with checked error semantics.
    ///
    /// # Errors
    ///
    /// Returns a [`Validation`](finstack_quant_core::Error::Validation) error naming the
    /// offending input when the shifted forward or strike is non-positive, the
    /// expiry is not strictly positive, or the expansion yields a non-finite or
    /// non-positive volatility.
    ///
    /// # Arguments
    ///
    /// * `f` - Unshifted forward in decimal rate or price units, strictly positive after any configured shift.
    /// * `k` - Unshifted strike in the same units as `f`, strictly positive after the shift.
    /// * `t` - Finite, strictly positive option expiry in years.
    pub fn implied_vol_lognormal(
        &self,
        f: f64,
        k: f64,
        t: f64,
    ) -> finstack_quant_core::Result<f64> {
        self.lognormal_expansion(f, k, t)
    }

    /// Normal SABR implied volatility with checked error semantics.
    ///
    /// # Errors
    ///
    /// - Returns a [`Validation`](finstack_quant_core::Error::Validation) error when
    ///   `f·k ≤ 0` (after any configured shift) with `beta > 0`: the CEV
    ///   backbone is not shift-invariant, so cross-zero quotes require an
    ///   explicit shift via [`new_with_shift`](Self::new_with_shift).
    /// - Returns [`Error::Validation`](finstack_quant_core::Error::Validation)
    ///   when the underlying expansion yields a non-finite volatility.
    ///
    /// # Arguments
    ///
    /// * `f` - Unshifted forward in decimal rate or price units; negative values require beta zero or a sufficient configured shift.
    /// * `k` - Unshifted strike in the same units as `f`, with the same positivity condition.
    /// * `t` - Finite, strictly positive option expiry in years.
    pub fn implied_vol_normal(&self, f: f64, k: f64, t: f64) -> finstack_quant_core::Result<f64> {
        let shift = self.shift.unwrap_or(0.0);
        let (sf, sk) = (f + shift, k + shift);
        if (sf <= 0.0 || sk <= 0.0)
            && self.beta >= BETA_SNAP_TOL
            && (sf - sk).abs() > 1e-12 * sf.abs().max(1e-10)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "SABR implied_vol_normal: forward or strike <= 0 (F={sf}, K={sk} after shift) with \
                 beta = {} > 0. The CEV backbone is not shift-invariant; configure an explicit \
                 shift via SabrParameters::with_shift for negative/cross-zero rates.",
                self.beta
            )));
        }
        let v = self.implied_vol_normal_unchecked(f, k, t);
        if v.is_finite() && v >= 0.0 {
            Ok(v)
        } else {
            Err(finstack_quant_core::Error::Validation(
                "normal SABR expansion produced a non-finite or negative volatility".to_string(),
            ))
        }
    }

    /// ATM normal volatility (simplified formula when F ≈ K).
    fn atm_vol_normal(&self, f: f64, t: f64) -> f64 {
        let alpha = self.alpha;
        let beta = self.beta;
        let rho = self.rho;
        let nu = self.nu;

        let f_abs = f.abs().max(1e-10);
        let omb = 1.0 - beta;
        let f_beta = f_abs.powf(beta);
        let f_omb = f_abs.powf(omb);

        let base = alpha * f_beta;

        // Normal-SABR leading coefficient is −β(2−β)/24 (Hagan 2.17b at F=K),
        // NOT the lognormal (1−β)²/24. The two series cancel at ATM.
        let correction = 1.0
            + (-beta * (2.0 - beta) / 24.0 * alpha * alpha / (f_omb * f_omb)
                + 0.25 * rho * beta * nu * alpha / f_omb
                + (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu)
                * t;

        base * correction
    }
}

/// Hagan's beta-zero normal limit (B.70a), independent of the rate origin.
pub(super) fn normal_beta_zero(alpha: f64, nu: f64, rho: f64, f: f64, k: f64, t: f64) -> f64 {
    let z = nu / alpha * (f - k);
    let ratio = if z.abs() < 1e-6 {
        1.0 - 0.5 * rho * z + (2.0 - 3.0 * rho * rho) * z * z / 12.0
    } else {
        z / chi(z, rho).unwrap_or(f64::NAN)
    };
    alpha * ratio * (1.0 + (2.0 - 3.0 * rho * rho) * nu * nu * t / 24.0)
}
