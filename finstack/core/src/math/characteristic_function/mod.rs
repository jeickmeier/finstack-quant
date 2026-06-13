//! Characteristic function trait and model implementations for Fourier pricing.
//!
//! Provides a generic [`CharacteristicFunction`] trait representing the characteristic
//! function of a log-price process phi(u, t) = E[exp(i * u * ln(S_T / S_0))].
//! All Fourier pricing methods (COS, Lewis, FFT) consume this trait.
//!
//! # Model Implementations
//!
//! - [`BlackScholesCf`]: Geometric Brownian motion
//! - [`VarianceGammaCf`]: Time-changed Brownian motion with Gamma subordinator
//! - [`MertonJumpCf`]: GBM plus compound Poisson jumps with log-normal sizes
//!
//! # References
//!
//! - Cont, R. & Tankov, P. (2004). *Financial Modelling with Jump Processes*.
//!   Chapman & Hall/CRC. Chapter 11.

mod black_scholes;
mod merton;
mod variance_gamma;

pub use black_scholes::BlackScholesCf;
pub use merton::MertonJumpCf;
pub use variance_gamma::VarianceGammaCf;

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
///   Chapman & Hall/CRC. Chapter 11.
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
    /// for best accuracy. Use [`cumulants_from_cf`] for numerical estimation.
    fn cumulants(&self, t: f64) -> Cumulants;
}

/// Estimate cumulants via finite differences of ln(phi(u)).
///
/// Uses centered differences at u = 0 with step h = 1e-4.
/// Accurate for smooth characteristic functions; prefer closed-form
/// expressions when available for better accuracy.
pub fn cumulants_from_cf(cf: &dyn CharacteristicFunction, t: f64) -> Cumulants {
    let h = 1e-4;
    let i = Complex64::i();

    // c_n = (-i)^n * d^n/du^n [ln phi(u)] at u=0
    let ln_phi = |u: f64| -> Complex64 { cf.cf(Complex64::new(u, 0.0), t).ln() };

    let c1 = ((-i) * (ln_phi(h) - ln_phi(-h)) / (2.0 * h)).re;
    let c2 = ((-i).powi(2) * (ln_phi(h) - 2.0 * ln_phi(0.0) + ln_phi(-h)) / (h * h)).re;
    let c3 = ((-i).powi(3)
        * (ln_phi(2.0 * h) - 2.0 * ln_phi(h) + 2.0 * ln_phi(-h) - ln_phi(-2.0 * h))
        / (2.0 * h.powi(3)))
    .re;
    let c4 = ((-i).powi(4)
        * (ln_phi(2.0 * h) - 4.0 * ln_phi(h) + 6.0 * ln_phi(0.0) - 4.0 * ln_phi(-h)
            + ln_phi(-2.0 * h))
        / h.powi(4))
    .re;

    Cumulants { c1, c2, c3, c4 }
}

/// Wrapper that converts a risk-neutral CF into a log-forward CF.
///
/// Given phi(u, t) = E[exp(iu * ln(S_T/S_0))], produces
/// psi(u, t) = E[exp(iu * ln(S_T/F))] = phi(u, t) * exp(-iu * (r-q) * t).
///
/// This is the standard input for COS and Lewis methods which work
/// in the log-forward-moneyness domain.
pub struct LogForwardCf<'a> {
    inner: &'a dyn CharacteristicFunction,
    r: f64,
    q: f64,
}

impl<'a> LogForwardCf<'a> {
    /// Create a new log-forward CF wrapper.
    ///
    /// # Arguments
    ///
    /// * `cf` - The underlying risk-neutral characteristic function
    /// * `r` - Risk-free rate
    /// * `q` - Dividend yield
    pub fn new(cf: &'a dyn CharacteristicFunction, r: f64, q: f64) -> Self {
        Self { inner: cf, r, q }
    }
}

