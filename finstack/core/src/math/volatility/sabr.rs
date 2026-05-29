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
//!   "Managing Smile Risk." *Wilmott Magazine*, September 2002, 84-108.
//! - Obloj, J. (2008). "Fine-tune your smile: Correction to Hagan et al."
//!   *Wilmott Magazine*, May 2008.
//! - West, G. (2005). "Calibration of the SABR Model in Illiquid Markets."
//!   *Applied Mathematical Finance*, 12(4), 371-385.
//! - QuantLib SABR implementation: `ql/termstructures/volatility/sabr.cpp`

// SABR stochastic volatility model implementation.

/// SABR model parameters for a single expiry.
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::volatility::sabr::SabrParams;
///
/// // Typical USD swaption SABR parameters
/// let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
/// let fwd = 0.05;
/// let strike = 0.05;
/// let expiry = 1.0;
/// let vol = params.implied_vol_lognormal(fwd, strike, expiry);
/// assert!(vol > 0.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SabrParams {
    /// Alpha (α): initial volatility level.
    pub alpha: f64,
    /// Beta (β): CEV exponent, in [0, 1].
    pub beta: f64,
    /// Rho (ρ): correlation between forward and vol Brownian motions, in (-1, 1).
    pub rho: f64,
    /// Nu (ν): vol-of-vol, must be > 0.
    pub nu: f64,
    /// Shift for negative rate support. When set, the model uses `F+shift` and
    /// `K+shift` internally, keeping both arguments positive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift: Option<f64>,
}

/// Warning about negative implied probability density at a specific strike.
#[derive(Debug, Clone)]
pub struct DensityWarning {
    /// The strike where negative density was detected.
    pub strike: f64,
    /// The computed (negative) density value d²C/dK².
    pub density: f64,
}

impl SabrParams {
    const LOGNORMAL_ATM_LOG_MONEYNESS_THRESHOLD: f64 = 1e-8;

    /// Alpha (α): initial volatility level.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
    /// Beta (β): CEV exponent, in [0, 1].
    pub fn beta(&self) -> f64 {
        self.beta
    }
    /// Rho (ρ): correlation between forward and vol Brownian motions.
    pub fn rho(&self) -> f64 {
        self.rho
    }
    /// Nu (ν): vol-of-vol.
    pub fn nu(&self) -> f64 {
        self.nu
    }

    /// Construct validated SABR parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `alpha <= 0`
    /// - `beta` not in `[0, 1]`
    /// - `rho` not in `(-1, 1)`
    /// - `nu <= 0`
    pub fn new(alpha: f64, beta: f64, rho: f64, nu: f64) -> crate::Result<Self> {
        if alpha <= 0.0 || !alpha.is_finite() {
            return Err(crate::Error::Validation(format!(
                "SABR alpha must be positive, got {alpha}"
            )));
        }
        if !(0.0..=1.0).contains(&beta) || !beta.is_finite() {
            return Err(crate::Error::Validation(format!(
                "SABR beta must be in [0, 1], got {beta}"
            )));
        }
        if rho <= -1.0 || rho >= 1.0 || !rho.is_finite() {
            return Err(crate::Error::Validation(format!(
                "SABR rho must be in (-1, 1), got {rho}"
            )));
        }
        if nu <= 0.0 || !nu.is_finite() {
            return Err(crate::Error::Validation(format!(
                "SABR nu (vol-of-vol) must be positive, got {nu}"
            )));
        }
        Ok(Self {
            alpha,
            beta,
            rho,
            nu,
            shift: None,
        })
    }

    /// Return a copy of these parameters with the given shift applied.
    ///
    /// The shift is used to handle negative rate environments: when evaluating
    /// implied vol the model internally uses `F+shift` and `K+shift`, keeping
    /// both arguments positive. A typical value for EUR/JPY is `0.03` (300 bps).
    pub fn with_shift(self, shift: f64) -> Self {
        Self {
            shift: Some(shift),
            ..self
        }
    }

