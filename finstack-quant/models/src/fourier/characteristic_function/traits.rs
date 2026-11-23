//! Core [`CharacteristicFunction`] trait and [`Cumulants`] container.

use num_complex::Complex64;

/// First four cumulants of a log-price distribution.
///
/// Used by the COS method for automatic truncation range selection
/// (Fang-Oosterlee 2008). The cumulants determine the integration
/// domain [a, b] that captures sufficient probability mass.
#[derive(Debug, Clone, Copy)]
pub struct Cumulants {
    /// First cumulant (mean of log-price).
    pub c1: f64,
    /// Second cumulant (variance of log-price).
    pub c2: f64,
    /// Third cumulant (related to skewness).
    pub c3: f64,
    /// Fourth cumulant (related to excess kurtosis).
    pub c4: f64,
}

/// Characteristic function of a log-price process.
///
/// Represents phi(u, t) = E[exp(i * u * X_t)] where X_t = ln(S_T / S_0)
/// under the risk-neutral measure. All Fourier pricing methods consume
/// this trait.
///
/// # Implementors
///
/// Implement this trait for any model whose characteristic function is
/// known in closed form or can be computed numerically (e.g., via ODE).
///
/// # Mathematical Properties
///
/// Valid characteristic functions must satisfy:
/// - phi(0, t) = 1 for all t (normalization)
/// - |phi(u, t)| <= 1 for real u (boundedness)
///
/// # References
///
/// - Cont, R. & Tankov, P. (2004). *Financial Modelling with Jump Processes*.
///   Chapman & Hall/CRC. Chapter 11. `docs/REFERENCES.md#cont-tankov-2004`
pub trait CharacteristicFunction: Send + Sync {
    /// Evaluate the characteristic function at complex frequency `u` and time `t`.
    ///
    /// # Arguments
    ///
    /// * `u` - Complex Fourier frequency
    /// * `t` - Time to maturity (years)
    ///
    /// # Returns
    ///
    /// phi(u, t) as a complex number.
    fn cf(&self, u: Complex64, t: f64) -> Complex64;

    /// First four cumulants of the log-price distribution at time t.
    ///
    /// Used by the COS method for automatic truncation range selection.
    /// Implementations should provide closed-form cumulants when available
    /// for best accuracy.
    fn cumulants(&self, t: f64) -> Cumulants;
}
