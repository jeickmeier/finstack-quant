//! Individual arbitrage checks.
//!
//! Each check is a small config struct with a `check(&self, surface)` method
//! that inspects a volatility surface for a specific class of arbitrage
//! violation. Checks return `Vec<ArbitrageViolation>` (empty means pass) and
//! never mutate input.
//!
//! ## Butterfly
//!
//! Butterfly arbitrage check via the Durrleman density condition.
//!
//! Verifies that the implied probability density is non-negative by
//! evaluating Durrleman's g(k) function in log-moneyness space.
//!
//! ### Financial Background
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
//! ### References
//!
//! - Durrleman, V. (2003). *From Implied to Spot Volatilities*. PhD thesis.
//! - Gatheral, J. (2006). *The Volatility Surface*. Wiley. `docs/REFERENCES.md#gatheral-volatility-surface`
//!
//! ## Calendar spread
//!
//! Calendar spread arbitrage check: total variance monotonicity in expiry.
//!
//! Verifies that total variance w(k, T) is non-decreasing in maturity at
//! each log-moneyness k. The check operates in log-moneyness space using
//! per-expiry forward prices, which is the correct no-arbitrage condition
//! when forwards drift across expiries.
//!
//! ### Financial Background
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
//! ### References
//!
//! - Gatheral, J. (2006). *The Volatility Surface*. Wiley. Chapter 3. `docs/REFERENCES.md#gatheral-volatility-surface`
//! - Roper, M. (2010). *Arbitrage Free Implied Volatility Surfaces*.
//!
//! ## Local volatility density
//!
//! Local volatility density check via the Dupire formula.
//!
//! Verifies that the Dupire local variance is positive everywhere on the grid.
//! This is the definitive no-arbitrage condition for a volatility surface: if
//! the local variance is negative at any point, there exists an arbitrage.
//!
//! The formula, its coordinates and its finite differences live in
//! the crate-private `volatility::dupire` module, shared with the
//! local-volatility extractor.
//! Both numerator (dw/dT >= 0) and denominator (density condition) must be
//! non-negative.

use super::types::{ArbitrageSeverity, ArbitrageType, ArbitrageViolation, ViolationLocation};
use crate::volatility::dupire::{dupire_node, MoneynessLookup};
use finstack_quant_core::market_data::surfaces::VolSurface;

/// Classify violation magnitude into a severity bucket.
///
/// Thresholds are in total-variance units (sigma^2 * T).
fn classify_severity(
    magnitude: f64,
    minor_threshold: f64,
    major_threshold: f64,
    critical_threshold: f64,
) -> ArbitrageSeverity {
    if magnitude < minor_threshold {
        ArbitrageSeverity::Negligible
    } else if magnitude < major_threshold {
        ArbitrageSeverity::Minor
    } else if magnitude < critical_threshold {
        ArbitrageSeverity::Major
    } else {
        ArbitrageSeverity::Critical
    }
}

/// Checks that implied probability density is non-negative via Durrleman's
/// g(k) condition in log-moneyness space.
///
/// Requires per-expiry forward prices to convert cash strikes to
/// log-moneyness. When constructed via the orchestrator [`check_surface`],
/// forwards are taken from [`ArbitrageCheckConfig::forward_prices`].
///
/// [`check_surface`]: crate::volatility::arbitrage::check_surface
/// [`ArbitrageCheckConfig::forward_prices`]: crate::volatility::arbitrage::ArbitrageCheckConfig::forward_prices
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
            tolerance: crate::volatility::arbitrage::DEFAULT_ARBITRAGE_TOLERANCE,
        }
    }
}

impl ButterflyCheck {
    /// Run the butterfly (Durrleman density) check and return every violation found.
    ///
    /// An empty `Vec` means the check passes. The surface is not mutated.
    ///
    /// # Arguments
    ///
    /// * `surface` - Implied-volatility surface on an expiry (years) by cash
    ///   strike grid; expiries pair positionally with `self.forwards`.
    pub fn check(&self, surface: &VolSurface) -> Vec<ArbitrageViolation> {
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
                    let v = crate::volatility::get_surface_vol_clamped(surface, t, k);
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

                let dk_sum = dk_plus + dk_minus;
                let w_prime = -dk_plus / (dk_minus * dk_sum) * ws[i - 1]
                    + (dk_plus - dk_minus) / (dk_minus * dk_plus) * ws[i]
                    + dk_minus / (dk_plus * dk_sum) * ws[i + 1];

                let dk_avg = 0.5 * dk_sum;
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
            tolerance: crate::volatility::arbitrage::DEFAULT_ARBITRAGE_TOLERANCE,
        }
    }
}

