//! Black-Scholes characteristic function.

use super::{CharacteristicFunction, Cumulants};
use num_complex::Complex64;

/// Black-Scholes characteristic function.
///
/// phi(u, t) = exp(iu * (r - q - 0.5 * sigma^2) * t - 0.5 * sigma^2 * u^2 * t)
///
/// The simplest characteristic function, corresponding to geometric Brownian
/// motion. Primarily used as a reference for testing Fourier pricers, since
/// closed-form Black-Scholes prices are available for comparison.
///
/// # References
///
/// - Black, F. & Scholes, M. (1973). "The Pricing of Options and Corporate
///   Liabilities." *Journal of Political Economy*, 81(3), 637-654.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BlackScholesCf {
    /// Risk-free rate.
    pub r: f64,
    /// Dividend yield.
    pub q: f64,
    /// Volatility (annualized).
    pub sigma: f64,
}

impl CharacteristicFunction for BlackScholesCf {
    fn cf(&self, u: Complex64, t: f64) -> Complex64 {
        let iu = Complex64::i() * u;
        let drift = (self.r - self.q - 0.5 * self.sigma * self.sigma) * t;
        let diffusion = -0.5 * self.sigma * self.sigma * t;
        (iu * drift + diffusion * u * u).exp()
    }

    fn cumulants(&self, t: f64) -> Cumulants {
        let s2 = self.sigma * self.sigma;
        Cumulants {
            c1: (self.r - self.q - 0.5 * s2) * t,
            c2: s2 * t,
            c3: 0.0,
            c4: 0.0,
        }
    }
}
