//! Quadratic-Exponential (QE) scheme for Heston variance process.
//!
//! The QE scheme (Andersen, 2008) ensures positive variance while maintaining
//! accuracy for the CIR-type variance process in Heston.
//!
//! Reference: Andersen (2008) - "Simple and efficient simulation of the Heston stochastic volatility model"

use super::super::process::heston::HestonProcess;
use super::super::traits::Discretization;
#[cfg(test)]
use super::qe_common::qe_step_variance;
use super::qe_common::{qe_regime, KAPPA_DT_EXPANSION_EPS};

/// Integrated variance approximation method.
///
/// Controls how the integrated variance ∫_t^{t+Δt} v_s ds is computed
/// for the martingale-corrected spot evolution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IntegratedVarianceMethod {
    /// Trapezoidal (midpoint) approximation: (v_t + v_{t+Δt}) / 2 × Δt
    ///
    /// This is the standard approximation from Andersen (2008) Section 3.2.4.
    /// Adequate for most use cases with monthly or finer time steps.
    #[default]
    Trapezoidal,

    /// Mean-reversion-adjusted trapezoidal correction (Andersen 2008 QE-M).
    ///
    /// Replaces the plain trapezoidal rule with a closed-form expression that
    /// accounts for the exponential mean-reversion of the CIR variance process:
    /// ```text
    /// ∫_t^{t+Δt} v_s ds ≈ θ·Δt + (v_t + v_{t+Δt} − 2θ)(1 − e^{−κΔt}) / (2κ)
    /// ```
    /// This reduces to the trapezoidal rule `(v_t + v_{t+Δt})/2 · Δt` as
    /// `κ → 0` and to `θ·Δt` as `κ → ∞` or `v_t = v_{t+Δt} = θ`. It removes
    /// the leading drift bias of the trapezoidal approximation for high
    /// mean-reversion (κ > 5) or coarse time steps.
    ///
    /// This is **not** the Broadie & Kaya (2006) exact simulation of the
    /// conditional integrated-variance distribution (which requires inverting
    /// the conditional characteristic function); it is the lightweight drift
    /// correction standard in Andersen's QE-M implementation. If true
    /// unbiased simulation is required, a separate method variant should be
    /// added.
    ///
    /// # Naming
    ///
    /// The variant name avoids "Exact" — which risked suggesting
    /// Broadie–Kaya-class unbiased simulation — and instead describes
    /// the formula: a drift-corrected trapezoidal rule on the CIR
    /// integrated variance.
    ///
    /// Reference: Andersen, L. (2008). "Simple and efficient simulation of
    /// the Heston stochastic volatility model." *Journal of Computational
    /// Finance*, 11(3), §3.5 and Eq. (33).
    MeanReversionAdjusted,
}

/// QE discretization for Heston model.
///
/// This scheme handles both the variance (CIR process) and the spot price,
/// ensuring variance stays positive while maintaining good accuracy.
///
/// # Algorithm
///
/// For variance:
/// - Compute ψ = s²/m² (scaled variance)
/// - If ψ <= ψ_c: use power/gamma approximation
/// - If ψ > ψ_c: use exponential/uniform mixture
///
/// For spot:
/// - Use a martingale-corrected log update with integrated variance approximation
///
/// # Integrated Variance Options
///
/// The spot evolution requires an estimate of ∫_t^{t+Δt} v_s ds.
/// Two methods are available:
///
/// - [`IntegratedVarianceMethod::Trapezoidal`] (default): (v_t + v_{t+Δt}) / 2 × Δt
/// - [`IntegratedVarianceMethod::MeanReversionAdjusted`]: Uses conditional expectation formula
///
/// The mean-reversion-adjusted method is more accurate for high mean-reversion
/// or coarse time steps, but remains an approximation.
#[derive(Debug, Clone)]
pub struct QeHeston {
    /// Critical value for ψ (default 1.5)
    psi_c: f64,
    /// Integrated variance method
    int_var_method: IntegratedVarianceMethod,
}

impl QeHeston {
    /// Create a new QE Heston discretization with default settings.
    pub fn new() -> Self {
        Self {
            psi_c: 1.5,
            int_var_method: IntegratedVarianceMethod::default(),
        }
    }

    /// Create with custom ψ_c threshold.
    pub fn with_psi_c(psi_c: f64) -> Self {
        Self {
            psi_c,
            int_var_method: IntegratedVarianceMethod::default(),
        }
    }

