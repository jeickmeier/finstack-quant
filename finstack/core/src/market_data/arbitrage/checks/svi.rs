//! SVI-specific arbitrage checks for calibrated SVI slices.
//!
//! Requires the raw SVI parameters (not a VolSurface) because the checks
//! operate on the analytical SVI formula rather than discrete grid points.
//! This enables exact derivative computation and avoids finite-difference
//! artifacts.
//!
//! # Checks Implemented
//!
//! - **Moment bounds**: Roger Lee (2004) asymptotic slope constraint
//! - **Butterfly density**: Gatheral-Jacquier g(k) >= 0 sufficient condition
//! - **Calendar spread**: Cross-slice total variance ordering

use super::classify_severity;
use crate::market_data::arbitrage::types::{
    ArbitrageSeverity, ArbitrageType, ArbitrageViolation, ViolationLocation,
};
use crate::math::volatility::svi::SviParams;

/// SVI-specific arbitrage checks for a set of calibrated SVI slices.
///
/// Requires the raw SVI parameters (not a VolSurface) because the checks
/// operate on the analytical SVI formula rather than discrete grid points.
pub struct SviArbitrageCheck {
    /// Expiry for each SVI slice (must be sorted ascending).
    pub expiries: Vec<f64>,
    /// SVI parameters for each slice.
    pub params: Vec<SviParams>,
    /// Range of log-moneyness to scan for violations.
    pub k_range: (f64, f64),
    /// Number of sample points across the k range.
    pub n_samples: usize,
}

impl SviArbitrageCheck {
    /// Check Roger Lee moment bounds: the asymptotic slope of total variance
    /// must satisfy |dw/dk| <= 2 at extreme strikes.
    ///
    /// For raw SVI, the left/right wing slopes are b(1+rho) and b(1-rho).
    /// Both must be <= 2.
    pub fn check_moment_bounds(&self) -> Vec<ArbitrageViolation> {
        let mut violations = Vec::new();

        for (i, params) in self.params.iter().enumerate() {
            let t = self.expiries[i];
            let left_slope = params.b * (1.0 + params.rho);
            let right_slope = params.b * (1.0 - params.rho);

            if left_slope > 2.0 + 1e-12 {
                violations.push(ArbitrageViolation {
                    violation_type: ArbitrageType::SviMomentBound,
                    location: ViolationLocation {
                        strike: f64::NEG_INFINITY,
                        expiry: t,
                        adjacent_expiry: None,
                    },
                    severity: ArbitrageSeverity::Major,
                    magnitude: left_slope - 2.0,
                    description: format!(
                        "SVI left wing slope b(1+rho) = {left_slope:.4} > 2 at T={t:.4}"
                    ),
                    suggested_fix: None,
                });
            }

            if right_slope > 2.0 + 1e-12 {
                violations.push(ArbitrageViolation {
                    violation_type: ArbitrageType::SviMomentBound,
                    location: ViolationLocation {
                        strike: f64::INFINITY,
                        expiry: t,
                        adjacent_expiry: None,
                    },
                    severity: ArbitrageSeverity::Major,
                    magnitude: right_slope - 2.0,
                    description: format!(
                        "SVI right wing slope b(1-rho) = {right_slope:.4} > 2 at T={t:.4}"
                    ),
                    suggested_fix: None,
                });
            }
        }

        violations
    }

    /// Check Gatheral-Jacquier sufficient conditions for no butterfly
    /// arbitrage within each SVI slice.
    ///
    /// For each slice, evaluates the density function g(k) at sampled
    /// log-moneyness points. g(k) >= 0 is required for no butterfly arb.
    ///
    /// g(k) = (1 - k*w'/(2w))^2 - w'^2/4 * (1/w + 1/4) + w''/2
    ///
    /// where w = w(k), w' = dw/dk, w'' = d2w/dk2.
    pub fn check_butterfly_density(&self) -> Vec<ArbitrageViolation> {
        let mut violations = Vec::new();

        if self.n_samples == 0 {
            return violations;
        }

        let dk = (self.k_range.1 - self.k_range.0) / self.n_samples as f64;

        for (i, params) in self.params.iter().enumerate() {
            let t = self.expiries[i];

            for j in 0..=self.n_samples {
                let k = self.k_range.0 + j as f64 * dk;
                let w = params.total_variance(k);

                if w < 1e-14 {
                    continue;
                }

                // Analytical derivatives of raw SVI
                let km = k - params.m;
                let r = (km * km + params.sigma * params.sigma).sqrt();
                let w_prime = params.b * (params.rho + km / r);
                let w_double_prime = params.b * params.sigma * params.sigma / (r * r * r);

                // Density function g(k)
                let term1 = 1.0 - k * w_prime / (2.0 * w);
                let g = term1 * term1 - w_prime * w_prime / 4.0 * (1.0 / w + 0.25)
                    + w_double_prime / 2.0;

                if g < -1e-10 {
                    violations.push(ArbitrageViolation {
                        violation_type: ArbitrageType::SviButterflyCondition,
                        location: ViolationLocation {
                            strike: k, // log-moneyness
                            expiry: t,
                            adjacent_expiry: None,
                        },
                        severity: classify_severity(-g, 1e-8, 1e-5, 1e-3),
                        magnitude: -g,
                        description: format!("SVI density g(k={k:.4}) = {g:.2e} < 0 at T={t:.4}"),
                        suggested_fix: None,
                    });
                }
            }
        }

        violations
    }

