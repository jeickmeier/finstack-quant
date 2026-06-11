//! SVI (Stochastic Volatility Inspired) parameterization for implied variance.
//!
//! The raw SVI parameterization (Gatheral 2004) provides a parsimonious,
//! arbitrage-controllable model of the implied volatility smile. It is widely
//! used for equity and FX volatility surface construction, especially for wing
//! extrapolation beyond observed market strikes.
//!
//! # Mathematical Foundation
//!
//! The raw SVI parameterization expresses total implied variance as:
//!
//! ```text
//! w(k) = a + b × (ρ(k - m) + √((k - m)² + σ²))
//! ```
//!
//! where:
//!   - `w = σ²T` — total implied variance
//!   - `k = ln(K/F)` — log-moneyness
//!   - `a` — overall variance level
//!   - `b` — slope of the wings (b ≥ 0)
//!   - `ρ` — rotation/asymmetry, in (-1, 1)
//!   - `m` — translation (shift of minimum variance)
//!   - `σ` — smoothing (minimum curvature at vertex), must be > 0
//!
//! # No-Arbitrage Conditions
//!
//! The SVI slice is free of butterfly arbitrage when:
//! - `b ≥ 0`
//! - `|ρ| < 1`
//! - `σ > 0`
//! - `a + b × σ × √(1 - ρ²) ≥ 0` (non-negative variance at minimum)
//!
//! # Reference
//!
//! - Gatheral, J. (2004). "A parsimonious arbitrage-free implied volatility
//!   parameterization with application to the valuation of volatility derivatives."
//!   *Presentation at Global Derivatives & Risk Management*, Madrid.
//! - Gatheral, J., & Jacquier, A. (2014). "Arbitrage-free SVI volatility surfaces."
//!   *Quantitative Finance*, 14(1), 59-71.

/// SVI (Stochastic Volatility Inspired) raw parameterization.
///
/// Represents one slice of the volatility surface at a fixed expiry
/// using five parameters that control the shape of the smile.
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::volatility::svi::SviParams;
///
/// let params = SviParams {
///     a: 0.04, b: 0.4, rho: -0.4, m: 0.0, sigma: 0.1,
/// };
/// params.validate().expect("valid SVI params");
///
/// let w = params.total_variance(0.0); // ATM total variance
/// assert!(w > 0.0);
///
/// let vol = params.implied_vol(0.0, 1.0); // ATM implied vol at T=1
/// assert!(vol > 0.0);
/// ```
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawSviParams")]
pub struct SviParams {
    /// Overall variance level.
    pub a: f64,
    /// Slope of the wings (must be ≥ 0).
    pub b: f64,
    /// Rotation / asymmetry parameter, in (-1, 1).
    pub rho: f64,
    /// Translation (shift of minimum variance point).
    pub m: f64,
    /// Smoothing parameter (minimum curvature at vertex), must be > 0.
    pub sigma: f64,
}

/// Raw deserialization state of [`SviParams`].
///
/// Mirrors the serialized field layout exactly so the wire format is
/// unchanged; conversion runs [`SviParams::validate`] (no-arbitrage and range
/// checks) and rejects unknown fields.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSviParams {
    /// Overall variance level.
    a: f64,
    /// Slope of the wings.
    b: f64,
    /// Rotation / asymmetry parameter.
    rho: f64,
    /// Translation.
    m: f64,
    /// Smoothing parameter.
    sigma: f64,
}

impl TryFrom<RawSviParams> for SviParams {
    type Error = crate::Error;

    fn try_from(raw: RawSviParams) -> crate::Result<Self> {
        let params = Self {
            a: raw.a,
            b: raw.b,
            rho: raw.rho,
            m: raw.m,
            sigma: raw.sigma,
        };
        params.validate()?;
        Ok(params)
    }
}

impl SviParams {
    /// Compute the total implied variance `w(k) = σ²T` at log-moneyness `k`.
    ///
    /// # Arguments
    ///
    /// * `k` — log-moneyness, `ln(K/F)`
    ///
    /// # Formula
    ///
    /// ```text
    /// w(k) = a + b × (ρ(k - m) + √((k - m)² + σ²))
    /// ```
    #[inline]
    pub fn total_variance(&self, k: f64) -> f64 {
        let km = k - self.m;
        self.a + self.b * (self.rho * km + (km * km + self.sigma * self.sigma).sqrt())
    }

