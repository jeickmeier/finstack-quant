//! Local volatility density check via the Dupire formula.
//!
//! Verifies that the Dupire local variance is positive everywhere on the grid.
//! This is the definitive no-arbitrage condition for a volatility surface: if
//! the local variance is negative at any point, there exists an arbitrage.
//!
//! # Mathematical Background
//!
//! The Dupire local variance is:
//!
//! ```text
//! sigma^2_local(K, T) = (dw/dT) / (1 - k/w * dw/dk
//!     + 1/4 * (-1/4 - 1/w + k^2/w^2) * (dw/dk)^2
//!     + 1/2 * d2w/dk2)
//! ```
//!
//! where w = sigma^2 * T is total implied variance and k = ln(K/F).
//!
//! Both numerator (dw/dT >= 0) and denominator (density condition) must be
//! non-negative.

use super::{classify_severity, ArbitrageCheck};
use crate::market_data::arbitrage::types::{ArbitrageType, ArbitrageViolation, ViolationLocation};
use crate::market_data::surfaces::VolSurface;

/// Checks that the Dupire local variance is positive everywhere on the grid.
pub struct LocalVolDensityCheck {
    /// Forward price for log-moneyness calculation.
    pub forward: f64,
    /// Tolerance for density positivity.
    pub tolerance: f64,
}

impl ArbitrageCheck for LocalVolDensityCheck {
    fn name(&self) -> &str {
        "Local Vol Density"
    }

    fn check(&self, surface: &VolSurface) -> Vec<ArbitrageViolation> {
        let expiries = surface.expiries();
        let strikes = surface.strikes();
        let mut violations = Vec::new();

        if expiries.len() < 2 || strikes.len() < 3 {
            return violations;
        }

        for (ei, &t) in expiries.iter().enumerate() {
            for (si, &big_k) in strikes.iter().enumerate() {
                let k = (big_k / self.forward).ln(); // log-moneyness
                let v = surface.value_clamped(t, big_k);
                let w = v * v * t;

                if w < 1e-14 {
                    continue; // Skip near-zero variance points
                }

                // dw/dT via finite differences along the expiry dimension
                let dw_dt = finite_diff_time(surface, expiries, ei, strikes[si]);

                // dw/dk and d2w/dk2 via finite differences along the strike dimension
                let (dw_dk, d2w_dk2) = finite_diff_strike(surface, strikes, si, expiries[ei]);

                // Dupire denominator
                let term1 = 1.0 - k / w * dw_dk;
                let term2 = 0.25 * (-0.25 - 1.0 / w + k * k / (w * w)) * dw_dk * dw_dk;
                let term3 = 0.5 * d2w_dk2;
                let denominator = term1 + term2 + term3;

                // Local variance = dw_dt / denominator
                if denominator < -self.tolerance {
                    let magnitude = -denominator;
                    let severity = classify_severity(magnitude, 1e-8, 1e-5, 1e-3);
                    violations.push(ArbitrageViolation {
                        violation_type: ArbitrageType::LocalVolDensity,
                        location: ViolationLocation {
                            strike: big_k,
                            expiry: t,
                            adjacent_expiry: None,
                        },
                        severity,
                        magnitude,
                        description: format!(
                            "Negative Dupire density at T={t:.4}, K={big_k:.2}: \
                            denominator = {denominator:.2e}"
                        ),
                        suggested_fix: None,
                    });
                } else if denominator > self.tolerance && dw_dt < -self.tolerance {
                    // Positive denominator but negative numerator: calendar spread
                    // component detected via density check.
                    let local_var = dw_dt / denominator;
                    if local_var < -self.tolerance {
                        let magnitude = -local_var;
                        let severity = classify_severity(magnitude, 1e-8, 1e-5, 1e-3);
                        violations.push(ArbitrageViolation {
                            violation_type: ArbitrageType::LocalVolDensity,
                            location: ViolationLocation {
                                strike: big_k,
                                expiry: t,
                                adjacent_expiry: None,
                            },
                            severity,
                            magnitude,
                            description: format!(
                                "Negative local variance at T={t:.4}, K={big_k:.2}: \
                                sigma^2_local = {local_var:.2e}"
                            ),
                            suggested_fix: None,
                        });
                    }
                }
            }
        }

        violations
    }
}

