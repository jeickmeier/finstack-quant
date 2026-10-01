//! Merton jump-diffusion characteristic function.

use super::{CharacteristicFunction, Cumulants};
use num_complex::Complex64;

/// Merton (1976) jump-diffusion characteristic function.
///
/// GBM plus compound Poisson jumps with log-normal jump sizes.
///
/// phi(u, t) = exp(iu*(r-q-lambda*m_bar)*t - 0.5*sigma^2*u^2*t
///              + lambda*t*(exp(iu*mu_j - 0.5*sigma_j^2*u^2) - 1))
///
/// where m_bar = exp(mu_j + 0.5*sigma_j^2) - 1 is the mean relative
/// jump size ensuring the drift correction is martingale-consistent.
///
/// # References
///
/// - Merton, R. C. (1976). "Option Pricing When Underlying Stock Returns
///   Are Discontinuous." *J. Financial Economics*, 3, 125-144. `docs/REFERENCES.md#merton-1976-jump`
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MertonJumpCf {
    /// Risk-free rate.
    pub r: f64,
    /// Dividend yield.
    pub q: f64,
    /// Diffusion volatility.
    pub sigma: f64,
    /// Jump intensity (expected number of jumps per year).
    pub lambda: f64,
    /// Mean of log-jump size.
    pub mu_j: f64,
    /// Standard deviation of log-jump size.
    pub sigma_j: f64,
}

impl MertonJumpCf {
    /// Validate the risk-neutral jump-diffusion parameters before pricing.
    ///
    /// Rates and the log-jump mean must be finite. Diffusion volatility,
    /// jump intensity (arrivals per year), and log-jump standard deviation
    /// must be finite and non-negative. The jump compensator must be finite.
    ///
    /// # Errors
    ///
    /// Returns a validation message for invalid parameters or an overflowing
    /// risk-neutral jump compensator.
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("rate", self.r),
            ("div_yield", self.q),
            ("mu_jump", self.mu_j),
        ] {
            if !value.is_finite() {
                return Err(format!("Merton {name} must be finite, got {value}"));
            }
        }
        for (name, value) in [
            ("sigma", self.sigma),
            ("lambda", self.lambda),
            ("sigma_jump", self.sigma_j),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "Merton {name} must be finite and non-negative, got {value}"
                ));
            }
        }
        let compensator = self.lambda * (self.mu_j + 0.5 * self.sigma_j * self.sigma_j).exp_m1();
        if !compensator.is_finite() {
            return Err("Merton risk-neutral jump compensator must be finite".to_string());
        }
        Ok(())
    }
}

impl CharacteristicFunction for MertonJumpCf {
    fn cf(&self, u: Complex64, t: f64) -> Complex64 {
        let iu = Complex64::i() * u;
        let s2 = self.sigma * self.sigma;
        let m_bar = (self.mu_j + 0.5 * self.sigma_j * self.sigma_j).exp() - 1.0;
        // Drift includes Ito correction -0.5*sigma^2 for the log-price process
        let drift = iu * (self.r - self.q - 0.5 * s2 - self.lambda * m_bar) * t;
        let diffusion = -0.5 * s2 * u * u * t;
        let jump_cf = (iu * self.mu_j - 0.5 * self.sigma_j * self.sigma_j * u * u).exp();
        let jumps = self.lambda * t * (jump_cf - Complex64::new(1.0, 0.0));
        (drift + diffusion + jumps).exp()
    }

    fn cumulants(&self, t: f64) -> Cumulants {
        let s2 = self.sigma * self.sigma;
        let sj2 = self.sigma_j * self.sigma_j;
        let m_bar = (self.mu_j + 0.5 * sj2).exp() - 1.0;
        Cumulants {
            c1: (self.r - self.q - 0.5 * s2 - self.lambda * m_bar + self.lambda * self.mu_j) * t,
            c2: (s2 + self.lambda * (sj2 + self.mu_j * self.mu_j)) * t,
            c3: self.lambda * (self.mu_j.powi(3) + 3.0 * self.mu_j * sj2) * t,
            c4: self.lambda
                * (self.mu_j.powi(4) + 6.0 * self.mu_j * self.mu_j * sj2 + 3.0 * sj2 * sj2)
                * t,
        }
    }
}