    /// Check calendar spread freedom across SVI slices.
    ///
    /// For each pair of adjacent slices (T1, T2) with T2 > T1, verifies
    /// that w(k, T2) >= w(k, T1) at sampled log-moneyness points.
    pub fn check_calendar_spread(&self) -> Vec<ArbitrageViolation> {
        let mut violations = Vec::new();

        if self.n_samples == 0 {
            return violations;
        }

        let dk = (self.k_range.1 - self.k_range.0) / self.n_samples as f64;

        for i in 0..self.params.len().saturating_sub(1) {
            let t1 = self.expiries[i];
            let t2 = self.expiries[i + 1];
            let p1 = &self.params[i];
            let p2 = &self.params[i + 1];

            for j in 0..=self.n_samples {
                let k = self.k_range.0 + j as f64 * dk;
                let w1 = p1.total_variance(k);
                let w2 = p2.total_variance(k);

                if w2 < w1 - 1e-10 {
                    let decrease = w1 - w2;
                    violations.push(ArbitrageViolation {
                        violation_type: ArbitrageType::SviCalendarSpread,
                        location: ViolationLocation {
                            strike: k,
                            expiry: t2,
                            adjacent_expiry: Some(t1),
                        },
                        severity: classify_severity(decrease, 1e-8, 1e-5, 1e-3),
                        magnitude: decrease,
                        description: format!(
                            "SVI calendar spread at k={k:.4}: w(T={t2:.4}) = {w2:.6} \
                            < w(T={t1:.4}) = {w1:.6}"
                        ),
                        suggested_fix: None,
                    });
                }
            }
        }

        violations
    }

    /// Run all SVI checks and aggregate.
    pub fn check_all(&self) -> Vec<ArbitrageViolation> {
        let mut all = self.check_moment_bounds();
        all.extend(self.check_butterfly_density());
        all.extend(self.check_calendar_spread());
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn butterfly_density_reports_negative_density_samples() {
        let params = SviParams {
            a: 0.044_305_840_516_802_71,
            b: 0.553_365_870_766_427_3,
            rho: -0.6284714869358965,
            m: -0.25653822799947434,
            sigma: 0.066_468_598_928_966_02,
        };
        params
            .validate()
            .expect("chosen SVI slice satisfies constructor-level constraints");

        let check = SviArbitrageCheck {
            expiries: vec![1.0],
            params: vec![params],
            k_range: (-1.0, 1.0),
            n_samples: 200,
        };

        let violations = check.check_butterfly_density();
        assert!(
            violations
                .iter()
                .any(|v| matches!(v.violation_type, ArbitrageType::SviButterflyCondition)),
            "negative SVI density should be reported as a butterfly violation"
        );
        assert!(violations.iter().all(|v| v.magnitude > 0.0));
    }

    #[test]
    fn butterfly_density_clean_params_return_no_violations() {
        let params = SviParams {
            a: 0.04,
            b: 0.20,
            rho: -0.2,
            m: 0.0,
            sigma: 0.3,
        };
        params.validate().expect("clean SVI params should validate");

        let check = SviArbitrageCheck {
            expiries: vec![1.0],
            params: vec![params],
            k_range: (-1.0, 1.0),
            n_samples: 200,
        };

        assert!(check.check_butterfly_density().is_empty());
    }

    #[test]
    fn butterfly_density_zero_samples_returns_no_violations() {
        let check = SviArbitrageCheck {
            expiries: vec![1.0],
            params: vec![SviParams {
                a: 0.04,
                b: 0.20,
                rho: -0.2,
                m: 0.0,
                sigma: 0.3,
            }],
            k_range: (-1.0, 1.0),
            n_samples: 0,
        };

        assert!(check.check_butterfly_density().is_empty());
    }
}