    /// Set the integrated variance method.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_monte_carlo::discretization::qe_heston::{
    ///     QeHeston, IntegratedVarianceMethod
    /// };
    ///
    /// // Use the mean-reversion-adjusted approximation for high-kappa scenarios.
    /// let qe = QeHeston::new().with_integrated_variance(IntegratedVarianceMethod::MeanReversionAdjusted);
    /// ```
    pub fn with_integrated_variance(mut self, method: IntegratedVarianceMethod) -> Self {
        self.int_var_method = method;
        self
    }

    /// Create with the mean-reversion-adjusted integrated variance approximation.
    ///
    /// This is recommended when κ > 5 or using coarse time steps (Δt > 0.1 years).
    /// The method is not a Broadie-Kaya exact integrated-variance draw.
    pub fn exact_variance() -> Self {
        Self {
            psi_c: 1.5,
            int_var_method: IntegratedVarianceMethod::MeanReversionAdjusted,
        }
    }

    /// One QE step of the Heston variance process.
    ///
    /// Thin wrapper around [`qe_step_variance`] that plugs in the instance's
    /// ψ threshold. See [`super::qe_common`] for the algorithm, references,
    /// and the numerical safeguards that are shared with `QeCir`.
    /// (Production stepping goes through [`qe_regime`] directly so the spot
    /// leg can reuse the regime for the K0* correction; this wrapper remains
    /// for tests.)
    #[cfg(test)]
    #[inline]
    fn step_variance(
        &self,
        v_t: f64,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        dt: f64,
        z_v: f64,
    ) -> f64 {
        qe_step_variance(v_t, kappa, theta, sigma_v, dt, z_v, self.psi_c)
    }

    /// Compute integrated variance for spot evolution.
    ///
    /// Supports two methods:
    ///
    /// ## Trapezoidal (default)
    ///
    /// ```text
    /// ∫_t^{t+Δt} v_s ds ≈ (v_t + v_{t+Δt}) / 2 × Δt
    /// ```
    ///
    /// Standard in the QE scheme, adequate for typical time steps (monthly or finer).
    ///
    /// ## Mean-reversion-adjusted trapezoidal
    ///
    /// Corrects the trapezoidal rule with a mean-reversion weighting factor:
    /// ```text
    /// ∫v ≈ θT + (v₀ + v_T - 2θ)(1 - e^{-κT}) / (2κ)
    /// ```
    ///
    /// This reduces to:
    /// - **(v₀+v_T)/2 · T** for κ → 0 (plain trapezoidal)
    /// - **θT** for κ → ∞ or v₀ = v_T = θ (mean dominates)
    ///
    /// Not the full Broadie-Kaya (2006) conditional distribution (which requires
    /// Fourier inversion); rather a lightweight correction that is standard in
    /// Andersen (2008) QE implementations.
    ///
    /// # Arguments
    /// * `v_t` - Current variance
    /// * `v_next` - Next variance (already simulated)
    /// * `dt` - Time step
    /// * `kappa` - Mean reversion speed (only used for Exact method)
    /// * `theta` - Long-run variance (only used for Exact method)
    ///
    /// # Returns
    /// Integrated variance over [t, t+dt]
    ///
    /// # References
    /// - Andersen, L. (2008). "Simple and efficient simulation of the Heston
    ///   stochastic volatility model." *J. Comp. Finance*, 11(3).
    ///
    /// (Production stepping uses [`Self::int_var_coeffs`] directly; this
    /// helper remains for tests of the integrated-variance approximations.)
    #[cfg(test)]
    #[inline]
    fn integrated_variance(&self, v_t: f64, v_next: f64, dt: f64, kappa: f64, theta: f64) -> f64 {
        let (c0, c1, c2) = self.int_var_coeffs(dt, kappa, theta);
        c0 + c1 * v_t + c2 * v_next
    }

    /// Affine coefficients `(c0, c1, c2)` of the integrated-variance
    /// approximation `∫_t^{t+Δt} v_s ds ≈ c0 + c1·v_t + c2·v_{t+Δt}`.
    ///
    /// Both supported methods are affine in `(v_t, v_{t+Δt})`, which is what
    /// makes the closed-form `K0*` martingale correction possible:
    ///
    /// - Trapezoidal: `(0, Δt/2, Δt/2)` (Andersen's γ1 = γ2 = ½).
    /// - Mean-reversion-adjusted: `c1 = c2 = (1 − e^{−κΔt})/(2κ)`,
    ///   `c0 = θΔt − 2θ·c1`.
    #[inline]
    fn int_var_coeffs(&self, dt: f64, kappa: f64, theta: f64) -> (f64, f64, f64) {
        match self.int_var_method {
            IntegratedVarianceMethod::Trapezoidal => (0.0, 0.5 * dt, 0.5 * dt),
            IntegratedVarianceMethod::MeanReversionAdjusted => {
                if (kappa * dt).abs() < KAPPA_DT_EXPANSION_EPS {
                    (0.0, 0.5 * dt, 0.5 * dt)
                } else {
                    let c1 = (1.0 - (-kappa * dt).exp()) / (2.0 * kappa);
                    (theta * dt - 2.0 * theta * c1, c1, c1)
                }
            }
        }
    }
}

