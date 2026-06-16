//! Quadratic-Exponential scheme for the Bates model (Heston + Merton jumps).
//!
//! Composes the three building blocks the Bates dynamics require:
//!
//! 1. **Variance leg** — Andersen's QE scheme (Case A / Case B switch with
//!    safeguards), via the shared [`super::qe_common`] implementation inside
//!    the embedded [`QeHeston`] step.
//! 2. **Diffusive spot leg** — the martingale-exact `K0*` log update from
//!    [`QeHeston`] (Andersen 2008, §4.2), with the spot–variance correlation
//!    ρ embedded through the Broadie–Kaya identity. The jump compensator
//!    `λk` enters here **exactly once**, absorbed into the leg's rate
//!    (`r_eff = r − λk`) by [`BatesProcess::new`].
//! 3. **Jump leg** — the per-step Poisson/lognormal sampling pattern of
//!    [`super::jump_euler::JumpEuler`]: `N ~ Poisson(λΔt)` via the inverse
//!    CDF of one normal factor, then the aggregate log jump
//!    `Normal(N·μ_J, N·σ_J²)` via another.
//!
//! # Martingale property
//!
//! The diffusive leg satisfies `E[S^{diff}_{t+Δt} | S_t, v_t] =
//! S_t·e^{(r−q−λk)Δt}` exactly (K0*), and the independent jump factor has
//! `E[∏J] = e^{λkΔt}`, so the combined step reproduces the risk-neutral
//! forward: `E[S_{t+Δt} | S_t, v_t] = S_t·e^{(r−q)Δt}`.
//!
//! # Factor Usage
//!
//! - `z[0]`: spot diffusion shock (orthogonal component of the BK identity)
//! - `z[1]`: variance shock (QE draw)
//! - `z[2]`: Poisson jump-count shock (via CDF transform)
//! - `z[3]`: aggregate jump-size shock conditional on the sampled count
//!
//! # References
//!
//! - Andersen, L. (2008). "Simple and efficient simulation of the Heston
//!   stochastic volatility model." *J. Computational Finance*, 11(3).
//! - Bates, D. S. (1996). "Jumps and Stochastic Volatility." *Review of
//!   Financial Studies*, 9(1), 69–107.

use super::super::process::bates::BatesProcess;
use super::super::rng::poisson::poisson_from_normal;
use super::super::traits::Discretization;
use super::qe_heston::{IntegratedVarianceMethod, QeHeston};

/// QE discretization for the Bates model.
///
/// This is the only scheme that applies the Bates jump leg; see the module
/// documentation of [`crate::process::bates`] for why the generic Euler
/// schemes must not be paired with [`BatesProcess`].
#[derive(Debug, Clone)]
pub struct QeBates {
    /// Embedded Heston QE leg (variance + martingale-corrected spot).
    heston_leg: QeHeston,
}

impl QeBates {
    /// Create a new QE Bates discretization with default settings
    /// (ψ_c = 1.5, trapezoidal integrated variance).
    pub fn new() -> Self {
        Self {
            heston_leg: QeHeston::new(),
        }
    }

    /// Create with custom ψ_c threshold for the variance leg.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when `psi_c` is not in
    /// \[1, 2\] — see [`QeHeston::with_psi_c`].
    pub fn with_psi_c(psi_c: f64) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            heston_leg: QeHeston::with_psi_c(psi_c)?,
        })
    }

    /// Set the integrated-variance method of the diffusive leg.
    #[must_use]
    pub fn with_integrated_variance(mut self, method: IntegratedVarianceMethod) -> Self {
        self.heston_leg = self.heston_leg.with_integrated_variance(method);
        self
    }
}

impl Default for QeBates {
    fn default() -> Self {
        Self::new()
    }
}

impl Discretization<BatesProcess> for QeBates {
    fn step(
        &self,
        process: &BatesProcess,
        t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        work: &mut [f64],
    ) {
        // Step 1+2: diffusive Heston leg (QE variance + K0* spot) with the
        // jump compensator absorbed into the leg's rate. QeHeston reads
        // z[0..2] only.
        self.heston_leg
            .step(process.compensated_heston(), t, dt, x, z, work);

        // Step 3: jumps, independent of the diffusive shocks. Conditional on
        // N ~ Poisson(λΔt), the sum of N lognormal jumps has log
        // Normal(N·μ_J, N·σ_J²).
        let jump = &process.params().jump;
        let lambda_dt = jump.lambda * dt;
        if lambda_dt > 1e-10 {
            let num_jumps = poisson_from_normal(lambda_dt, z[2]);
            if num_jumps > 0 {
                let n = num_jumps as f64;
                let log_jump_sum = n * jump.mu_j + n.sqrt() * jump.sigma_j * z[3];
                x[0] *= log_jump_sum.exp();
            }
        }
    }

    fn work_size(&self, _process: &BatesProcess) -> usize {
        0
    }

    fn applies_correlation_internally(&self) -> bool {
        // ρ is embedded via the BK identity inside the Heston leg; the jump
        // factors are independent by construction.
        true
    }