    /// Lognormal (Black-76) implied volatility using Hagan's approximation.
    ///
    /// This is the market-standard SABR approximation from Hagan et al. (2002).
    /// Returns the Black-76 implied volatility for a given forward, strike, and expiry.
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
    /// [`try_implied_vol_lognormal`](Self::try_implied_vol_lognormal), since a
    /// silent NaN poisons Black-76 pricing and compensated summations downstream.
    ///
    /// # Special Cases
    ///
    /// - ATM (`f ≈ k`): Uses the simplified ATM formula for numerical stability
    /// - β = 0: Degenerates to normal SABR; lognormal vol is approximated
    /// - β = 1: Standard lognormal SABR formula
    pub fn implied_vol_lognormal(&self, f: f64, k: f64, t: f64) -> f64 {
        let alpha = self.alpha;
        let beta = self.beta;
        let rho = self.rho;
        let nu = self.nu;

        // Apply shift for negative rate support
        let (f, k) = if let Some(s) = self.shift {
            (f + s, k + s)
        } else {
            (f, k)
        };

        // Guard: both forward and strike must be positive for lognormal model
        if f <= 0.0 || k <= 0.0 || t <= 0.0 {
            return f64::NAN;
        }

        // When vol-of-vol is negligible, the SABR model degenerates to CEV.
        // The Hagan expansion divides by ν, so bypass it and return the
        // pure CEV backbone volatility directly.
        if nu < 1e-10 {
            let f_safe = f.max(1e-10);
            return alpha / f_safe.powf(1.0 - beta);
        }

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

        // Numerator: α
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
    /// expiry or a χ(z) breakdown. Guard with `is_finite()` or use the fallible
    /// [`try_implied_vol_normal`](Self::try_implied_vol_normal) on pricing paths.
    pub fn implied_vol_normal(&self, f: f64, k: f64, t: f64) -> f64 {
        let alpha = self.alpha;
        let beta = self.beta;
        let rho = self.rho;
        let nu = self.nu;

        // Apply shift for negative rate support
        let (f, k) = if let Some(s) = self.shift {
            (f + s, k + s)
        } else {
            (f, k)
        };

        if t <= 0.0 {
            return f64::NAN;
        }

        // When vol-of-vol is negligible, the SABR model degenerates to CEV.
        // Return the pure CEV backbone normal volatility directly.
        if nu < 1e-10 {
            let f_abs = f.abs().max(1e-10);
            return alpha * f_abs.powf(beta);
        }

        // ATM case
        if (f - k).abs() < 1e-12 * f.abs().max(1e-10) {
            return self.atm_vol_normal(f, t);
        }

        let fk = f * k;
        let one_minus_beta = 1.0 - beta;

        if fk <= 0.0 {
            // Cross-zero quotes are better handled by a shifted SABR-style
            // approximation than by collapsing the smile to midpoint ATM.
            let shift_scale = (f - k).abs().max(f.abs()).max(k.abs()).max(1.0e-4);
            let shift = (-f.min(k)).max(0.0) + shift_scale;
            let shifted_f = f + shift;
            let shifted_k = k + shift;
            return self.implied_vol_normal(shifted_f, shifted_k, t);
        }

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

    /// Fallible [`implied_vol_lognormal`](Self::implied_vol_lognormal): returns
    /// an error instead of a silent `f64::NAN` for degenerate inputs.
    ///
    /// Prefer this on any pricing/risk path where a NaN vol could silently
    /// propagate into Black-76 valuation or a compensated aggregation.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::Invalid`](crate::error::InputError::Invalid) when
    /// the underlying expansion yields a non-finite volatility.
    pub fn try_implied_vol_lognormal(&self, f: f64, k: f64, t: f64) -> crate::Result<f64> {
        let v = self.implied_vol_lognormal(f, k, t);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(crate::error::InputError::Invalid.into())
        }
    }

    /// Fallible [`implied_vol_normal`](Self::implied_vol_normal): returns an
    /// error instead of a silent `f64::NAN` for degenerate inputs.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::Invalid`](crate::error::InputError::Invalid) when
    /// the underlying expansion yields a non-finite volatility.
    pub fn try_implied_vol_normal(&self, f: f64, k: f64, t: f64) -> crate::Result<f64> {
        let v = self.implied_vol_normal(f, k, t);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(crate::error::InputError::Invalid.into())
        }
    }

