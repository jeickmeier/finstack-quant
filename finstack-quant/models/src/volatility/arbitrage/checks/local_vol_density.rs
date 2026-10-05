//! Local volatility density check via the Dupire formula.
//!
//! Verifies that the Dupire local variance is positive everywhere on the grid.
//! This is the definitive no-arbitrage condition for a volatility surface: if
//! the local variance is negative at any point, there exists an arbitrage.
//!
//! The formula, its coordinates and its finite differences live in
//! [`crate::volatility::dupire`], shared with the local-volatility extractor.
//! Both numerator (dw/dT >= 0) and denominator (density condition) must be
//! non-negative.

use super::classify_severity;
use crate::volatility::arbitrage::types::{ArbitrageType, ArbitrageViolation, ViolationLocation};
use crate::volatility::dupire::{dupire_node, MoneynessLookup};
use finstack_quant_core::market_data::surfaces::VolSurface;

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
        let surface = VolSurface::from_grid("CARRY", &expiries, &strikes, &values).unwrap();
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
            (2, 0x1512_4344_8f8d_fbe4),
            (2, 0x34a1_2370_010a_583f),
        ];
        assert_eq!(actual.as_slice(), expected, "actual = {actual:#x?}");
    }
}