    /// Compute the Black-Scholes implied volatility from SVI total variance.
    ///
    /// # Arguments
    ///
    /// * `k` — log-moneyness, `ln(K/F)`
    /// * `t` — time to expiry in years (must be > 0)
    ///
    /// # Returns
    ///
    /// Implied volatility `σ = √(w(k) / T)`. Returns `NaN` if `t ≤ 0` or
    /// if total variance is negative (arbitrage violation).
    #[inline]
    pub fn implied_vol(&self, k: f64, t: f64) -> f64 {
        if t <= 0.0 {
            return f64::NAN;
        }
        let w = self.total_variance(k);
        if w < 0.0 {
            return f64::NAN;
        }
        (w / t).sqrt()
    }

    /// Fallible version of [`implied_vol`](Self::implied_vol) with descriptive errors.
    ///
    /// Prefer this when diagnostics are needed; use `implied_vol` on hot paths
    /// where NaN propagation is acceptable.
    pub fn try_implied_vol(&self, k: f64, t: f64) -> crate::Result<f64> {
        if t <= 0.0 {
            return Err(crate::Error::Validation(
                "SVI implied vol: time-to-expiry must be positive".into(),
            ));
        }
        let w = self.total_variance(k);
        if w < 0.0 {
            return Err(crate::Error::Validation(format!(
                "SVI negative total variance w={w:.6} at k={k:.4}"
            )));
        }
        Ok((w / t).sqrt())
    }

    /// Validate SVI parameters against necessary no-arbitrage constraints.
    ///
    /// # Butterfly Arbitrage
    ///
    /// A butterfly arbitrage exists when the implied density (second derivative
    /// of call prices with respect to strike) becomes negative, allowing a
    /// riskless profit via a butterfly spread. The constraints below are
    /// **necessary but not sufficient** to prevent this: they rule out the
    /// most common violations but do not guarantee non-negative density
    /// everywhere along the smile.
    ///
    /// Full absence of butterfly arbitrage requires verifying that the
    /// local variance density `g(k) >= 0` for all log-moneyness `k`, per
    /// Gatheral & Jacquier (2014) Theorem 4.1. That check is computationally
    /// expensive and is **not** performed here. This is standard industry
    /// practice — most production SVI implementations enforce only these
    /// necessary conditions.
    ///
    /// # Conditions Checked
    ///
    /// 1. `b ≥ 0` — non-negative wing slope
    /// 2. `σ > 0` — positive smoothing (controls curvature at the vertex)
    /// 3. `|ρ| < 1` — correlation in valid range
    /// 4. `a + b × σ × √(1 - ρ²) ≥ 0` — non-negative minimum variance,
    ///    ensuring `w(k) ≥ 0` at the vertex where total variance is minimized
    /// 5. `b(1 + |ρ|) ≤ 2` — Roger Lee moment bound preventing butterfly
    ///    arbitrage at extreme strikes by capping the asymptotic variance slope
    /// 6. All parameters are finite
    ///
    /// # References
    ///
    /// - Gatheral, J., & Jacquier, A. (2014). "Arbitrage-free SVI volatility
    ///   surfaces." *Quantitative Finance*, 14(1), 59-71. Theorem 4.1.
    /// - Lee, R. (2004). "The Moment Formula for Implied Volatility at Extreme
    ///   Strikes." *Mathematical Finance*, 14(3), 469-480.
    ///
    /// # Errors
    ///
    /// Returns a validation error describing which constraint failed.
    pub fn validate(&self) -> crate::Result<()> {
        if !self.a.is_finite()
            || !self.b.is_finite()
            || !self.rho.is_finite()
            || !self.m.is_finite()
            || !self.sigma.is_finite()
        {
            return Err(crate::Error::Validation(
                "SVI parameters must be finite".to_string(),
            ));
        }
        if self.b < 0.0 {
            return Err(crate::Error::Validation(format!(
                "SVI b must be >= 0, got {}",
                self.b
            )));
        }
        if self.sigma <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "SVI sigma must be > 0, got {}",
                self.sigma
            )));
        }
        if self.rho <= -1.0 || self.rho >= 1.0 {
            return Err(crate::Error::Validation(format!(
                "SVI rho must be in (-1, 1), got {}",
                self.rho
            )));
        }
        // No-arbitrage: minimum variance must be non-negative
        let min_var = self.a + self.b * self.sigma * (1.0 - self.rho * self.rho).sqrt();
        if min_var < -1e-14 {
            return Err(crate::Error::Validation(format!(
                "SVI no-arbitrage violated: a + b*sigma*sqrt(1-rho^2) = {min_var:.6e} < 0"
            )));
        }
        // Roger Lee moment bounds: the total variance slope in either wing is
        // b(1 ± ρ), and Lee (2004) shows the maximum slope must not exceed 2
        // to prevent butterfly arbitrage at extreme strikes.
        // Reference: Lee, R. (2004). "The Moment Formula for Implied Volatility
        // at Extreme Strikes." Mathematical Finance, 14(3), 469-480.
        let lee_bound = self.b * (1.0 + self.rho.abs());
        if lee_bound > 2.0 + 1e-12 {
            return Err(crate::Error::Validation(format!(
                "SVI Roger Lee moment bound violated: b*(1+|rho|) = {lee_bound:.6} > 2"
            )));
        }
        Ok(())
    }
}