impl Default for QeHeston {
    fn default() -> Self {
        Self::new()
    }
}

impl Discretization<HestonProcess> for QeHeston {
    fn step(
        &self,
        process: &HestonProcess,
        _t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        _work: &mut [f64],
    ) {
        let params = process.params();

        let s_t = x[0];
        let v_t = x[1].max(0.0);

        // Step 1: Evolve variance using QE scheme. The regime is kept so the
        // spot leg can form the exact conditional MGF of v_{t+Δt} for the
        // martingale-exact K0* correction (Andersen 2008, §4.2).
        let z_v = z[1]; // Independent shock for variance
        let regime = qe_regime(
            v_t,
            params.kappa,
            params.theta,
            params.sigma_v,
            dt,
            self.psi_c,
        );
        let v_next = regime.sample(z_v);

        // Step 2: Evolve the spot. With the affine integrated-variance
        // approximation ∫v ≈ c0 + c1·v_t + c2·v_{t+Δt}, the log-return is
        //
        //   Δln S = (r−q)Δt + C + B·v_t + A_pre·v_{t+Δt}
        //           + √((1−ρ²)·∫v)·Z
        //
        // with C = −½c0 + (ρ/σ_v)·κ·c0 − (ρ/σ_v)·κθΔt,
        //      B = −½c1 + (ρ/σ_v)(κc1 − 1),
        //      A_pre = −½c2 + (ρ/σ_v)(κc2 + 1).
        //
        // Plain QE replaces (C + B·v_t) by the analytic Itô compensation,
        // which is only asymptotically a martingale and drifts at high
        // σ_v/|ρ|. Andersen's K0* makes the step exactly martingale:
        //
        //   K0* = −ln M(A) − ½(1−ρ²)(c0 + c1·v_t),
        //   A   = A_pre + ½(1−ρ²)c2,
        //   M(A) = E[exp(A·v_{t+Δt}) | v_t]   (closed form per QE regime)
        //
        // so that E[S_{t+Δt} | S_t, v_t] = S_t·e^{(r−q)Δt} holds exactly.
        // When M(A) is not finite (A outside the regime's domain) or σ_v is
        // degenerate, the analytic drift is used as a fallback.
        let rho = params.rho.clamp(-1.0, 1.0);
        let (c0, c1, c2) = self.int_var_coeffs(dt, params.kappa, params.theta);
        let int_var = (c0 + c1 * v_t + c2 * v_next).max(0.0);
        let one_minus_rho2 = (1.0 - rho * rho).max(0.0);
        let orthogonal_diffusion = one_minus_rho2.sqrt() * int_var.sqrt() * z[0];

        let log_increment = if params.sigma_v.abs() > 1e-10 {
            let rho_over_sigma = rho / params.sigma_v;
            let a_pre = -0.5 * c2 + rho_over_sigma * (params.kappa * c2 + 1.0);
            let a_coeff = a_pre + 0.5 * one_minus_rho2 * c2;
            match regime.exp_moment(a_coeff) {
                Some(mgf) if mgf > 0.0 => {
                    // Martingale-exact K0* path.
                    let k0_star = -mgf.ln() - 0.5 * one_minus_rho2 * (c0 + c1 * v_t);
                    (params.r - params.q) * dt + k0_star + a_pre * v_next + orthogonal_diffusion
                }
                _ => {
                    // Fallback: analytic Itô-compensated drift (plain QE).
                    let drift = (params.r - params.q) * dt - 0.5 * int_var;
                    let variance_correction = rho_over_sigma
                        * (v_next - v_t - params.kappa * params.theta * dt
                            + params.kappa * int_var);
                    drift + variance_correction + orthogonal_diffusion
                }
            }
        } else {
            (params.r - params.q) * dt - 0.5 * int_var + orthogonal_diffusion
        };

        let s_next = s_t * log_increment.exp();

        // Update state
        x[0] = s_next;
        x[1] = v_next;
    }