impl CalendarSpreadCheck {
    /// Run the calendar-spread check and return every violation found.
    ///
    /// An empty `Vec` means the check passes. The surface is not mutated.
    ///
    /// # Arguments
    ///
    /// * `surface` - Implied-volatility surface on an expiry (years) by cash
    ///   strike grid; expiries pair positionally with `self.forwards`.
    pub fn check(&self, surface: &VolSurface) -> Vec<ArbitrageViolation> {
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
                let v1 = crate::volatility::get_surface_vol_clamped(surface, t1, big_k);
                let w1 = v1 * v1 * t1;

                // Total variance at (k, T2): find the cash strike that
                // corresponds to the same log-moneyness under T2's forward
                let big_k2 = f2 * (k).exp();
                let v2 = crate::volatility::get_surface_vol_clamped(surface, t2, big_k2);
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

/// Checks that the Dupire local variance is positive everywhere on the grid.
pub struct LocalVolDensityCheck {
    /// Finite positive forward prices, one per surface expiry, defining the
    /// fixed-log-moneyness coordinates of the time derivative.
    pub forwards: Vec<f64>,
    /// Tolerance for density positivity.
    pub tolerance: f64,
}

impl LocalVolDensityCheck {
    /// Run the Dupire local-variance positivity check and return every violation found.
    ///
    /// An empty `Vec` means the check passes. The surface is not mutated.
    ///
    /// # Arguments
    ///
    /// * `surface` - Implied-volatility surface on an expiry (years) by cash
    ///   strike grid; expiries pair positionally with `self.forwards`.
    pub fn check(&self, surface: &VolSurface) -> Vec<ArbitrageViolation> {
        let expiries = surface.expiries();
        let strikes = surface.strikes();
        let mut violations = Vec::new();

        if expiries.len() < 2
            || strikes.len() < 3
            || self.forwards.len() != expiries.len()
            || self.forwards.iter().any(|f| !f.is_finite() || *f <= 0.0)
        {
            return violations;
        }

        for (ei, &t) in expiries.iter().enumerate() {
            for (si, &big_k) in strikes.iter().enumerate() {
                // Near-zero variance points are skipped.
                let Some(node) =
                    dupire_node(surface, &self.forwards, ei, si, MoneynessLookup::Strict)
                else {
                    continue;
                };
                let (dw_dt, denominator) = (node.dw_dt, node.density);

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
                } else if denominator > self.tolerance
                    && dw_dt.is_some_and(|slope| slope < -self.tolerance)
                {
                    // Positive denominator but negative numerator: calendar spread
                    // component detected via density check.
                    let Some(dw_dt) = dw_dt else {
                        continue;
                    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volatility::dupire::finite_diff_time;

    #[test]
    fn time_derivative_holds_log_moneyness_fixed_under_carry() {
        let expiries = [1.0, 2.0];
        let forwards = [100.0, 150.0];
        let strikes: [f64; 7] = [50.0, 75.0, 100.0, 125.0, 150.0, 175.0, 200.0];
        let mut values = Vec::new();
        for (&time, &forward) in expiries.iter().zip(&forwards) {
            for &strike in &strikes {
                let variance = 0.04 + 0.02 * (strike / forward).ln() + 0.001 * time;
                values.push((variance / time).sqrt());
            }
        }
        let surface = VolSurface::from_grid(
            "CARRY",
            &expiries,
            &strikes,
            &values,
            Default::default(),
            None,
        )
        .unwrap();
        let fixed_strike_slope = 2.0 * values[7 + 2].powi(2) - values[2].powi(2);
        assert!(fixed_strike_slope < -0.005);
        for ei in 0..2 {
            let derivative = finite_diff_time(
                &surface,
                &expiries,
                &forwards,
                ei,
                0.0,
                MoneynessLookup::Strict,
            )
            .unwrap();
            assert!((derivative - 0.001).abs() < 1e-14);
        }
        assert!(
            finite_diff_time(
                &surface,
                &expiries,
                &forwards,
                0,
                2.0_f64.ln(),
                MoneynessLookup::Strict
            )
            .is_none(),
            "out-of-grid moneyness must not be replaced by a clamped cash strike"
        );
        let violations = LocalVolDensityCheck {
            forwards: forwards.to_vec(),
            tolerance: crate::volatility::arbitrage::DEFAULT_ARBITRAGE_TOLERANCE,
        }
        .check(&surface);
        assert!(
            violations.is_empty(),
            "positive fixed-moneyness slopes must pass: {violations:?}"
        );
    }

    /// FNV-1a digest of every violation's location, magnitude, severity and
    /// description, so any change in the check's arithmetic is visible.
    fn digest(violations: &[ArbitrageViolation]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut feed = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for violation in violations {
            feed(&violation.location.strike.to_bits().to_le_bytes());
            feed(&violation.location.expiry.to_bits().to_le_bytes());
            feed(&violation.magnitude.to_bits().to_le_bytes());
            feed(format!("{:?}", violation.severity).as_bytes());
            feed(violation.description.as_bytes());
        }
        hash
    }

    /// Pins the check's output bit for bit on an arbitrage-free surface, a
    /// calendar-violating one, a butterfly-violating one and a hand-drawn
    /// smile, each under sloped forwards. Captured before the Dupire kernel
    /// was shared with the local-volatility extractor.
    #[test]
    fn violations_are_bit_identical_to_the_pinned_run() {
        let flat = VolSurface::from_grid(
            "FLAT",
            &[0.25, 0.5, 1.0, 2.0],
            &[80.0, 90.0, 100.0, 110.0, 120.0],
            &[0.2; 20],
            Default::default(),
            None,
        )
        .unwrap();
        let calendar = VolSurface::builder("CALENDAR-BAD")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[0.30, 0.25, 0.20, 0.25, 0.30])
            .row(&[0.30, 0.25, 0.25, 0.25, 0.30])
            .row(&[0.20, 0.15, 0.15, 0.15, 0.20])
            .build()
            .unwrap();
        let butterfly = VolSurface::builder("BUTTERFLY-BAD")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[70.0, 85.0, 100.0, 115.0, 130.0])
            .row(&[0.20, 0.45, 0.20, 0.45, 0.20])
            .row(&[0.21, 0.46, 0.21, 0.46, 0.21])
            .row(&[0.22, 0.47, 0.22, 0.47, 0.22])
            .build()
            .unwrap();
        let smile = VolSurface::builder("SMILE")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0])
            .row(&[0.30, 0.25, 0.22, 0.20, 0.21, 0.23, 0.28])
            .row(&[0.28, 0.24, 0.21, 0.19, 0.20, 0.22, 0.26])
            .row(&[0.26, 0.22, 0.20, 0.18, 0.19, 0.21, 0.24])
            .row(&[0.24, 0.21, 0.19, 0.17, 0.18, 0.20, 0.22])
            .build()
            .unwrap();
        let run = |surface: &VolSurface, forwards: &[f64], tolerance: f64| {
            let violations = LocalVolDensityCheck {
                forwards: forwards.to_vec(),
                tolerance,
            }
            .check(surface);
            (violations.len(), digest(&violations))
        };
        let actual = [
            run(&flat, &[100.0, 101.0, 102.0, 104.0], 1e-10),
            run(&calendar, &[100.0, 101.0, 102.0], 1e-10),
            run(&calendar, &[100.0, 100.0, 100.0], 1e-6),
            run(&butterfly, &[100.0, 100.5, 101.5], 1e-10),
            run(&smile, &[100.0, 100.5, 101.0, 103.0], 1e-10),
            run(&smile, &[100.0, 100.0, 100.0, 100.0], 0.0),
        ];
        let expected: &[(usize, u64)] = &[
            (0, 0xcbf2_9ce4_8422_2325),
            (4, 0x797f_068f_cf44_9335),
            (5, 0x927f_0170_6cf3_7ef2),
            (7, 0xa13f_96de_6a06_612a),
            (2, 0x4b1f_a919_bef7_1130),
            (2, 0x60b4_6cc5_569b_6981),
        ];
        assert_eq!(actual.as_slice(), expected, "actual = {actual:#x?}");
    }
}
