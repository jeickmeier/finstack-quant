//! Butterfly arbitrage check via the Durrleman density condition.
//!
//! Verifies that the implied probability density is non-negative by
//! evaluating Durrleman's g(k) function in log-moneyness space.
//!
//! # Financial Background
//!
//! A butterfly spread at strike K consists of buying calls at K-d and K+d
//! while selling two calls at K. No-arbitrage requires this portfolio to have
//! non-negative value, which is equivalent to non-negative implied density.
//!
//! The correct condition (Gatheral, *The Volatility Surface*, §3.1) is:
//!
//! ```text
//! g(k) = (1 - k·w'/(2w))² - (w')²/4·(1/w + 1/4) + w''/2 ≥ 0
//! ```
//!
//! where w(k) = σ²·T is total implied variance as a function of
//! log-moneyness k = ln(K/F), and primes denote derivatives w.r.t. k.
//!
//! # References
//!
//! - Durrleman, V. (2003). *From Implied to Spot Volatilities*. PhD thesis.
//! - Gatheral, J. (2006). *The Volatility Surface*. Wiley.

use super::{classify_severity, ArbitrageCheck};
use crate::market_data::arbitrage::types::{ArbitrageType, ArbitrageViolation, ViolationLocation};
use crate::market_data::surfaces::VolSurface;

/// Checks that implied probability density is non-negative via Durrleman's
/// g(k) condition in log-moneyness space.
///
/// Requires per-expiry forward prices to convert cash strikes to
/// log-moneyness. When constructed via the orchestrator [`check_surface`],
/// the forward is taken from [`ArbitrageCheckConfig::forward`].
///
/// [`check_surface`]: crate::market_data::arbitrage::check_surface
/// [`ArbitrageCheckConfig::forward`]: crate::market_data::arbitrage::ArbitrageCheckConfig::forward
pub struct ButterflyCheck {
    /// Per-expiry forward prices (must match the surface expiry count).
    pub forwards: Vec<f64>,
    /// Tolerance below which a negative g(k) is classified as Negligible.
    pub tolerance: f64,
}

impl Default for ButterflyCheck {
    fn default() -> Self {
        Self {
            forwards: Vec::new(),
            tolerance: 1e-10,
        }
    }
}

impl ArbitrageCheck for ButterflyCheck {
    fn name(&self) -> &str {
        "Butterfly"
    }

    fn check(&self, surface: &VolSurface) -> Vec<ArbitrageViolation> {
        let expiries = surface.expiries();
        let strikes = surface.strikes();
        let mut violations = Vec::new();

        if strikes.len() < 3 || self.forwards.is_empty() {
            return violations;
        }

        for (ei, &t) in expiries.iter().enumerate() {
            let fwd = if ei < self.forwards.len() {
                self.forwards[ei]
            } else {
                *self.forwards.last().unwrap_or(&1.0)
            };

            if fwd <= 0.0 {
                continue;
            }

            let log_moneyness: Vec<f64> = strikes.iter().map(|&k| (k / fwd).ln()).collect();

            let ws: Vec<f64> = strikes
                .iter()
                .map(|&k| {
                    let v = surface.value_clamped(t, k);
                    v * v * t
                })
                .collect();

            for i in 1..strikes.len() - 1 {
                let dk_minus = log_moneyness[i] - log_moneyness[i - 1];
                let dk_plus = log_moneyness[i + 1] - log_moneyness[i];

                if dk_minus.abs() < 1e-14 || dk_plus.abs() < 1e-14 {
                    continue;
                }

                let w = ws[i];
                if w < 1e-14 {
                    continue;
                }

                let w_prime = (ws[i + 1] - ws[i - 1]) / (dk_plus + dk_minus);

                let dk_avg = 0.5 * (dk_minus + dk_plus);
                let w_double_prime = (ws[i + 1] / dk_plus
                    - ws[i] * (1.0 / dk_plus + 1.0 / dk_minus)
                    + ws[i - 1] / dk_minus)
                    / dk_avg;

                let k = log_moneyness[i];
                let term1 = 1.0 - k * w_prime / (2.0 * w);
                let g = term1 * term1 - w_prime * w_prime / 4.0 * (1.0 / w + 0.25)
                    + w_double_prime / 2.0;

                if g < -self.tolerance {
                    let magnitude = -g;
                    let severity = classify_severity(magnitude, 1e-8, 1e-5, 1e-3);
                    violations.push(ArbitrageViolation {
                        violation_type: ArbitrageType::Butterfly,
                        location: ViolationLocation {
                            strike: strikes[i],
                            expiry: t,
                            adjacent_expiry: None,
                        },
                        severity,
                        magnitude,
                        description: format!(
                            "Butterfly arbitrage at T={t:.4}, K={:.2} (k={k:.4}): g(k) = {g:.2e}",
                            strikes[i]
                        ),
                        suggested_fix: None,
                    });
                }
            }
        }

        violations
    }
}