impl CharacteristicFunction for LogForwardCf<'_> {
    fn cf(&self, u: Complex64, t: f64) -> Complex64 {
        let drift_adjust = Complex64::new(0.0, -(self.r - self.q) * t) * u;
        self.inner.cf(u, t) * drift_adjust.exp()
    }

    fn cumulants(&self, t: f64) -> Cumulants {
        let mut c = self.inner.cumulants(t);
        c.c1 -= (self.r - self.q) * t;
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bs_cf_normalization() {
        let bs = BlackScholesCf {
            r: 0.05,
            q: 0.02,
            sigma: 0.2,
        };
        let val = bs.cf(Complex64::new(0.0, 0.0), 1.0);
        assert!(
            (val.re - 1.0).abs() < 1e-12,
            "cf(0,t) should be 1, got {val}"
        );
        assert!(val.im.abs() < 1e-12);
    }

    #[test]
    fn bs_cf_boundedness() {
        let bs = BlackScholesCf {
            r: 0.05,
            q: 0.0,
            sigma: 0.2,
        };
        for u_re in [0.5, 1.0, 5.0, 10.0, 50.0] {
            let val = bs.cf(Complex64::new(u_re, 0.0), 1.0);
            assert!(
                val.norm() <= 1.0 + 1e-10,
                "|cf({u_re},1)| = {} > 1",
                val.norm()
            );
        }
    }

    #[test]
    fn vg_cf_normalization() {
        let vg = VarianceGammaCf {
            r: 0.05,
            q: 0.0,
            sigma: 0.12,
            nu: 0.2,
            theta: -0.14,
        };
        let val = vg.cf(Complex64::new(0.0, 0.0), 1.0);
        assert!(
            (val.re - 1.0).abs() < 1e-10,
            "VG cf(0,t) should be 1, got {val}"
        );
    }

    #[test]
    fn merton_cf_normalization() {
        let merton = MertonJumpCf {
            r: 0.05,
            q: 0.0,
            sigma: 0.2,
            lambda: 1.0,
            mu_j: -0.05,
            sigma_j: 0.1,
        };
        let val = merton.cf(Complex64::new(0.0, 0.0), 1.0);
        assert!(
            (val.re - 1.0).abs() < 1e-10,
            "Merton cf(0,t) should be 1, got {val}"
        );
    }

    #[test]
    fn bs_cumulants_closed_form() {
        let bs = BlackScholesCf {
            r: 0.05,
            q: 0.02,
            sigma: 0.2,
        };
        let c = bs.cumulants(1.0);
        let s2 = 0.2 * 0.2;
        assert!(
            (c.c1 - (0.05 - 0.02 - 0.5 * s2)).abs() < 1e-10,
            "c1 mismatch: {}",
            c.c1
        );
        assert!((c.c2 - s2).abs() < 1e-10, "c2 mismatch: {}", c.c2);
        assert!(c.c3.abs() < 1e-10, "BS c3 should be 0: {}", c.c3);
        assert!(c.c4.abs() < 1e-10, "BS c4 should be 0: {}", c.c4);
    }

    #[test]
    fn numerical_cumulants_match_closed_form_for_bs() {
        let bs = BlackScholesCf {
            r: 0.05,
            q: 0.0,
            sigma: 0.3,
        };
        let closed = bs.cumulants(1.0);
        let numerical = cumulants_from_cf(&bs, 1.0);
        assert!(
            (closed.c1 - numerical.c1).abs() < 1e-6,
            "c1: closed={}, numerical={}",
            closed.c1,
            numerical.c1
        );
        assert!(
            (closed.c2 - numerical.c2).abs() < 1e-6,
            "c2: closed={}, numerical={}",
            closed.c2,
            numerical.c2
        );
    }

    #[test]
    fn cf_models_roundtrip_through_json() {
        let bs = BlackScholesCf {
            r: 0.05,
            q: 0.01,
            sigma: 0.2,
        };
        let bs_restored: BlackScholesCf =
            serde_json::from_str(&serde_json::to_string(&bs).expect("serialize BS"))
                .expect("deserialize BS");
        assert_eq!(bs_restored.r, bs.r);
        assert_eq!(bs_restored.q, bs.q);
        assert_eq!(bs_restored.sigma, bs.sigma);

        let merton = MertonJumpCf {
            r: 0.04,
            q: 0.01,
            sigma: 0.2,
            lambda: 0.7,
            mu_j: -0.05,
            sigma_j: 0.1,
        };
        let merton_restored: MertonJumpCf =
            serde_json::from_str(&serde_json::to_string(&merton).expect("serialize Merton"))
                .expect("deserialize Merton");
        assert_eq!(merton_restored.r, merton.r);
        assert_eq!(merton_restored.lambda, merton.lambda);
        assert_eq!(merton_restored.mu_j, merton.mu_j);

        let vg = VarianceGammaCf::new(0.05, 0.01, 0.12, 0.2, -0.14).expect("valid VG");
        let vg_restored: VarianceGammaCf =
            serde_json::from_str(&serde_json::to_string(&vg).expect("serialize VG"))
                .expect("deserialize VG");
        assert_eq!(vg_restored.r, vg.r);
        assert_eq!(vg_restored.nu, vg.nu);
        assert_eq!(vg_restored.theta, vg.theta);
        assert!(vg_restored.validate().is_ok());
    }

    #[test]
    fn vg_deserialized_invalid_parameters_fail_validate() {
        let vg: VarianceGammaCf =
            serde_json::from_str(r#"{"r":0.05,"q":0.0,"sigma":0.5,"nu":4.0,"theta":0.2}"#)
                .expect("serde allows struct-literal compatibility");

        assert!(vg.validate().is_err());
    }

    #[test]
    fn merton_cumulants_match_numerical_cf_estimates() {
        let merton = MertonJumpCf {
            r: 0.04,
            q: 0.01,
            sigma: 0.2,
            lambda: 0.35,
            mu_j: -0.03,
            sigma_j: 0.08,
        };
        let closed = merton.cumulants(1.25);
        let numerical = cumulants_from_cf(&merton, 1.25);

        assert!((closed.c1 - numerical.c1).abs() < 1e-6);
        assert!((closed.c2 - numerical.c2).abs() < 1e-6);
    }

    #[test]
    fn log_forward_cf_shifts_mean() {
        let bs = BlackScholesCf {
            r: 0.05,
            q: 0.02,
            sigma: 0.2,
        };
        let lf = LogForwardCf::new(&bs, 0.05, 0.02);
        let c_orig = bs.cumulants(1.0);
        let c_fwd = lf.cumulants(1.0);
        assert!(
            (c_fwd.c1 - (c_orig.c1 - (0.05 - 0.02))).abs() < 1e-12,
            "Forward CF should shift c1 by -(r-q)*t"
        );
        assert!(
            (c_fwd.c2 - c_orig.c2).abs() < 1e-12,
            "Variance should be unchanged"
        );
    }

    #[test]
    fn merton_converges_to_bs_as_lambda_zero() {
        let sigma = 0.2;
        let bs = BlackScholesCf {
            r: 0.05,
            q: 0.0,
            sigma,
        };
        // Use lambda = 0 exactly for the convergence test.
        let merton = MertonJumpCf {
            r: 0.05,
            q: 0.0,
            sigma,
            lambda: 0.0,
            mu_j: -0.05,
            sigma_j: 0.1,
        };
        for u_re in [0.5, 1.0, 5.0, 10.0] {
            let u = Complex64::new(u_re, 0.0);
            let bs_val = bs.cf(u, 1.0);
            let merton_val = merton.cf(u, 1.0);
            assert!(
                (bs_val - merton_val).norm() < 1e-12,
                "Merton should converge to BS at u={u_re}: bs={bs_val}, merton={merton_val}"
            );
        }
    }
}
