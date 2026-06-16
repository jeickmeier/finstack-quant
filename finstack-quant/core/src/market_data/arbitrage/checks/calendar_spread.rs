//! Calendar spread arbitrage check: total variance monotonicity in expiry.
//!
//! Verifies that total variance w(k, T) is non-decreasing in maturity at
//! each log-moneyness k. The check operates in log-moneyness space using
//! per-expiry forward prices, which is the correct no-arbitrage condition
//! when forwards drift across expiries.
//!
//! # Financial Background
//!
//! For European options, a longer-dated option at the same *log-moneyness*
//! must have at least as much total variance as a shorter-dated one. The
//! condition w(k, T₂) ≥ w(k, T₁) for T₂ > T₁ at each fixed k ensures
//! that undiscounted call prices are non-decreasing in maturity, preventing
//! riskless calendar spread profits.
//!
//! Using fixed cash strikes instead of fixed log-moneyness is only valid
//! when forwards are identical across expiries. When forwards differ, the
//! same cash strike corresponds to different moneyness levels, and the
//! check must account for this.
//!
//! # References
//!
//! - Gatheral, J. (2006). *The Volatility Surface*. Wiley. Chapter 3.
//! - Roper, M. (2010). *Arbitrage Free Implied Volatility Surfaces*.

use super::{classify_severity, ArbitrageCheck};
use crate::market_data::arbitrage::types::{ArbitrageType, ArbitrageViolation, ViolationLocation};
use crate::market_data::surfaces::VolSurface;

/// Checks that total variance is non-decreasing in expiry at each
/// log-moneyness level.
///
/// Requires per-expiry forward prices. When forwards are flat, this reduces
/// to the fixed-strike monotonicity check. When forwards differ across
/// expiries, the check correctly re-maps strikes into a common
/// log-moneyness grid.
pub struct CalendarSpreadCheck {
    /// Per-expiry forward prices (must match the surface expiry count).
    pub forwards: Vec<f64>,
    /// Tolerance below which a decrease is classified as Negligible.
    pub tolerance: f64,
}

impl Default for CalendarSpreadCheck {
    fn default() -> Self {
        Self {
            forwards: Vec::new(),
            tolerance: 1e-10,
        }
    }
}

impl ArbitrageCheck for CalendarSpreadCheck {
    fn name(&self) -> &str {
        "Calendar Spread"
    }

    fn check(&self, surface: &VolSurface) -> Vec<ArbitrageViolation> {
        let expiries = surface.expiries();
        let strikes = surface.strikes();
        let mut violations = Vec::new();

        if expiries.len() < 2 || self.forwards.is_empty() {
            return violations;
        }

        for ei in 0..expiries.len() - 1 {
            let t1 = expiries[ei];
            let t2 = expiries[ei + 1];
            let f1 = if ei < self.forwards.len() {
                self.forwards[ei]
            } else {
                *self.forwards.last().unwrap_or(&1.0)
            };
            let f2 = if ei + 1 < self.forwards.len() {
                self.forwards[ei + 1]
            } else {
                *self.forwards.last().unwrap_or(&1.0)
            };

            if f1 <= 0.0 || f2 <= 0.0 {
                continue;
            }

            // Use the strike grid to build a reference log-moneyness grid
            // anchored to the earlier expiry's forward.
            for &big_k in strikes {
                let k = (big_k / f1).ln();

                // Total variance at (k, T1): directly from the surface
                let v1 = surface.value_clamped(t1, big_k);
                let w1 = v1 * v1 * t1;

                // Total variance at (k, T2): find the cash strike that
                // corresponds to the same log-moneyness under T2's forward
                let big_k2 = f2 * (k).exp();
                let v2 = surface.value_clamped(t2, big_k2);
                let w2 = v2 * v2 * t2;

                if w2 < w1 - self.tolerance {
                    let decrease = w1 - w2;
                    let severity = classify_severity(decrease, 1e-8, 1e-5, 1e-3);
                    violations.push(ArbitrageViolation {
                        violation_type: ArbitrageType::CalendarSpread,
                        location: ViolationLocation {
                            strike: big_k,
                            expiry: t2,
                            adjacent_expiry: Some(t1),
                        },
                        severity,
                        magnitude: decrease,
                        description: format!(
                            "Calendar spread arbitrage at k={k:.4} (K={big_k:.2}): \
                            w(T={t2:.4}) = {w2:.6} < w(T={t1:.4}) = {w1:.6}"
                        ),
                        suggested_fix: None,
                    });
                }
            }
        }

        violations
    }
}
