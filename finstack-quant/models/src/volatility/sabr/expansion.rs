//! SABR stochastic volatility model.
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

use super::{model::chi, SabrParameters};

impl SabrParameters {
    const LOGNORMAL_ATM_LOG_MONEYNESS_THRESHOLD: f64 = 1e-8;

    /// Lognormal (Black-76) implied volatility using Hagan's approximation.
    ///
    /// Hagan et al. (2002) Black-76 implied volatility for a given forward, strike, and expiry.
    ///
    /// # Arguments
    ///
    /// * `f` - Forward rate
    /// * `k` - Strike rate
    /// * `t` - Time to expiry in years
    ///
    /// # Returns
    ///
    /// Black-76 implied volatility (lognormal). Returns `alpha` for the ATM case.
    ///
    /// Returns `f64::NAN` for degenerate inputs (non-positive forward/strike,
    /// non-positive expiry, or a χ(z) breakdown). Callers on pricing/risk paths
    /// must guard the result with `is_finite()`, or use the fallible
    /// [`implied_vol_lognormal`](Self::implied_vol_lognormal), since a
    /// silent NaN poisons Black-76 pricing and compensated summations downstream.
    ///
    /// # Special Cases
    ///
    /// - ATM (`f ≈ k`): Uses the simplified ATM formula for numerical stability
    /// - β = 0: Degenerates to normal SABR; lognormal vol is approximated
    /// - β = 1: Standard lognormal SABR formula
    fn implied_vol_lognormal_unchecked(&self, f: f64, k: f64, t: f64) -> f64 {
        let alpha = self.alpha;
        let beta = self.beta;
        let rho = self.rho;
        let nu = self.nu;

        let (f, k) = if let Some(s) = self.shift {
            (f + s, k + s)
        } else {
            (f, k)
        };

        // Guard: both forward and strike must be positive for lognormal model
        if f <= 0.0 || k <= 0.0 || t <= 0.0 {
            return f64::NAN;
        }

        // No special-case for ν → 0: the general Hagan expansion is continuous
        // in ν. z = (ν/α)(FK)^((1-β)/2) ln(F/K) → 0 and z/χ(z) → 1 (handled by
        // the small-z Taylor branch in `chi`), while the (1 + [...]T) correction
        // smoothly degenerates to the pure CEV correction. A hard ν threshold
        // with a corrections-free CEV formula introduced a vol discontinuity.

        let fk = f * k;
        let one_minus_beta = 1.0 - beta;
        let log_fk = (f / k).ln();

        // ATM case: use the stable formula once log-moneyness is tiny, including
        // low-rate environments where a purely relative |f-k| threshold becomes ineffective.
        if log_fk.abs() <= Self::LOGNORMAL_ATM_LOG_MONEYNESS_THRESHOLD {
            return self.atm_vol_lognormal(f, t);
        }

        // z = (ν/α) * (FK)^((1-β)/2) * ln(F/K)
        let fk_mid = fk.powf(one_minus_beta / 2.0);
        let z = (nu / alpha) * fk_mid * log_fk;

        // χ(z) = log[(√(1 - 2ρz + z²) + z - ρ) / (1 - ρ)]
        let chi_z = chi(z, rho).unwrap_or(f64::NAN);

        let numerator = alpha;

        // Denominator: (FK)^((1-β)/2) * [1 + (1-β)²/24 * log²(F/K) + (1-β)⁴/1920 * log⁴(F/K)]
        let log_fk_sq = log_fk * log_fk;
        let omb2 = one_minus_beta * one_minus_beta;
        let denominator =
            fk_mid * (1.0 + omb2 / 24.0 * log_fk_sq + omb2 * omb2 / 1920.0 * log_fk_sq * log_fk_sq);

        // First-order correction factor
        // 1 + [ (1-β)²/24 * α² / (FK)^(1-β)
        //      + ¼ * ρβνα / (FK)^((1-β)/2)
        //      + (2-3ρ²)/24 * ν² ] * T
        let fk_omb = fk.powf(one_minus_beta);
        let correction = 1.0
            + (omb2 / 24.0 * alpha * alpha / fk_omb
                + 0.25 * rho * beta * nu * alpha / fk_mid
                + (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu)
                * t;

        // σ_B(K) = (z / χ(z)) × (α / denominator) × correction
        let z_over_chi = if chi_z.abs() < 1e-14 {
            1.0 // L'Hôpital at z=0
        } else {
            z / chi_z
        };

        numerator / denominator * z_over_chi * correction
    }

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
        let beta = if self.beta < super::model::BETA_SNAP_TOL {
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

        let z_over_chi = if chi_z.abs() < 1e-14 { 1.0 } else { z / chi_z };

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
    /// Returns [`InputError::Invalid`](finstack_quant_core::error::InputError::Invalid) when
    /// the underlying expansion yields a non-finite volatility.
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
        let v = self.implied_vol_lognormal_unchecked(f, k, t);
        if v.is_finite() && v >= 0.0 {
            Ok(v)
        } else {
            Err(finstack_quant_core::error::InputError::Invalid.into())
        }
    }

    /// Normal SABR implied volatility with checked error semantics.
    ///
    /// # Errors
    ///
    /// - Returns a [`Validation`](finstack_quant_core::Error::Validation) error when
    ///   `f·k ≤ 0` (after any configured shift) with `beta > 0`: the CEV
    ///   backbone is not shift-invariant, so cross-zero quotes require an
    ///   explicit shift via [`new_with_shift`](Self::new_with_shift).
    /// - Returns [`InputError::Invalid`](finstack_quant_core::error::InputError::Invalid)
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
        if sf * sk <= 0.0
            && self.beta >= super::model::BETA_SNAP_TOL
            && (sf - sk).abs() > 1e-12 * sf.abs().max(1e-10)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "SABR implied_vol_normal: forward*strike <= 0 (F={sf}, K={sk} after shift) with \
                 beta = {} > 0. The CEV backbone is not shift-invariant; configure an explicit \
                 shift via SabrParameters::with_shift for negative/cross-zero rates.",
                self.beta
            )));
        }
        let v = self.implied_vol_normal_unchecked(f, k, t);
        if v.is_finite() && v >= 0.0 {
            Ok(v)
        } else {
            Err(finstack_quant_core::error::InputError::Invalid.into())
        }
    }

    /// ATM lognormal volatility (simplified formula when F ≈ K).
    ///
    /// ```text
    /// σ_ATM = α / F^(1-β) * [1 + ((1-β)²/24 * α²/F^(2(1-β)) + ¼ρβνα/F^(1-β) + (2-3ρ²)/24 * ν²) * T]
    /// ```
    fn atm_vol_lognormal(&self, f: f64, t: f64) -> f64 {
        let alpha = self.alpha;
        let beta = self.beta;
        let rho = self.rho;
        let nu = self.nu;

        let omb = 1.0 - beta;
        let f_safe = f.max(1e-10);
        let f_omb = f_safe.powf(omb);

        let base = alpha / f_omb;

        let correction = 1.0
            + (omb * omb / 24.0 * alpha * alpha / (f_omb * f_omb)
                + 0.25 * rho * beta * nu * alpha / f_omb
                + (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu)
                * t;

        base * correction
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
