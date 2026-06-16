//! Variance Gamma model characteristic function.

use super::{CharacteristicFunction, Cumulants};
use num_complex::Complex64;

/// Variance Gamma model characteristic function.
///
/// The VG process is a time-changed Brownian motion where the subordinator
/// is a Gamma process. Parameters: sigma (volatility), nu (variance rate
/// of Gamma subordinator), theta (drift of subordinated BM).
///
/// phi(u, t) = exp(iu * omega * t) * (1 - iu * theta * nu + 0.5 * sigma^2 * nu * u^2)^{-t/nu}
///
/// where omega = (1/nu) * ln(1 - theta * nu - 0.5 * sigma^2 * nu) is the
/// convexity correction ensuring `E[S_T] = S_0 * exp((r-q)*T)`.
///
/// # Parameter validity
///
/// The convexity correction `omega` takes the log of
/// `1 - theta * nu - 0.5 * sigma^2 * nu`. That expression must be strictly
/// positive for `omega` to be real-valued, i.e.
///
/// ```text
/// theta * nu + 0.5 * sigma^2 * nu < 1
/// ```
///
/// Use [`VarianceGammaCf::new`] to enforce this at construction time.
/// Direct struct-literal construction is still allowed for deserialisation
/// flows, but callers that go that route should invoke
/// [`VarianceGammaCf::validate`] before pricing.
///
/// # References
///
/// - Madan, D. B., Carr, P. P. & Chang, E. C. (1998). "The Variance Gamma
///   Process and Option Pricing." *European Finance Review*, 2, 79-105.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct VarianceGammaCf {
    /// Risk-free rate.
    pub r: f64,
    /// Dividend yield.
    pub q: f64,
    /// Volatility of the subordinated Brownian motion (`sigma > 0`).
    pub sigma: f64,
    /// Variance rate of the Gamma subordinator (`nu > 0`).
    pub nu: f64,
    /// Drift of the subordinated Brownian motion.
    pub theta: f64,
}