    fn work_size(&self, _process: &HestonProcess) -> usize {
        0
    }

    fn applies_correlation_internally(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::process::heston::HestonParams;
    use super::super::super::process::heston::HestonProcess;
    use super::*;

    #[test]
    fn test_qe_heston_variance_positive() {
        let qe = QeHeston::new();
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        // Test with various shocks
        for z in [-3.0, -1.0, 0.0, 1.0, 3.0] {
            let v_next =
                qe.step_variance(0.04, params.kappa, params.theta, params.sigma_v, 0.01, z);
            assert!(v_next >= 0.0, "Variance became negative with z={}", z);
        }
    }

    #[test]
    fn test_qe_heston_mean_reversion() {
        let qe = QeHeston::new();
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.1, -0.5, 0.04).expect("valid");

        // Starting above theta
        let v_high = qe.step_variance(0.08, params.kappa, params.theta, params.sigma_v, 0.1, 0.0);
        assert!(v_high < 0.08, "Should mean-revert toward theta");

        // Starting below theta
        let v_low = qe.step_variance(0.02, params.kappa, params.theta, params.sigma_v, 0.1, 0.0);
        assert!(v_low > 0.02, "Should mean-revert toward theta");
    }

    #[test]
    fn test_qe_heston_step() {
        let heston =
            HestonProcess::with_params(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let qe = QeHeston::new();

        let mut x = vec![100.0, 0.04];
        let z = vec![0.0, 0.0]; // No shocks
        let mut work = vec![];

        qe.step(&heston, 0.0, 0.01, &mut x, &z, &mut work);

        // Spot and variance should be positive
        assert!(x[0] > 0.0);
        assert!(x[1] >= 0.0);

        // With zero shocks and v=theta, variance should stay near theta
        assert!((x[1] - 0.04).abs() < 0.01);
    }

    #[test]
    fn test_qe_heston_correlated_shocks() {
        let heston =
            HestonProcess::with_params(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let qe = QeHeston::new();

        // Negative variance shock should (with neg correlation) give positive spot shock
        let mut x1 = vec![100.0, 0.04];
        let z_neg_var = vec![0.0, -1.0]; // Negative variance shock
        let mut work = vec![];

        qe.step(&heston, 0.0, 0.01, &mut x1, &z_neg_var, &mut work);

        // With ρ=-0.7, negative variance shock gives positive contribution to spot
        // This is captured in the correlation structure
        assert!(x1[0] > 0.0);
        assert!(x1[1] >= 0.0);
    }

    #[test]
    fn test_integrated_variance_bounds() {
        let qe = QeHeston::new();
        let v_t = 0.04;
        let v_next = 0.05;
        let dt = 0.1;
        let kappa = 2.0;
        let theta = 0.04;

        let int_var = qe.integrated_variance(v_t, v_next, dt, kappa, theta);

        // Integrated variance should be between v_t * dt and v_next * dt
        let lower = v_t.min(v_next) * dt;
        let upper = v_t.max(v_next) * dt;
        assert!(
            int_var >= lower && int_var <= upper,
            "Integrated variance {} out of bounds [{}, {}]",
            int_var,
            lower,
            upper
        );

        // Should equal the midpoint
        let midpoint = (v_t + v_next) / 2.0 * dt;
        assert!(
            (int_var - midpoint).abs() < 1e-12,
            "Integrated variance should equal midpoint: got {} vs {}",
            int_var,
            midpoint
        );
    }

    #[test]
    fn test_integrated_variance_symmetric() {
        // When v_t == v_next, result should equal v * dt
        let qe = QeHeston::new();
        let v = 0.04;
        let dt = 0.1;
        let kappa = 2.0;
        let theta = 0.04;

        let int_var = qe.integrated_variance(v, v, dt, kappa, theta);
        let expected = v * dt;

        assert!(
            (int_var - expected).abs() < 1e-12,
            "When v_t == v_next, integrated variance should equal v*dt: {} vs {}",
            int_var,
            expected
        );
    }

    #[test]
    fn test_integrated_variance_various_dt() {
        // Test with various time steps
        let qe = QeHeston::new();
        let v_t = 0.04;
        let v_next = 0.05;
        let kappa = 2.0;
        let theta = 0.04;

        for dt in [0.001, 0.01, 0.1, 0.25, 1.0] {
            let int_var = qe.integrated_variance(v_t, v_next, dt, kappa, theta);
            let midpoint = (v_t + v_next) / 2.0 * dt;

            assert!(
                (int_var - midpoint).abs() < 1e-12,
                "Integrated variance should equal midpoint for dt={}: got {} vs {}",
                dt,
                int_var,
                midpoint
            );
        }
    }

    #[test]
    fn test_exact_integrated_variance() {
        // Test exact method vs trapezoidal
        let qe_trap = QeHeston::new();
        let qe_exact = QeHeston::exact_variance();

        let v_t = 0.04;
        let v_next = 0.06;
        let dt = 0.25;
        let kappa = 5.0; // High mean reversion
        let theta = 0.04;

        let trap = qe_trap.integrated_variance(v_t, v_next, dt, kappa, theta);
        let exact = qe_exact.integrated_variance(v_t, v_next, dt, kappa, theta);

        // Both should be positive
        assert!(trap > 0.0);
        assert!(exact > 0.0);

        // Exact should differ from trapezoidal for high kappa
        // (they're not equal but both reasonable)
        let diff_pct = ((exact - trap) / trap).abs() * 100.0;
        assert!(
            diff_pct < 20.0,
            "Exact and trapezoidal should be within 20%: {} vs {}, diff={}%",
            exact,
            trap,
            diff_pct
        );
    }

    #[test]
    fn test_exact_variance_converges_for_small_kappa() {
        // For κ ≈ 0, exact should fall back to trapezoidal
        let qe_exact = QeHeston::exact_variance();

        let v_t = 0.04;
        let v_next = 0.05;
        let dt = 0.1;
        let kappa = 1e-12; // Effectively zero
        let theta = 0.04;

        let int_var = qe_exact.integrated_variance(v_t, v_next, dt, kappa, theta);
        let trap = (v_t + v_next) / 2.0 * dt;

        assert!(
            (int_var - trap).abs() < 1e-10,
            "Exact should match trapezoidal for κ≈0: {} vs {}",
            int_var,
            trap
        );
    }

    #[test]
    fn test_builder_pattern() {
        // Test that builder pattern works for configuring QE scheme
        let qe = QeHeston::new()
            .with_integrated_variance(IntegratedVarianceMethod::MeanReversionAdjusted);

        // Verify it works without panics
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let v = qe.step_variance(0.04, params.kappa, params.theta, params.sigma_v, 0.1, 0.0);
        assert!(v >= 0.0);
    }

    #[test]
    fn test_with_psi_c() {
        // Test custom psi_c threshold
        let qe = QeHeston::with_psi_c(2.0);
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        // Variance should remain positive
        for z in [-2.0, 0.0, 2.0] {
            let v = qe.step_variance(0.04, params.kappa, params.theta, params.sigma_v, 0.1, z);
            assert!(v >= 0.0);
        }
    }

    #[test]
    fn test_qe_heston_spot_update_uses_k0_star_correction() {
        let heston =
            HestonProcess::with_params(0.03, 0.01, 1.7, 0.04, 0.6, -0.4, 0.05).expect("valid");
        let qe = QeHeston::new();
        let dt = 0.1;
        let mut x = vec![100.0, 0.05];
        let z = vec![0.3, -0.2];
        let mut work = vec![];

        // Reconstruct the expected Andersen §4.2 K0* update independently.
        let params = heston.params();
        let s_t = x[0];
        let v_t = x[1];
        let regime = qe_regime(v_t, params.kappa, params.theta, params.sigma_v, dt, 1.5);
        let v_next = regime.sample(z[1]);
        let (c0, c1, c2) = (0.0, 0.5 * dt, 0.5 * dt); // trapezoidal γ1=γ2=½
        let int_var = c0 + c1 * v_t + c2 * v_next;
        let rho = params.rho;
        let one_minus_rho2 = 1.0 - rho * rho;
        let a_pre = -0.5 * c2 + rho / params.sigma_v * (params.kappa * c2 + 1.0);
        let a_coeff = a_pre + 0.5 * one_minus_rho2 * c2;
        let mgf = regime.exp_moment(a_coeff).expect("finite MGF");
        let k0_star = -mgf.ln() - 0.5 * one_minus_rho2 * (c0 + c1 * v_t);
        let expected_log_return = (params.r - params.q) * dt
            + k0_star
            + a_pre * v_next
            + one_minus_rho2.sqrt() * int_var.sqrt() * z[0];
        let expected_spot = s_t * expected_log_return.exp();

        qe.step(&heston, 0.0, dt, &mut x, &z, &mut work);

        assert!(
            (x[0] - expected_spot).abs() < 1e-12,
            "expected spot {} but got {}",
            expected_spot,
            x[0]
        );
    }

    /// One-step exact martingale property of the K0* correction: for a fixed
    /// `v_t`, `E[S_{t+Δt}/S_t] = e^{(r−q)Δt}` holds exactly (up to MC noise)
    /// even at a coarse Δt with high σ_v and strong ρ — the regime where the
    /// plain Itô-compensated QE drift is visibly biased.
    #[test]
    fn k0_star_one_step_martingale() {
        use crate::rng::philox::PhiloxRng;
        use crate::traits::RandomStream;

        let heston =
            HestonProcess::with_params(0.05, 0.0, 0.5, 0.04, 1.0, -0.9, 0.04).expect("valid");
        let qe = QeHeston::new();
        let dt = 0.25; // coarse quarterly step
        let params = heston.params();

        let mut rng = PhiloxRng::new(0xA11D_0001);
        let n = 400_000usize;
        let mut zs = vec![0.0; 2 * n];
        rng.fill_std_normals(&mut zs);

        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        let mut work = vec![];
        for i in 0..n {
            let mut x = vec![1.0, 0.04];
            let z = [zs[2 * i], zs[2 * i + 1]];
            qe.step(&heston, 0.0, dt, &mut x, &z, &mut work);
            sum += x[0];
            sum_sq += x[0] * x[0];
        }
        let npf = n as f64;
        let mean = sum / npf;
        let var = (sum_sq / npf - mean * mean).max(0.0);
        let se = (var / npf).sqrt();
        let target = ((params.r - params.q) * dt).exp();
        assert!(
            (mean - target).abs() < 4.0 * se,
            "one-step E[S]={mean} must equal {target} within 4·SE={:.2e}",
            4.0 * se
        );
    }

    /// Multi-step martingale test (Andersen §4.2): `E[S_T] = S_0·e^{(r−q)T}`
    /// within MC error at high σ_v and strong negative ρ, where plain QE
    /// (analytic drift, no K0*) drifts measurably.
    #[test]
    fn k0_star_multi_step_martingale_at_high_vol_of_vol() {
        use crate::rng::philox::PhiloxRng;
        use crate::traits::RandomStream;

        let r = 0.05;
        let heston = HestonProcess::with_params(r, 0.0, 0.5, 0.04, 1.0, -0.9, 0.04).expect("valid");
        let qe = QeHeston::new();
        let t_end = 1.0;
        let n_steps = 12usize; // coarse monthly grid
        let dt = t_end / n_steps as f64;

        let mut rng = PhiloxRng::new(0xA11D_0002);
        let n_paths = 200_000usize;
        let mut zs = vec![0.0; 2 * n_steps];

        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        let mut work = vec![];
        for _ in 0..n_paths {
            rng.fill_std_normals(&mut zs);
            let mut x = vec![1.0, 0.04];
            for k in 0..n_steps {
                let z = [zs[2 * k], zs[2 * k + 1]];
                qe.step(&heston, k as f64 * dt, dt, &mut x, &z, &mut work);
            }
            sum += x[0];
            sum_sq += x[0] * x[0];
        }
        let npf = n_paths as f64;
        let mean = sum / npf;
        let var = (sum_sq / npf - mean * mean).max(0.0);
        let se = (var / npf).sqrt();
        let target = (r * t_end).exp();
        assert!(
            (mean - target).abs() < 4.0 * se,
            "E[S_T]={mean} must equal S_0·e^{{rT}}={target} within 4·SE={:.2e}",
            4.0 * se
        );
    }

    #[test]
    fn test_qe_heston_clamps_rho_and_integrated_variance_before_sqrt() {
        let heston = HestonProcess::new(HestonParams {
            r: 0.03,
            q: 0.01,
            kappa: 1.5,
            theta: 0.04,
            sigma_v: 1.0e-16,
            rho: 1.0 + 1.0e-12,
            v0: 0.04,
        });
        let qe = QeHeston::new();
        let mut x = vec![100.0, 0.04];
        let z = vec![0.2, -0.1];
        let mut work = vec![];

        qe.step(&heston, 0.0, 0.25, &mut x, &z, &mut work);

        assert!(x[0].is_finite(), "spot update should stay finite");
        assert!(x[1].is_finite(), "variance update should stay finite");
        assert!(x[0] > 0.0, "spot should remain positive");
        assert!(x[1] >= 0.0, "variance should remain non-negative");
    }
}