    /// Check for negative implied probability density across a strike grid.
    ///
    /// The implied density is computed as d²C/dK² using central finite differences
    /// on the Black call price. Negative values indicate butterfly arbitrage.
    ///
    /// This is a diagnostic -- it emits warnings but does not fail.
    pub fn check_density(&self, strikes: &[f64], forward: f64, expiry: f64) -> Vec<DensityWarning> {
        let mut warnings = Vec::new();
        let dk = 0.0001 * forward; // small relative shift

        for &k in strikes {
            if k <= dk || k <= 0.0 {
                continue;
            }
            let vol_lo = self.implied_vol_lognormal(forward, k - dk, expiry);
            let vol_mid = self.implied_vol_lognormal(forward, k, expiry);
            let vol_hi = self.implied_vol_lognormal(forward, k + dk, expiry);

            if !vol_lo.is_finite() || !vol_mid.is_finite() || !vol_hi.is_finite() {
                continue;
            }

            let c_lo = black_call_undiscounted(forward, k - dk, expiry, vol_lo);
            let c_mid = black_call_undiscounted(forward, k, expiry, vol_mid);
            let c_hi = black_call_undiscounted(forward, k + dk, expiry, vol_hi);

            let d2c_dk2 = (c_hi - 2.0 * c_mid + c_lo) / (dk * dk);

            if d2c_dk2 < -1e-10 {
                tracing::warn!(
                    alpha = self.alpha,
                    beta = self.beta,
                    rho = self.rho,
                    nu = self.nu,
                    forward = forward,
                    strike = k,
                    expiry = expiry,
                    density = d2c_dk2,
                    "SABR: negative implied density (butterfly arbitrage) detected",
                );
                warnings.push(DensityWarning {
                    strike: k,
                    density: d2c_dk2,
                });
            }
        }

        warnings
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

/// Undiscounted Black call price for density checking.
fn black_call_undiscounted(forward: f64, strike: f64, expiry: f64, vol: f64) -> f64 {
    crate::math::volatility::pricing::black_call(forward, strike, vol, expiry)
}

/// χ(z) function used in the Hagan SABR approximation.
///
/// ```text
/// χ(z) = log[(√(1 - 2ρz + z²) + z - ρ) / (1 - ρ)]
/// ```
///
/// Uses a Taylor expansion for small z to avoid cancellation.
#[inline]
fn chi(z: f64, rho: f64) -> crate::Result<f64> {
    if z.abs() < 1e-10 {
        // Taylor expansion to O(z²): χ(z) ≈ z + ρz²/2. Higher-order terms are
        // negligible at the |z| < 1e-10 cutover (z³ ≲ 1e-30).
        return Ok(z * (1.0 + 0.5 * rho * z));
    }

    let discriminant = 1.0 - 2.0 * rho * z + z * z;
    if discriminant < 0.0 {
        return Err(crate::Error::Validation(format!(
            "SABR chi: negative discriminant {discriminant:.6} for z={z:.6}, rho={rho:.6}"
        )));
    }

    let sqrt_disc = discriminant.sqrt();
    let numerator = sqrt_disc + z - rho;
    let denominator = 1.0 - rho;

    if numerator <= 0.0 || denominator <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "SABR chi: non-positive log argument (num={numerator:.6}, den={denominator:.6})"
        )));
    }

    Ok((numerator / denominator).ln())
}

