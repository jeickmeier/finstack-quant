use super::parameters::SABRParameters;
use finstack_core::{Error, Result};

/// SABR model for volatility smile dynamics
pub struct SABRModel {
    params: SABRParameters,
}

impl SABRModel {
    /// Create new SABR model
    pub fn new(params: SABRParameters) -> Self {
        Self { params }
    }

    /// Calculate implied volatility using Hagan's approximation
    ///
    /// This is the standard SABR formula from Hagan et al. (2002) with enhanced
    /// numerical stability and support for negative rates through shifting.
    #[must_use = "computed volatility should be used"]
    #[inline]
    pub fn implied_volatility(
        &self,
        forward: f64,
        strike: f64,
        time_to_expiry: f64,
    ) -> Result<f64> {
        // Apply shift if using shifted SABR for negative rates
        let (effective_forward, effective_strike) = if let Some(shift) = self.params.shift {
            (forward + shift, strike + shift)
        } else {
            // Validate non-negative rates for standard SABR
            if forward <= 0.0 || strike <= 0.0 {
                return Err(Error::Validation(format!(
                    "Standard SABR requires positive rates. Got forward={:.6}, strike={:.6}. \
                     Use shifted SABR (new_with_shift) for negative rates.",
                    forward, strike
                )));
            }
            (forward, strike)
        };

        let alpha = self.params.alpha;
        let mut beta = self.params.beta;
        let nu = self.params.nu;
        let rho = self.params.rho;

        // Clamp beta to avoid numerical instability near 0 or 1
        if beta < 1e-4 {
            beta = 0.0;
        } else if (1.0 - beta).abs() < 1e-4 {
            beta = 1.0;
        }

        let beta_is_zero = beta.abs() < 1e-12;
        let beta_is_one = (beta - 1.0).abs() < 1e-12;

        // Calculate intermediate values with numerical protection
        let f_mid = (effective_forward * effective_strike).sqrt();
        let f_mid_beta = if beta_is_zero {
            1.0 // Special case for normal model
        } else {
            f_mid.powf(1.0 - beta)
        };

        // Enhanced log-moneyness calculation
        let z = if nu.abs() < 1e-14 {
            // Handle nu ≈ 0 case (pure CEV)
            return self.atm_volatility(effective_forward, time_to_expiry);
        } else if beta_is_one {
            (nu / alpha) * (effective_forward / effective_strike).ln()
        } else if beta_is_zero {
            (nu / alpha) * (effective_forward - effective_strike)
        } else {
            (nu / alpha) * (effective_forward.powf(1.0 - beta) - effective_strike.powf(1.0 - beta))
                / (1.0 - beta)
        };

        // ATM detection using single scale-invariant relative threshold
        // This ensures consistent behavior regardless of forward/strike scale (rates vs equities)
        let relative_diff =
            (effective_forward - effective_strike).abs() / effective_forward.max(effective_strike);
        if relative_diff < 1e-8 || z.abs() < 1e-8 {
            return self.atm_volatility(effective_forward, time_to_expiry);
        }

        // Calculate log-moneyness for correction terms
        let log_moneyness = (effective_forward / effective_strike).ln();

        // Obloj (2008) correction: use geometric-mean-based z for the z/χ(z) ratio.
        // The original Hagan et al. (2002) formula uses z = (ν/α)(F^{1-β} - K^{1-β})/(1-β)
        // which introduces O(ε²) errors for long maturities and high vol-of-vol.
        // The corrected formula uses z = (ν/α) * f_mid^{1-β} * ln(F/K) for 0 < β < 1.
        // For β=0 (normal) and β=1 (lognormal), the original formula is already exact.
        //
        // Reference: Obloj, J. (2008). "Fine-tune your smile: Correction to Hagan et al."
        // arXiv:0708.0998v2
        let z_corrected = if beta_is_one || beta_is_zero {
            z
        } else {
            (nu / alpha) * f_mid.powf(1.0 - beta) * log_moneyness
        };

        // Calculate chi(z) with robust numerical handling
        let x = self.calculate_chi_robust(z_corrected)?;

        // First factor with enhanced numerical stability
        let factor1 = if f_mid_beta.abs() < 1e-14 {
            alpha // Handle degenerate case
        } else {
            let correction_term = if beta_is_zero {
                1.0 // No correction for normal model
            } else {
                1.0 + (1.0 - beta).powi(2) / 24.0 * log_moneyness.powi(2)
                    + (1.0 - beta).powi(4) / 1920.0 * log_moneyness.powi(4)
            };
            alpha / (f_mid_beta * correction_term)
        };

        // Second factor (z/x correction) with numerical protection
        let factor2 = if x.abs() < 1e-14 {
            1.0 // Avoid division by zero
        } else {
            z_corrected / x
        };

        // Third factor (time correction) with enhanced precision
        let time_correction = if beta_is_zero {
            // Normal SABR time correction
            (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2)
        } else {
            (1.0 - beta).powi(2) / 24.0 * alpha.powi(2) / f_mid.powf(2.0 * (1.0 - beta))
                + 0.25 * rho * beta * nu * alpha / f_mid_beta
                + (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2)
        };

        let factor3 = 1.0 + time_to_expiry * time_correction;

        let volatility = factor1 * factor2 * factor3;

        // Validate result
        if volatility <= 0.0 || !volatility.is_finite() {
            return Err(Error::Validation(format!(
                "SABR produced invalid volatility={:.6} for forward={:.6}, strike={:.6}, T={:.4}. \
                 Check parameter values.",
                volatility, forward, strike, time_to_expiry
            )));
        }

        Ok(volatility)
    }