/// Compute dw/dT at a grid point using finite differences along the expiry axis.
///
/// Uses central differences for interior points and one-sided differences at
/// boundaries.
fn finite_diff_time(surface: &VolSurface, expiries: &[f64], ei: usize, strike: f64) -> f64 {
    let total_var = |idx: usize| -> f64 {
        let t = expiries[idx];
        let v = surface.value_clamped(t, strike);
        v * v * t
    };

    let n = expiries.len();
    if n < 2 {
        return 0.0;
    }

    if ei == 0 {
        // Forward difference
        let dt = expiries[1] - expiries[0];
        if dt.abs() < 1e-14 {
            return 0.0;
        }
        (total_var(1) - total_var(0)) / dt
    } else if ei == n - 1 {
        // Backward difference
        let dt = expiries[n - 1] - expiries[n - 2];
        if dt.abs() < 1e-14 {
            return 0.0;
        }
        (total_var(n - 1) - total_var(n - 2)) / dt
    } else {
        // Central difference
        let dt = expiries[ei + 1] - expiries[ei - 1];
        if dt.abs() < 1e-14 {
            return 0.0;
        }
        (total_var(ei + 1) - total_var(ei - 1)) / dt
    }
}

/// Compute dw/dk and d2w/dk2 at a grid point using finite differences along
/// the strike axis.
///
/// Returns (first_derivative, second_derivative) of total variance with
/// respect to log-moneyness.
fn finite_diff_strike(surface: &VolSurface, strikes: &[f64], si: usize, expiry: f64) -> (f64, f64) {
    let total_var = |idx: usize| -> f64 {
        let v = surface.value_clamped(expiry, strikes[idx]);
        v * v * expiry
    };

    let n = strikes.len();
    if n < 3 {
        return (0.0, 0.0);
    }

    if si == 0 {
        // Forward differences
        let dk1 = strikes[1] - strikes[0];
        let dk2 = strikes[2] - strikes[0];
        if dk1.abs() < 1e-14 || dk2.abs() < 1e-14 {
            return (0.0, 0.0);
        }
        let w0 = total_var(0);
        let w1 = total_var(1);
        let w2 = total_var(2);
        let dw_dk = (w1 - w0) / dk1;
        let dk12 = strikes[2] - strikes[1];
        let dk_avg = 0.5 * (dk1 + dk12);
        let d2w_dk2 = (w2 / dk12 - w1 * (1.0 / dk12 + 1.0 / dk1) + w0 / dk1) / dk_avg;
        (dw_dk, d2w_dk2)
    } else if si == n - 1 {
        // Backward differences
        let dk1 = strikes[n - 1] - strikes[n - 2];
        let dk2 = strikes[n - 1] - strikes[n - 3];
        if dk1.abs() < 1e-14 || dk2.abs() < 1e-14 {
            return (0.0, 0.0);
        }
        let w0 = total_var(n - 3);
        let w1 = total_var(n - 2);
        let w2 = total_var(n - 1);
        let dw_dk = (w2 - w1) / dk1;
        let dk01 = strikes[n - 2] - strikes[n - 3];
        let dk_avg = 0.5 * (dk01 + dk1);
        let d2w_dk2 = (w2 / dk1 - w1 * (1.0 / dk1 + 1.0 / dk01) + w0 / dk01) / dk_avg;
        (dw_dk, d2w_dk2)
    } else {
        // Central differences
        let dk_minus = strikes[si] - strikes[si - 1];
        let dk_plus = strikes[si + 1] - strikes[si];
        if dk_minus.abs() < 1e-14 || dk_plus.abs() < 1e-14 {
            return (0.0, 0.0);
        }
        let w_minus = total_var(si - 1);
        let w_center = total_var(si);
        let w_plus = total_var(si + 1);

        let dw_dk = (w_plus - w_minus) / (dk_minus + dk_plus);
        let dk_avg = 0.5 * (dk_minus + dk_plus);
        let d2w_dk2 = (w_plus / dk_plus - w_center * (1.0 / dk_plus + 1.0 / dk_minus)
            + w_minus / dk_minus)
            / dk_avg;

        (dw_dk, d2w_dk2)
    }
}