impl VarianceGammaCf {
    /// Construct a validated VG characteristic function.
    ///
    /// # Errors
    ///
    /// Returns an error string when `sigma <= 0`, `nu <= 0`, or the
    /// convexity-correction argument `1 - theta*nu - 0.5*sigma^2*nu`
    /// is not strictly positive. The last condition would otherwise
    /// produce a complex (or NaN) `omega`.
    pub fn new(r: f64, q: f64, sigma: f64, nu: f64, theta: f64) -> Result<Self, String> {
        let candidate = Self {
            r,
            q,
            sigma,
            nu,
            theta,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    /// Validate VG parameters against the martingale constraint.
    ///
    /// Checks:
    /// 1. `sigma > 0` and finite
    /// 2. `nu > 0` and finite
    /// 3. `1 - theta*nu - 0.5*sigma^2*nu > 0` (real-valued omega)
    pub fn validate(&self) -> Result<(), String> {
        if !self.sigma.is_finite() || self.sigma <= 0.0 {
            return Err(format!(
                "VG sigma must be > 0 and finite, got {}",
                self.sigma
            ));
        }
        if !self.nu.is_finite() || self.nu <= 0.0 {
            return Err(format!("VG nu must be > 0 and finite, got {}", self.nu));
        }
        if !self.theta.is_finite() {
            return Err(format!("VG theta must be finite, got {}", self.theta));
        }
        let omega_arg = 1.0 - self.theta * self.nu - 0.5 * self.sigma * self.sigma * self.nu;
        if omega_arg <= 0.0 {
            return Err(format!(
                "VG martingale constraint violated: 1 - theta*nu - 0.5*sigma^2*nu = {} \
                 must be > 0 (sigma={}, nu={}, theta={})",
                omega_arg, self.sigma, self.nu, self.theta
            ));
        }
        Ok(())
    }

    /// Convexity correction omega = (1/nu) * ln(1 - theta*nu - 0.5*sigma^2*nu).
    ///
    /// Returns NaN when the argument is non-positive. Callers that want
    /// a guaranteed-finite value should check [`validate`](Self::validate)
    /// first.
    #[inline]
    fn omega(&self) -> f64 {
        let arg = 1.0 - self.theta * self.nu - 0.5 * self.sigma * self.sigma * self.nu;
        if arg <= 0.0 {
            f64::NAN
        } else {
            arg.ln() / self.nu
        }
    }
}

impl CharacteristicFunction for VarianceGammaCf {
    fn cf(&self, u: Complex64, t: f64) -> Complex64 {
        let iu = Complex64::i() * u;
        let omega = self.omega();
        let base = Complex64::new(1.0, 0.0) - iu * self.theta * self.nu
            + 0.5 * self.sigma * self.sigma * self.nu * u * u;
        (iu * (self.r - self.q + omega) * t + (-t / self.nu) * base.ln()).exp()
    }

    fn cumulants(&self, t: f64) -> Cumulants {
        let s2 = self.sigma * self.sigma;
        let omega = self.omega();
        Cumulants {
            // c1 = (r − q + ω + θ)t: the subordinated-BM drift θ contributes
            // to the mean of the log-price (Fang & Oosterlee 2008, Table 2).
            c1: (self.r - self.q + omega + self.theta) * t,
            c2: (s2 + self.nu * self.theta * self.theta) * t,
            c3: (2.0 * self.theta.powi(3) * self.nu * self.nu + 3.0 * s2 * self.theta * self.nu)
                * t,
            c4: (3.0 * s2 * s2 * self.nu
                + 12.0 * s2 * self.theta * self.theta * self.nu * self.nu
                + 6.0 * self.theta.powi(4) * self.nu.powi(3))
                * t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_canonical_equity_parameters() {
        let vg = VarianceGammaCf::new(0.05, 0.0, 0.12, 0.2, -0.14).expect("valid VG");
        assert!(vg.validate().is_ok());
    }

    #[test]
    fn new_rejects_martingale_violation() {
        // 1 - theta*nu - 0.5*sigma^2*nu = 1 - 0.8 - 0.5 = -0.3
        let err = VarianceGammaCf::new(0.05, 0.0, 0.5, 4.0, 0.2)
            .expect_err("martingale violation should be rejected");
        assert!(
            err.contains("martingale"),
            "expected constraint error: {err}"
        );
    }

    #[test]
    fn new_rejects_non_positive_sigma() {
        assert!(VarianceGammaCf::new(0.05, 0.0, 0.0, 0.2, -0.14).is_err());
        assert!(VarianceGammaCf::new(0.05, 0.0, -0.1, 0.2, -0.14).is_err());
    }

    #[test]
    fn new_rejects_non_positive_nu() {
        assert!(VarianceGammaCf::new(0.05, 0.0, 0.12, 0.0, -0.14).is_err());
        assert!(VarianceGammaCf::new(0.05, 0.0, 0.12, -0.2, -0.14).is_err());
    }

    #[test]
    fn cumulants_match_numerical_differentiation_of_cf() {
        // Locks in c1 = (r − q + ω + θ)t (Fang & Oosterlee 2008, Table 2):
        // the pre-fix code dropped the +θt term, mis-centering the COS
        // truncation range for skewed (θ ≠ 0) VG parameters.
        let vg = VarianceGammaCf::new(0.05, 0.01, 0.12, 0.2, -0.14).expect("valid VG");
        let t = 1.5;
        let closed = vg.cumulants(t);
        let numerical = super::super::cumulants_from_cf(&vg, t);
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
    fn validate_catches_unsafe_struct_literal() {
        let vg = VarianceGammaCf {
            r: 0.05,
            q: 0.0,
            sigma: 0.5,
            nu: 4.0,
            theta: 0.2,
        };
        assert!(vg.validate().is_err());
    }
}