    /// Calculate ATM implied volatility with enhanced numerical stability
    #[inline]
    pub(crate) fn atm_volatility(&self, forward: f64, time_to_expiry: f64) -> Result<f64> {
        let alpha = self.params.alpha;
        let beta = self.params.beta;
        let nu = self.params.nu;
        let rho = self.params.rho;
        let beta_is_zero = beta.abs() < 1e-12;

        // Handle degenerate cases
        if alpha.abs() < 1e-14 {
            return Ok(0.0);
        }

        // ATM volatility formula with numerical protection
        // Hagan et al. (2002) eq. 2.18:
        //   σ_ATM = α / F^(1-β) · [1 + ((1-β)²α²/24/F^(2(1-β)) + ρβνα/4/F^(1-β) + (2-3ρ²)ν²/24) · T]
        let vol = if beta_is_zero {
            // Normal SABR (β=0): F^(1-β) = F, but the α·T·(...) time correction
            // collapses to the normal-SABR form with no F dependence.
            // Hagan eq. 2.18 at β=0: σ_ATM = α · (1 + (2-3ρ²)ν²/24 · T)
            alpha * (1.0 + time_to_expiry * (2.0 - 3.0 * rho.powi(2)) / 24.0 * nu.powi(2))
        } else {
            // General β ∈ (0,1] case, including β=1.
            // At β=1: F^(1-β)=1, (1-β)²=0, so alpha_term→0 and rho_term→ρνα/4.
            // At β=0.5: covered by (1-β)²/24·α²/F^(2(1-β)) = α²/(24·F) which
            //           matches the original beta_is_half shortcut (redundant, removed).
            let f_beta = if forward.abs() < 1e-14 {
                1e-14_f64.powf(1.0 - beta) // Avoid zero to very small power
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

        // Validate result
        if vol <= 0.0 || !vol.is_finite() {
            return Err(Error::Validation(format!(
                "SABR ATM volatility calculation produced invalid result={:.6} for forward={:.6}, T={:.4}",
                vol, forward, time_to_expiry
            )));
        }

        Ok(vol)
    }

    /// Calculate chi(z) for the SABR formula with enhanced numerical stability.
    ///
    /// Uses a smooth blending function between series expansion (for small z)
    /// and the exact formula (for larger z) to ensure continuous derivatives
    /// for smooth Greeks near ATM.
    ///
    /// # Implementation Notes
    ///
    /// - Series expansion: χ(z) ≈ z + (ρ/2)z² + ((2ρ² - 1)/6)z³ + O(z⁴)
    /// - Smooth sigmoid blend in transition region [1e-5, 1e-3]
    /// - Special handling for extreme rho values (±1)
    #[inline]
    pub(crate) fn calculate_chi_robust(&self, z: f64) -> Result<f64> {
        let rho = self.params.rho;

        // Fourth-order Taylor series expansion around z = 0:
        // χ(z) = ln((√(1 - 2ρz + z²) + z - ρ)/(1 - ρ))
        //
        // Expand: Let f(z) = √(1 - 2ρz + z²) + z - ρ
        // f(0) = 1 - ρ
        // f'(0) = (-ρ + 0)/1 + 1 = 1 - ρ (since d/dz √(1-2ρz+z²)|_{z=0} = -ρ)
        // etc.
        //
        // After careful expansion: χ(z) ≈ z + (ρ/2)z² + ((2ρ² - 1)/6)z³ + O(z⁴)
        let series_chi = |z_val: f64| -> f64 {
            let z2 = z_val * z_val;
            let z3 = z2 * z_val;
            let z4 = z2 * z2;
            // Coefficients from Taylor expansion
            let c1 = 1.0; // coefficient of z
            let c2 = rho / 2.0; // coefficient of z²
            let c3 = (2.0 * rho * rho - 1.0) / 6.0; // coefficient of z³
            let c4 = rho * (5.0 * rho * rho - 3.0) / 24.0; // coefficient of z⁴
            c1 * z_val + c2 * z2 + c3 * z3 + c4 * z4
        };

        // Exact chi formula
        let exact_chi = |z_val: f64| -> Result<f64> {
            let discriminant = 1.0 - 2.0 * rho * z_val + z_val * z_val;

            if discriminant < 0.0 {
                return Err(Error::Validation(format!(
                    "SABR chi function: negative discriminant {} for z={:.6}, rho={:.6}",
                    discriminant, z_val, rho
                )));
            }

            let sqrt_disc = discriminant.sqrt();

            // Handle extreme rho cases
            if (1.0 - rho).abs() < 1e-10 {
                // rho ≈ 1: Use approximation χ(z) ≈ z / (1 + z/2)
                return Ok(z_val / (1.0 + z_val / 2.0));
            }
            if (1.0 + rho).abs() < 1e-10 {
                // rho ≈ -1: Use stable form
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

        // Transition region bounds
        let z_low = 1e-5; // Below this, use pure series
        let z_high = 1e-3; // Above this, use pure exact

        if abs_z < z_low {
            // Pure series expansion for very small z
            Ok(series_chi(z))
        } else if abs_z > z_high {
            // Pure exact formula for larger z
            exact_chi(z)
        } else {
            // Smooth sigmoid blend in transition region
            // Use smooth step function: t = (|z| - z_low) / (z_high - z_low)
            // blend = 3t² - 2t³ (smooth step with zero derivative at endpoints)
            let t = (abs_z - z_low) / (z_high - z_low);
            let blend = t * t * (3.0 - 2.0 * t); // Hermite smoothstep

            let series_val = series_chi(z);
            let exact_val = exact_chi(z)?;

            // Linear interpolation with smooth weights
            Ok((1.0 - blend) * series_val + blend * exact_val)
        }
    }

    /// Get model parameters
    pub fn parameters(&self) -> &SABRParameters {
        &self.params
    }

    /// Update model parameters
    pub fn set_parameters(&mut self, params: SABRParameters) {
        self.params = params;
    }

    /// Check if this model supports negative rates
    pub fn supports_negative_rates(&self) -> bool {
        self.params.shift.is_some()
    }

    /// Get the effective forward/strike after applying shift
    pub fn effective_rates(&self, forward: f64, strike: f64) -> (f64, f64) {
        if let Some(shift) = self.params.shift {
            (forward + shift, strike + shift)
        } else {
            (forward, strike)
        }
    }

    /// Validate inputs for SABR model
    pub fn validate_inputs(&self, forward: f64, strike: f64, time_to_expiry: f64) -> Result<()> {
        // Time validation
        if time_to_expiry <= 0.0 {
            return Err(Error::Validation(format!(
                "SABR time_to_expiry must be positive, got: {:.6}",
                time_to_expiry
            )));
        }

        // Rate validation based on model type
        if self.params.shift.is_none() {
            // Standard SABR requires positive rates
            if forward <= 0.0 || strike <= 0.0 {
                return Err(Error::Validation(format!(
                    "Standard SABR requires positive rates. Got forward={:.6}, strike={:.6}. \
                     Use shifted SABR for negative rates.",
                    forward, strike
                )));
            }
        } else if let Some(shift) = self.params.shift {
            // Shifted SABR allows negative rates but shifted values must be positive
            if forward + shift <= 0.0 || strike + shift <= 0.0 {
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