    fn scheme_id(&self) -> &'static str {
        // Satisfies `BatesProcess::dedicated_scheme`.
        "qe_bates"
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::engine::McEngine;
    use super::super::super::payoff::vanilla::Forward;
    use super::super::super::process::bates::{BatesParams, BatesProcess};
    use super::super::super::process::heston::{HestonParams, HestonProcess};
    use super::super::super::process::jump_diffusion::MertonJumpParams;
    use super::super::super::rng::philox::PhiloxRng;
    use super::super::qe_heston::QeHeston;
    use super::*;
    use finstack_quant_core::currency::Currency;

    fn bates_process(lambda: f64, mu_j: f64, sigma_j: f64) -> BatesProcess {
        let heston = HestonParams::new(0.05, 0.01, 1.7, 0.04, 0.6, -0.4, 0.05).expect("valid");
        let jump = MertonJumpParams::new(0.05, 0.01, 0.0, lambda, mu_j, sigma_j).expect("valid");
        BatesProcess::new(BatesParams::new(heston, jump).expect("matching r/q"))
    }

    /// With λ = 0 the compensator vanishes and QeBates must reproduce the
    /// plain QeHeston step bit-for-bit on the same shocks.
    #[test]
    fn zero_intensity_degenerates_to_qe_heston() {
        let bates = bates_process(0.0, 0.0, 0.1);
        let heston =
            HestonProcess::with_params(0.05, 0.01, 1.7, 0.04, 0.6, -0.4, 0.05).expect("valid");

        let qe_bates = QeBates::new();
        let qe_heston = QeHeston::new();

        for z_pair in [[-1.3, 0.7], [0.0, 0.0], [2.1, -0.4]] {
            let mut x_bates = [100.0, 0.05];
            let mut x_heston = [100.0, 0.05];
            let z_bates = [z_pair[0], z_pair[1], 1.5, -0.8]; // jump shocks unused at λ=0
            let mut work = [];

            qe_bates.step(&bates, 0.0, 0.1, &mut x_bates, &z_bates, &mut work);
            qe_heston.step(&heston, 0.0, 0.1, &mut x_heston, &z_pair, &mut work);

            assert_eq!(x_bates[0].to_bits(), x_heston[0].to_bits());
            assert_eq!(x_bates[1].to_bits(), x_heston[1].to_bits());
        }
    }

    /// A forced jump multiplies the spot by exactly
    /// `exp(N·μ_J + √N·σ_J·z[3])` on top of the diffusive leg.
    #[test]
    fn forced_jump_applies_aggregate_lognormal_factor() {
        let lambda = 2.0;
        let (mu_j, sigma_j) = (-0.05, 0.15);
        let bates = bates_process(lambda, mu_j, sigma_j);
        let qe = QeBates::new();
        let dt = 0.25;

        // Same diffusive shocks; one run with a high Poisson draw, one with
        // a deeply negative draw (zero jumps).
        let z_no_jump = [0.4, -0.6, -8.0, 1.0];
        let z_jump = [0.4, -0.6, 3.0, 1.0];

        let mut x_no = [100.0, 0.05];
        let mut x_jump = [100.0, 0.05];
        let mut work = [];
        qe.step(&bates, 0.0, dt, &mut x_no, &z_no_jump, &mut work);
        qe.step(&bates, 0.0, dt, &mut x_jump, &z_jump, &mut work);

        let n = poisson_from_normal(lambda * dt, 3.0);
        assert!(n > 0, "test setup must force at least one jump");
        let nf = n as f64;
        let expected_factor = (nf * mu_j + nf.sqrt() * sigma_j * 1.0).exp();

        assert!((x_jump[0] / x_no[0] - expected_factor).abs() < 1e-12);
        // Variance leg is jump-independent.
        assert_eq!(x_jump[1].to_bits(), x_no[1].to_bits());
    }

    /// Engine-level martingale test: with active jumps, stochastic vol, and
    /// nonzero correlation, `E[S_T] = S₀·e^{(r−q)T}` must hold within the
    /// Monte Carlo standard error. This is the test whose absence let the
    /// original Bates wiring (compensated drift, no jumps) ship (quant
    /// .
    #[test]
    fn qe_bates_terminal_spot_is_martingale() {
        let bates = bates_process(1.0, -0.05, 0.15);
        let qe = QeBates::new();

        let s0 = 100.0;
        let t = 1.0;
        let engine = McEngine::builder()
            .num_paths(100_000)
            .uniform_grid(t, 12)
            .parallel(false)
            .build()
            .expect("engine should build");

        // Forward::long(0, 1, n) pays exactly S_T.
        let payoff = Forward::long(0.0, 1.0, 12);
        let rng = PhiloxRng::new(20_260_612);

        let result = engine
            .price(&rng, &bates, &qe, &[s0, 0.05], &payoff, Currency::USD, 1.0)
            .expect("pricing should succeed");

        let forward = s0 * ((0.05 - 0.01) * t).exp();
        let mean = result.mean.amount();
        let tol = 4.0 * result.stderr;
        assert!(
            (mean - forward).abs() < tol,
            "E[S_T] = {mean:.4} should match the forward {forward:.4} within 4·stderr = {tol:.4}"
        );
    }
}