/// Calibrate SVI parameters to market-implied volatilities at a single expiry.
///
/// Uses Levenberg-Marquardt least squares minimization to fit the five SVI
/// parameters to observed (strike, vol) pairs.
///
/// # Arguments
///
/// * `strikes` — observed option strikes
/// * `vols` — observed Black-Scholes implied volatilities
/// * `forward` — forward price for this expiry
/// * `expiry` — time to expiry in years
///
/// # Returns
///
/// Calibrated [`SviParams`] that minimise the weighted sum of squared vol errors.
///
/// # Errors
///
/// Returns an error if:
/// - Input arrays have different lengths
/// - Fewer than 5 data points (5 free parameters)
/// - Calibration fails to converge or produces poor fit
///
/// # Example
///
/// ```rust
/// use finstack_core::math::volatility::svi::{calibrate_svi, SviParams};
///
/// let forward = 100.0;
/// let expiry = 1.0;
/// let strikes = &[80.0, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0];
/// let vols = &[0.30, 0.25, 0.22, 0.20, 0.21, 0.23, 0.28];
///
/// let params = calibrate_svi(strikes, vols, forward, expiry)
///     .expect("calibration should succeed");
/// params.validate().expect("calibrated params should be valid");
///
/// // ATM vol should be close to input
/// let atm_vol = params.implied_vol(0.0, expiry);
/// assert!((atm_vol - 0.20).abs() < 0.02);
/// ```
///
/// # Reference
///
/// Gatheral, J. (2004). "A parsimonious arbitrage-free implied volatility
/// parameterization with application to the valuation of volatility derivatives."
pub fn calibrate_svi(
    strikes: &[f64],
    vols: &[f64],
    forward: f64,
    expiry: f64,
) -> crate::Result<SviParams> {
    const MAX_VOL_RMSE: f64 = 0.005;

    if strikes.len() != vols.len() {
        return Err(crate::Error::Validation(
            "strikes and vols must have the same length".to_string(),
        ));
    }
    if strikes.len() < 5 {
        return Err(crate::Error::Validation(
            "Need at least 5 strike/vol pairs for SVI calibration (5 free parameters)".to_string(),
        ));
    }
    if !forward.is_finite() || forward <= 0.0 || !expiry.is_finite() || expiry <= 0.0 {
        return Err(crate::Error::Validation(
            format!(
                "forward and expiry must be finite and positive; got forward={forward}, expiry={expiry}"
            ),
        ));
    }
    for (idx, &strike) in strikes.iter().enumerate() {
        if !strike.is_finite() || strike <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "SVI strike at index {idx} must be finite and positive; got {strike}"
            )));
        }
    }
    for (idx, &vol) in vols.iter().enumerate() {
        if !vol.is_finite() || vol <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "SVI vol at index {idx} must be finite and positive; got {vol}"
            )));
        }
    }

    // Convert to log-moneyness and total variance
    let ks: Vec<f64> = strikes.iter().map(|&k| (k / forward).ln()).collect();
    let ws: Vec<f64> = vols.iter().map(|&v| v * v * expiry).collect();

    // Initial guesses from data:
    // a ≈ ATM total variance
    // b ≈ slope from wing variance difference
    // rho ≈ 0 (no asymmetry initially)
    // m ≈ 0 (centered)
    // sigma ≈ 0.1
    let atm_idx = ks
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.abs()
                .partial_cmp(&b.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    let a_init = ws[atm_idx];
    let b_init = 0.1_f64;
    let rho_init = 0.0_f64;
    let m_init = 0.0_f64;
    let sigma_init = 0.1_f64;

    let n_points = ks.len();

    // Unconstrained parametrisation:
    //   x[0] = a                          (unconstrained)
    //   x[1] = ln(b + epsilon)            → b = exp(x[1]) > 0
    //   x[2] = atanh(rho)                 → rho = tanh(x[2]) ∈ (-1, 1)
    //   x[3] = m                          (unconstrained)
    //   x[4] = ln(sigma)                  → sigma = exp(x[4]) > 0
    let residuals = |x: &[f64], resid: &mut [f64]| {
        let a = x[0];
        let b = x[1].exp();
        let rho = x[2].tanh();
        let m = x[3];
        let sigma = x[4].exp();

        let params = SviParams {
            a,
            b,
            rho,
            m,
            sigma,
        };

        for (i, (&k, &w_mkt)) in ks.iter().zip(ws.iter()).enumerate() {
            let w_model = params.total_variance(k);
            resid[i] = w_model - w_mkt;
        }
    };

    let x0 = [
        a_init,
        (b_init.max(1e-6)).ln(),
        rho_init.clamp(-0.999, 0.999).atanh(),
        m_init,
        sigma_init.ln(),
    ];

    let solver = crate::math::solver_multi::LevenbergMarquardtSolver::new()
        .with_tolerance(1e-12)
        .with_max_iterations(300);

    let result = solver.solve_system_with_dim_stats(residuals, &x0, n_points);

    let sol =
        result.map_err(|e| crate::Error::Validation(format!("SVI calibration failed: {e}")))?;

    let a = sol.params[0];
    let b = sol.params[1].exp();
    let rho = sol.params[2].tanh();
    let m = sol.params[3];
    let sigma = sol.params[4].exp();

    let params = SviParams {
        a,
        b,
        rho,
        m,
        sigma,
    };

    // Validate no-arbitrage and compute RMSE
    params.validate()?;

    let sse: f64 = ks
        .iter()
        .zip(ws.iter())
        .map(|(&k, &w_mkt)| {
            let w_model = params.total_variance(k);
            (w_model - w_mkt) * (w_model - w_mkt)
        })
        .sum();
    let rmse_w = (sse / n_points as f64).sqrt();

    // Convert variance RMSE to approximate vol RMSE for quality check
    let avg_w: f64 = ws.iter().sum::<f64>() / ws.len() as f64;
    let rmse_vol_approx = if avg_w > 1e-14 {
        rmse_w / (2.0 * avg_w.sqrt())
    } else {
        rmse_w
    };

    if rmse_vol_approx > MAX_VOL_RMSE {
        return Err(crate::Error::Validation(format!(
            "SVI calibration RMSE too high: {rmse_vol_approx:.4} (>{:.2}%)",
            MAX_VOL_RMSE * 100.0
        )));
    }

    Ok(params)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svi_total_variance_at_minimum() {
        let params = SviParams {
            a: 0.04,
            b: 0.4,
            rho: -0.4,
            m: 0.0,
            sigma: 0.1,
        };
        params.validate().expect("params should be valid");

        // At k = m, w(m) = a + b * sigma
        let w_at_m = params.total_variance(0.0);
        let expected = 0.04 + 0.4 * 0.1;
        assert!(
            (w_at_m - expected).abs() < 1e-12,
            "w(0) = {w_at_m}, expected {expected}"
        );
    }

    #[test]
    fn svi_implied_vol_positive() {
        let params = SviParams {
            a: 0.04,
            b: 0.4,
            rho: -0.3,
            m: 0.0,
            sigma: 0.1,
        };
        params.validate().expect("params should be valid");

        for k in [-0.5, -0.2, 0.0, 0.2, 0.5] {
            let vol = params.implied_vol(k, 1.0);
            assert!(
                vol > 0.0 && vol.is_finite(),
                "vol at k={k} should be positive and finite: {vol}"
            );
        }
    }

    #[test]
    fn svi_wing_behavior() {
        // With negative rho, left wing should be steeper
        let params = SviParams {
            a: 0.04,
            b: 0.4,
            rho: -0.5,
            m: 0.0,
            sigma: 0.1,
        };
        params.validate().expect("params should be valid");

        let w_left = params.total_variance(-0.5);
        let w_right = params.total_variance(0.5);

        // Negative rho means left wing (negative k) has higher variance
        assert!(
            w_left > w_right,
            "Left wing should have higher variance with rho < 0: w(-0.5)={w_left}, w(0.5)={w_right}"
        );
    }

    #[test]
    fn svi_validate_rejects_invalid() {
        // Negative b
        let bad_b = SviParams {
            a: 0.04,
            b: -0.1,
            rho: 0.0,
            m: 0.0,
            sigma: 0.1,
        };
        assert!(bad_b.validate().is_err());

        // sigma = 0
        let bad_sigma = SviParams {
            a: 0.04,
            b: 0.4,
            rho: 0.0,
            m: 0.0,
            sigma: 0.0,
        };
        assert!(bad_sigma.validate().is_err());

        // rho = 1
        let bad_rho = SviParams {
            a: 0.04,
            b: 0.4,
            rho: 1.0,
            m: 0.0,
            sigma: 0.1,
        };
        assert!(bad_rho.validate().is_err());

        // No-arbitrage violation: a too negative
        let bad_arb = SviParams {
            a: -0.5,
            b: 0.1,
            rho: 0.0,
            m: 0.0,
            sigma: 0.1,
        };
        assert!(bad_arb.validate().is_err());

        // Roger Lee moment bound violation: b*(1+|rho|) > 2
        let bad_lee = SviParams {
            a: 0.04,
            b: 1.5,
            rho: 0.5,
            m: 0.0,
            sigma: 0.1,
        };
        let err = bad_lee.validate().expect_err("Lee bound should fail");
        assert!(
            err.to_string().contains("Roger Lee"),
            "Expected Lee bound error, got: {err}"
        );
    }

    #[test]
    fn svi_implied_vol_nan_for_bad_inputs() {
        let params = SviParams {
            a: 0.04,
            b: 0.4,
            rho: 0.0,
            m: 0.0,
            sigma: 0.1,
        };
        assert!(params.implied_vol(0.0, 0.0).is_nan());
        assert!(params.implied_vol(0.0, -1.0).is_nan());
    }

    #[test]
    fn calibrate_svi_round_trip() {
        // Generate synthetic market data from known SVI params
        let true_params = SviParams {
            a: 0.04,
            b: 0.3,
            rho: -0.3,
            m: 0.02,
            sigma: 0.15,
        };
        true_params.validate().expect("true params should be valid");

        let forward = 100.0;
        let expiry = 1.0;
        let strikes: Vec<f64> = vec![
            70.0, 80.0, 85.0, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0, 130.0,
        ];

        let vols: Vec<f64> = strikes
            .iter()
            .map(|&k| {
                let log_k = (k / forward).ln();
                true_params.implied_vol(log_k, expiry)
            })
            .collect();

        let calibrated =
            calibrate_svi(&strikes, &vols, forward, expiry).expect("calibration should succeed");

        // Check vol fit is close at each strike
        for (&k, &mkt_vol) in strikes.iter().zip(vols.iter()) {
            let log_k = (k / forward).ln();
            let cal_vol = calibrated.implied_vol(log_k, expiry);
            assert!(
                (cal_vol - mkt_vol).abs() < 0.005,
                "Vol mismatch at K={k}: calibrated={cal_vol:.4}, market={mkt_vol:.4}"
            );
        }
    }

    #[test]
    fn calibrate_svi_rejects_insufficient_data() {
        let strikes = &[90.0, 100.0, 110.0, 120.0]; // only 4 points for 5 params
        let vols = &[0.25, 0.20, 0.21, 0.23];
        let result = calibrate_svi(strikes, vols, 100.0, 1.0);
        assert!(result.is_err());
    }

    #[test]
    fn calibrate_svi_rejects_mismatched_lengths() {
        let strikes = &[90.0, 100.0, 110.0, 120.0, 130.0];
        let vols = &[0.25, 0.20, 0.21];
        let result = calibrate_svi(strikes, vols, 100.0, 1.0);
        assert!(result.is_err());
    }

    #[test]
    fn calibrate_svi_rejects_noisy_non_svi_slice() {
        let strikes = &[80.0, 90.0, 100.0, 110.0, 120.0, 130.0, 140.0];
        let vols = &[0.20, 0.26, 0.19, 0.27, 0.18, 0.28, 0.17];

        let result = calibrate_svi(strikes, vols, 100.0, 1.0);
        assert!(
            result.is_err(),
            "alternating smile should be rejected as a poor SVI fit"
        );
    }

    #[test]
    fn calibrate_svi_rejects_moderate_fit_error() {
        let true_params = SviParams {
            a: 0.04,
            b: 0.3,
            rho: -0.3,
            m: 0.02,
            sigma: 0.15,
        };
        let strikes = &[80.0, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0];
        let mut vols: Vec<f64> = strikes
            .iter()
            .map(|&k| true_params.implied_vol((k / 100.0_f64).ln(), 1.0))
            .collect();
        vols[1] += 0.03;
        vols[5] -= 0.03;

        let result = calibrate_svi(strikes, &vols, 100.0, 1.0);
        assert!(
            result.is_err(),
            "10 vol-point perturbations should exceed production fit tolerance"
        );
    }

    #[test]
    fn calibrate_svi_rejects_non_positive_strike() {
        let strikes = &[0.0, 90.0, 100.0, 110.0, 120.0];
        let vols = &[0.25, 0.22, 0.20, 0.21, 0.23];

        let err = calibrate_svi(strikes, vols, 100.0, 1.0)
            .expect_err("non-positive strikes should be rejected");
        assert!(
            err.to_string().to_lowercase().contains("strike"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn calibrate_svi_rejects_non_finite_vol() {
        let strikes = &[80.0, 90.0, 100.0, 110.0, 120.0];
        let vols = &[0.30, 0.24, f64::NAN, 0.22, 0.27];

        let err =
            calibrate_svi(strikes, vols, 100.0, 1.0).expect_err("non-finite vols should fail");
        assert!(
            err.to_string().to_lowercase().contains("vol"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn svi_params_serde_validates_on_deserialize() {
        // Valid JSON round-trips.
        let p = SviParams {
            a: 0.04,
            b: 0.4,
            rho: -0.4,
            m: 0.0,
            sigma: 0.1,
        };
        let json = serde_json::to_string(&p).expect("serialize");
        let back: SviParams = serde_json::from_str(&json).expect("round-trip");
        assert_eq!(p.a, back.a);
        assert_eq!(p.b, back.b);
        assert_eq!(p.rho, back.rho);
        assert_eq!(p.m, back.m);
        assert_eq!(p.sigma, back.sigma);

        // Out-of-range rho rejected.
        let bad = r#"{"a":0.04,"b":0.4,"rho":1.5,"m":0.0,"sigma":0.1}"#;
        assert!(serde_json::from_str::<SviParams>(bad).is_err());

        // No-arbitrage violation rejected.
        let bad_arb = r#"{"a":-0.5,"b":0.1,"rho":0.0,"m":0.0,"sigma":0.1}"#;
        assert!(serde_json::from_str::<SviParams>(bad_arb).is_err());

        // Unknown field rejected.
        let unknown = r#"{"a":0.04,"b":0.4,"rho":-0.4,"m":0.0,"sigma":0.1,"extra":1.0}"#;
        assert!(serde_json::from_str::<SviParams>(unknown).is_err());
    }

    #[test]
    fn svi_symmetric_smile() {
        // With rho = 0, smile should be symmetric around m
        let params = SviParams {
            a: 0.04,
            b: 0.3,
            rho: 0.0,
            m: 0.0,
            sigma: 0.1,
        };
        params.validate().expect("params should be valid");

        let w_left = params.total_variance(-0.2);
        let w_right = params.total_variance(0.2);

        assert!(
            (w_left - w_right).abs() < 1e-12,
            "Symmetric smile expected: w(-0.2)={w_left}, w(0.2)={w_right}"
        );
    }
}