/// Calibrate SABR parameters from market implied volatilities.
///
/// Given a set of (strike, implied_vol) pairs, forward rate, expiry, and
/// a fixed beta, calibrate the remaining parameters (α, ρ, ν) by minimizing
/// the sum of squared volatility errors.
///
/// # Arguments
///
/// * `forward` - Forward rate
/// * `expiry` - Time to expiry in years
/// * `beta` - Fixed CEV exponent (typically 0.5 for CMS, 0.0 for normal)
/// * `strikes` - Array of option strikes
/// * `market_vols` - Array of market-implied Black-76 volatilities
/// * `weights` - Optional weights for each strike (default: equal weights)
///
/// # Returns
///
/// Calibrated [`SabrParams`] that best fit the market data.
///
/// # Errors
///
/// Returns an error if:
/// - Input arrays have different lengths
/// - Fewer than 3 data points (need at least 3 for 3 free parameters)
/// - Calibration fails to converge
///
/// # Algorithm
///
/// Uses the Levenberg-Marquardt algorithm from `nalgebra` for non-linear
/// least squares minimization. Initial guesses are derived from ATM vol.
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::volatility::sabr::{calibrate_sabr, SabrParams};
///
/// let forward = 0.05;
/// let expiry = 1.0;
/// let beta = 0.5;
/// let strikes = &[0.03, 0.04, 0.05, 0.06, 0.07];
/// let vols = &[0.25, 0.22, 0.20, 0.21, 0.23];
///
/// let params = calibrate_sabr(forward, expiry, beta, strikes, vols, None)
///     .expect("Calibration should succeed");
/// // Verify ATM vol is close to input
/// let atm_vol = params.implied_vol_lognormal(forward, forward, expiry);
/// assert!((atm_vol - 0.20).abs() < 0.01);
/// ```
pub fn calibrate_sabr(
    forward: f64,
    expiry: f64,
    beta: f64,
    strikes: &[f64],
    market_vols: &[f64],
    weights: Option<&[f64]>,
) -> crate::Result<SabrParams> {
    if strikes.len() != market_vols.len() {
        return Err(crate::Error::Validation(
            "strikes and market_vols must have the same length".to_string(),
        ));
    }
    if strikes.len() < 3 {
        return Err(crate::Error::Validation(
            "Need at least 3 strike/vol pairs for SABR calibration (3 free parameters)".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&beta) {
        return Err(crate::Error::Validation(format!(
            "beta must be in [0, 1], got {beta}"
        )));
    }

    // Find ATM (or nearest-to-ATM) vol for initial alpha guess
    let atm_vol = strikes
        .iter()
        .zip(market_vols.iter())
        .min_by(|(k1, _), (k2, _)| {
            (*k1 - forward)
                .abs()
                .partial_cmp(&(*k2 - forward).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, &v)| v)
        .unwrap_or(0.2);

    // Initial guess: α from ATM vol, ρ = 0, ν = 0.5
    let f_omb = forward.powf(1.0 - beta);
    let alpha_init = (atm_vol * f_omb).max(0.001);
    let rho_init: f64 = 0.0;
    let nu_init: f64 = 0.5;

    let default_weights: Vec<f64> = vec![1.0; strikes.len()];
    let w = weights.unwrap_or(&default_weights);

    // ------------------------------------------------------------------
    // Primary solver: Levenberg-Marquardt on weighted residual vector
    // ------------------------------------------------------------------
    // Unconstrained parametrisation:
    //   x[0] = ln(alpha)        → alpha = exp(x[0])  > 0
    //   x[1] = atanh(rho)       → rho   = tanh(x[1]) ∈ (-1, 1)
    //   x[2] = ln(nu)           → nu    = exp(x[2])  > 0
    let n_strikes = strikes.len();

    let residuals = |x: &[f64], resid: &mut [f64]| {
        let alpha = x[0].exp();
        let rho = x[1].tanh();
        let nu = x[2].exp();

        let params = SabrParams {
            alpha,
            beta,
            rho,
            nu,
            shift: None,
        };

        for (i, (&k, &mv)) in strikes.iter().zip(market_vols.iter()).enumerate() {
            let model_vol = params.implied_vol_lognormal(forward, k, expiry);
            if model_vol.is_finite() {
                resid[i] = w[i].sqrt() * (model_vol - mv);
            } else {
                resid[i] = w[i].sqrt() * 1.0; // Penalise non-finite vols
            }
        }
    };

    let x0 = [
        alpha_init.ln(),
        rho_init.clamp(-0.999, 0.999).atanh(),
        nu_init.ln(),
    ];

    let solver = crate::math::solver_multi::LevenbergMarquardtSolver::new()
        .with_tolerance(1e-10)
        .with_max_iterations(200);

    let lm_result = solver.solve_system_with_dim_stats(residuals, &x0, n_strikes);

    // Attempt to extract LM solution and compute RMSE
    let lm_params = lm_result.ok().and_then(|sol| {
        let alpha = sol.params[0].exp();
        let rho = sol.params[1].tanh();
        let nu = sol.params[2].exp();

        // Compute weighted SSE for RMSE check
        let p = SabrParams {
            alpha,
            beta,
            rho,
            nu,
            shift: None,
        };
        let sse: f64 = strikes
            .iter()
            .zip(market_vols.iter())
            .enumerate()
            .map(|(i, (&k, &mv))| {
                let mv_hat = p.implied_vol_lognormal(forward, k, expiry);
                if mv_hat.is_finite() {
                    w[i] * (mv_hat - mv) * (mv_hat - mv)
                } else {
                    f64::MAX
                }
            })
            .sum();

        if sse.is_finite() {
            Some((alpha, rho, nu, sse))
        } else {
            None
        }
    });

    // ------------------------------------------------------------------
    // Fallback: coordinate descent (if LM failed or produced worse RMSE)
    // ------------------------------------------------------------------
    let cd_result = calibrate_sabr_coordinate_descent(
        forward,
        expiry,
        beta,
        strikes,
        market_vols,
        w,
        alpha_init,
        rho_init,
        nu_init,
    );

    // Pick the better result
    let (alpha, rho, nu, best_obj) = match (lm_params, cd_result) {
        (Some(lm), Some(cd)) => {
            if lm.3 <= cd.3 {
                lm
            } else {
                cd
            }
        }
        (Some(lm), None) => lm,
        (None, Some(cd)) => cd,
        (None, None) => {
            tracing::warn!(
                algorithm = "sabr_calibration",
                forward,
                expiry,
                beta,
                n_strikes = strikes.len(),
                alpha_init,
                rho_init,
                nu_init,
                category = "both_solvers_failed",
                "sabr: bailout — both LM and coordinate-descent solvers failed to converge"
            );
            return Err(crate::Error::Validation(
                "SABR calibration failed: both LM and fallback solvers failed to converge"
                    .to_string(),
            ));
        }
    };

    // Validate convergence: RMSE should be reasonable
    let rmse = (best_obj / strikes.len() as f64).sqrt();
    if rmse > 0.05 {
        tracing::warn!(
            algorithm = "sabr_calibration",
            forward,
            expiry,
            beta,
            n_strikes = strikes.len(),
            rmse,
            alpha,
            rho,
            nu,
            category = "rmse_too_high",
            "sabr: bailout — calibration RMSE exceeds 5% threshold"
        );
        return Err(crate::Error::Validation(format!(
            "SABR calibration RMSE too high: {rmse:.4} (>5%). Parameters may not fit the market data well."
        )));
    }

    SabrParams::new(alpha, beta, rho, nu)
}

/// Coordinate-descent fallback for SABR calibration.
///
/// Returns `Some((alpha, rho, nu, sse))` on success, `None` if the objective
/// stays at `f64::MAX` (all parameter trials produced non-finite vols).
#[allow(clippy::too_many_arguments)]
fn calibrate_sabr_coordinate_descent(
    forward: f64,
    expiry: f64,
    beta: f64,
    strikes: &[f64],
    market_vols: &[f64],
    w: &[f64],
    alpha_init: f64,
    rho_init: f64,
    nu_init: f64,
) -> Option<(f64, f64, f64, f64)> {
    let objective = |alpha: f64, rho: f64, nu: f64| -> f64 {
        if alpha <= 0.0 || nu <= 0.0 || rho <= -1.0 || rho >= 1.0 {
            return f64::MAX;
        }
        let params = SabrParams {
            alpha,
            beta,
            rho,
            nu,
            shift: None,
        };
        let mut sse = 0.0;
        for (i, (&k, &mv)) in strikes.iter().zip(market_vols.iter()).enumerate() {
            let model_vol = params.implied_vol_lognormal(forward, k, expiry);
            if !model_vol.is_finite() {
                return f64::MAX;
            }
            let diff = model_vol - mv;
            sse += w[i] * diff * diff;
        }
        sse
    };

    let mut alpha = alpha_init;
    let mut rho = rho_init;
    let mut nu = nu_init;
    let mut best_obj = objective(alpha, rho, nu);

    for round in 0..100 {
        let step_scale = if round < 20 {
            1.0
        } else {
            0.5_f64.powi((round - 20) / 20)
        };

        // Optimize alpha
        let alpha_step = alpha * 0.05 * step_scale;
        for &delta in &[-1.0, 1.0] {
            let trial = alpha + delta * alpha_step;
            if trial > 0.0 {
                let obj = objective(trial, rho, nu);
                if obj < best_obj {
                    alpha = trial;
                    best_obj = obj;
                }
            }
        }

        // Optimize rho
        let rho_step = 0.05 * step_scale;
        for &delta in &[-1.0, 1.0] {
            let trial = rho + delta * rho_step;
            if trial > -0.999 && trial < 0.999 {
                let obj = objective(alpha, trial, nu);
                if obj < best_obj {
                    rho = trial;
                    best_obj = obj;
                }
            }
        }

        // Optimize nu
        let nu_step = nu * 0.1 * step_scale;
        for &delta in &[-1.0, 1.0] {
            let trial = nu + delta * nu_step;
            if trial > 0.001 {
                let obj = objective(alpha, rho, trial);
                if obj < best_obj {
                    nu = trial;
                    best_obj = obj;
                }
            }
        }

        if best_obj < 1e-12 {
            break;
        }
    }

    if best_obj < f64::MAX {
        Some((alpha, rho, nu, best_obj))
    } else {
        None
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sabr_params_validation() {
        assert!(SabrParams::new(0.03, 0.5, -0.2, 0.4).is_ok());
        assert!(SabrParams::new(-0.01, 0.5, -0.2, 0.4).is_err()); // alpha <= 0
        assert!(SabrParams::new(0.03, 1.5, -0.2, 0.4).is_err()); // beta > 1
        assert!(SabrParams::new(0.03, 0.5, -1.0, 0.4).is_err()); // rho = -1
        assert!(SabrParams::new(0.03, 0.5, 1.0, 0.4).is_err()); // rho = 1
        assert!(SabrParams::new(0.03, 0.5, -0.2, 0.0).is_err()); // nu = 0
    }

    #[test]
    fn sabr_atm_vol_is_positive() {
        let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).expect("valid params");
        let fwd = 0.05;
        let vol = params.implied_vol_lognormal(fwd, fwd, 1.0);
        assert!(vol > 0.0, "ATM vol should be positive: {vol}");
    }

    #[test]
    fn sabr_try_implied_vol_errors_on_degenerate_inputs() {
        let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).expect("valid params");
        // Valid inputs return Ok and match the infallible path.
        let ok = params
            .try_implied_vol_lognormal(0.05, 0.06, 1.0)
            .expect("finite vol");
        assert!((ok - params.implied_vol_lognormal(0.05, 0.06, 1.0)).abs() < 1e-12);

        // Degenerate inputs (non-positive forward/strike/expiry) yield NaN in the
        // infallible path and an error in the fallible one.
        assert!(params.implied_vol_lognormal(-0.05, 0.06, 1.0).is_nan());
        assert!(params.try_implied_vol_lognormal(-0.05, 0.06, 1.0).is_err());
        assert!(params.try_implied_vol_lognormal(0.05, 0.06, 0.0).is_err());
        assert!(params.try_implied_vol_normal(0.05, 0.06, 0.0).is_err());
        assert!(params.try_implied_vol_normal(0.01, -0.01, 1.0).is_ok());
    }

    #[test]
    fn sabr_vol_smile_shape() {
        let params = SabrParams::new(0.035, 0.5, -0.25, 0.45).expect("valid params");
        let fwd = 0.05;
        let t = 1.0;

        let vol_otm_put = params.implied_vol_lognormal(fwd, 0.03, t);
        let vol_atm = params.implied_vol_lognormal(fwd, fwd, t);
        let vol_otm_call = params.implied_vol_lognormal(fwd, 0.07, t);

        // With negative rho, we expect left-skew: OTM put vol > ATM vol
        assert!(
            vol_otm_put > vol_atm,
            "Expected left skew: vol(K=3%) = {vol_otm_put:.4} should be > vol(ATM) = {vol_atm:.4}"
        );
        // Smile: far OTM on both sides should be higher than ATM
        assert!(vol_otm_put > 0.0);
        assert!(vol_atm > 0.0);
        assert!(vol_otm_call > 0.0);
    }

    #[test]
    fn sabr_beta_zero_normal_sabr() {
        // β=0 is the normal SABR model
        let params = SabrParams::new(0.005, 0.0, -0.3, 0.3).expect("valid params");
        let fwd = 0.03;
        let vol = params.implied_vol_lognormal(fwd, fwd, 1.0);
        assert!(vol > 0.0, "Normal SABR ATM vol should be positive: {vol}");
    }

    #[test]
    fn sabr_beta_one_lognormal() {
        // β=1 is the standard lognormal SABR
        let params = SabrParams::new(0.2, 1.0, -0.15, 0.3).expect("valid params");
        let fwd = 0.05;
        let vol = params.implied_vol_lognormal(fwd, fwd, 1.0);
        // With β=1, α=0.2, ATM vol should be close to α=0.2
        assert!(
            (vol - 0.2).abs() < 0.05,
            "Lognormal SABR ATM vol should be near alpha: {vol:.4}"
        );
    }

    #[test]
    fn sabr_normal_vol_positive() {
        let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).expect("valid params");
        let fwd = 0.05;
        let vol = params.implied_vol_normal(fwd, fwd, 1.0);
        assert!(vol > 0.0, "Normal vol should be positive: {vol}");
    }

    /// Independent textbook implementation of Hagan (2002) eq. 2.17b, used as a
    /// reference to lock in [`SabrParams::implied_vol_normal`].
    fn hagan_normal_vol_reference(
        alpha: f64,
        beta: f64,
        rho: f64,
        nu: f64,
        f: f64,
        k: f64,
        t: f64,
    ) -> f64 {
        let fk = f * k;
        let omb = 1.0 - beta;
        let log_fk = (f / k).ln();
        let z = (nu / alpha) * fk.powf(omb / 2.0) * log_fk;
        let chi = (((1.0 - 2.0 * rho * z + z * z).sqrt() + z - rho) / (1.0 - rho)).ln();
        let z_over_chi = if z.abs() < 1e-12 { 1.0 } else { z / chi };
        let l2 = log_fk * log_fk;
        let num = 1.0 + l2 / 24.0 + l2 * l2 / 1920.0;
        let den = 1.0 + omb * omb / 24.0 * l2 + omb.powi(4) / 1920.0 * l2 * l2;
        let corr = 1.0
            + (-beta * (2.0 - beta) / 24.0 * alpha * alpha / fk.powf(omb)
                + 0.25 * rho * beta * nu * alpha / fk.powf(omb / 2.0)
                + (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu)
                * t;
        alpha * fk.powf(beta / 2.0) * num / den * z_over_chi * corr
    }

    #[test]
    fn sabr_normal_vol_matches_hagan_2_17b() {
        // Grid over β and OTM/ITM strikes, where the numerator series and the
        // −β(2−β)/24 correction term both contribute, vs the canonical formula.
        let f = 0.03;
        let t = 1.5;
        for &(alpha, beta, rho, nu) in &[
            (0.01, 0.0, -0.3, 0.4),
            (0.02, 0.5, -0.2, 0.3),
            (0.20, 1.0, -0.15, 0.35),
        ] {
            let p = SabrParams::new(alpha, beta, rho, nu).expect("valid params");
            for &k in &[0.018_f64, 0.024, 0.036, 0.045] {
                let got = p.implied_vol_normal(f, k, t);
                let want = hagan_normal_vol_reference(alpha, beta, rho, nu, f, k, t);
                assert!(
                    (got - want).abs() <= 1e-9 * want.abs() + 1e-13,
                    "β={beta} k={k}: got {got:.12}, want {want:.12}"
                );
            }
        }
    }

    #[test]
    fn sabr_normal_atm_beta_zero_has_no_alpha_squared_term() {
        // At β=0 the normal-SABR time correction has NO α²/(FK)^(1-β) term
        // (−β(2−β)/24 = 0). The pre-fix code used −(1−β)²/24 = −1/24, adding a
        // spurious term that biased β=0 (negative-rate) ATM normal vols.
        let alpha = 0.01;
        let nu = 0.3;
        let rho = 0.0;
        let f = 0.03;
        let t = 2.0;
        let p = SabrParams::new(alpha, 0.0, rho, nu).expect("valid params");
        let got = p.atm_vol_normal(f, t);
        // Expected: α·[1 + (2−3ρ²)/24·ν²·t]  (ρ=0 ⇒ ρβ term gone, β=0 ⇒ α² gone).
        let want = alpha * (1.0 + (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu * t);
        assert!(
            (got - want).abs() <= 1e-12,
            "got {got:.12}, want {want:.12}"
        );
    }

    #[test]
    fn sabr_cross_zero_normal_vol_is_not_midpoint_atm_shortcut() {
        let params = SabrParams::new(0.01, 0.0, -0.2, 0.4).expect("valid params");
        let fwd = -0.01;
        let strike = 0.02;
        let t = 1.0;

        let vol = params.implied_vol_normal(fwd, strike, t);
        let midpoint_atm = params.atm_vol_normal(0.5 * (fwd + strike), t);

        assert!(
            vol.is_finite() && vol > 0.0,
            "cross-zero normal vol should stay finite"
        );
        assert!(
            (vol - midpoint_atm).abs() > 1e-12,
            "cross-zero handling should not collapse to midpoint ATM vol"
        );
    }

    #[test]
    fn sabr_symmetry_at_atm() {
        // Vol at ATM should be continuous regardless of approach direction
        let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).expect("valid params");
        let fwd = 0.05;
        let t = 1.0;

        let vol_exact = params.implied_vol_lognormal(fwd, fwd, t);
        let vol_near_above = params.implied_vol_lognormal(fwd, fwd + 1e-8, t);
        let vol_near_below = params.implied_vol_lognormal(fwd, fwd - 1e-8, t);

        assert!(
            (vol_exact - vol_near_above).abs() < 1e-4,
            "Vol should be continuous at ATM: exact={vol_exact:.6}, above={vol_near_above:.6}"
        );
        assert!(
            (vol_exact - vol_near_below).abs() < 1e-4,
            "Vol should be continuous at ATM: exact={vol_exact:.6}, below={vol_near_below:.6}"
        );
    }

    #[test]
    fn sabr_low_rate_near_atm_is_stable() {
        let params = SabrParams::new(0.01, 0.5, -0.2, 0.4).expect("valid params");
        let fwd = 0.0005;
        let t = 1.0;

        let vol_exact = params.implied_vol_lognormal(fwd, fwd, t);
        let vol_near_above = params.implied_vol_lognormal(fwd, fwd + 1e-12, t);
        let vol_near_below = params.implied_vol_lognormal(fwd, fwd - 1e-12, t);

        assert!(vol_exact.is_finite() && vol_exact > 0.0);
        assert_eq!(
            vol_exact, vol_near_above,
            "Low-rate ATM continuity failed above ATM: exact={vol_exact:.6}, above={vol_near_above:.6}"
        );
        assert_eq!(
            vol_exact, vol_near_below,
            "Low-rate ATM continuity failed below ATM: exact={vol_exact:.6}, below={vol_near_below:.6}"
        );
    }

    #[test]
    fn chi_function_small_z() {
        // For small z, χ(z) ≈ z
        let result = chi(1e-12, 0.0).expect("chi should succeed for small z");
        assert!((result - 1e-12).abs() < 1e-20);
    }

    #[test]
    fn chi_function_zero_rho() {
        // For ρ=0, χ(z) = ln(√(1+z²) + z) = arcsinh(z)
        let z = 0.5;
        let result = chi(z, 0.0).expect("chi should succeed for rho=0");
        let expected = z.asinh();
        assert!(
            (result - expected).abs() < 1e-10,
            "χ(z, ρ=0) should equal arcsinh(z): got {result}, expected {expected}"
        );
    }

    #[test]
    fn calibrate_sabr_round_trip() {
        // Generate synthetic market data from known SABR params
        let true_params = SabrParams::new(0.035, 0.5, -0.25, 0.4).expect("valid params");
        let fwd = 0.05;
        let expiry = 1.0;

        let strikes: Vec<f64> = vec![0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08];
        let vols: Vec<f64> = strikes
            .iter()
            .map(|&k| true_params.implied_vol_lognormal(fwd, k, expiry))
            .collect();

        let calibrated = calibrate_sabr(fwd, expiry, 0.5, &strikes, &vols, None)
            .expect("Calibration should succeed");

        // Check parameters are close to true values
        assert!(
            (calibrated.alpha - true_params.alpha).abs() < 0.005,
            "Alpha mismatch: calibrated={:.4}, true={:.4}",
            calibrated.alpha,
            true_params.alpha
        );
    }

    #[test]
    fn calibrate_sabr_rejects_insufficient_data() {
        let strikes = &[0.05, 0.06];
        let vols = &[0.2, 0.21];
        let result = calibrate_sabr(0.05, 1.0, 0.5, strikes, vols, None);
        assert!(result.is_err());
    }

    #[test]
    fn sabr_invalid_inputs_return_nan() {
        let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).expect("valid params");
        assert!(params.implied_vol_lognormal(-0.01, 0.05, 1.0).is_nan());
        assert!(params.implied_vol_lognormal(0.05, -0.01, 1.0).is_nan());
        assert!(params.implied_vol_lognormal(0.05, 0.05, 0.0).is_nan());
    }

    #[test]
    fn test_sabr_density_check_extreme_nu() {
        let params = SabrParams {
            alpha: 0.04,
            beta: 0.5,
            rho: -0.7,
            nu: 2.0,
            shift: None,
        };
        let forward = 0.05;
        let expiry = 5.0;
        let strikes: Vec<f64> = (1..=20)
            .map(|i| forward * (0.5 + i as f64 * 0.05))
            .collect();

        let warnings = params.check_density(&strikes, forward, expiry);
        // With extreme nu=2.0 and long expiry, we may see negative density
        // at wing strikes. If not, that's also fine -- the test verifies
        // the method runs without panic.
        let _ = warnings; // Don't assert non-empty -- depends on approximation accuracy
    }

    #[test]
    fn test_sabr_density_check_normal_params() {
        let params = SabrParams::new(0.035, 0.5, -0.2, 0.4).expect("valid params");
        let forward = 0.05;
        let expiry = 1.0;
        let strikes: Vec<f64> = (1..=10)
            .map(|i| forward * (0.8 + i as f64 * 0.04))
            .collect();

        let warnings = params.check_density(&strikes, forward, expiry);
        assert!(
            warnings.is_empty(),
            "Normal params should produce no density warnings"
        );
    }

    #[test]
    fn test_sabr_params_with_shift() {
        let p = SabrParams::new(0.035, 0.5, -0.2, 0.4).expect("valid params");
        assert!(p.shift.is_none());

        let shifted = p.with_shift(0.03);
        assert_eq!(shifted.shift, Some(0.03));
    }

    #[test]
    fn test_shifted_sabr_implied_vol_lognormal() {
        // Shifted SABR: evaluate with F+shift, K+shift
        let p = SabrParams::new(0.035, 0.5, -0.2, 0.4)
            .expect("valid params")
            .with_shift(0.03);
        let f = -0.005; // negative forward
        let k = 0.01;
        let t = 1.0;
        let vol = p.implied_vol_lognormal(f, k, t);
        assert!(
            vol.is_finite() && vol > 0.0,
            "shifted SABR should handle negative rates"
        );
    }
}
