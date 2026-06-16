//! Jump-augmented Euler scheme for jump-diffusion processes.
//!
//! Combines continuous diffusion (Euler) with discrete jump arrivals (Poisson).
//!
//! # Algorithm
//!
//! For Merton model over [t, t+Δt]:
//!
//! 1. Apply GBM diffusion step (continuous part)
//! 2. Sample number of jumps N ~ Poisson(λΔt)
//! 3. Conditional on N, sample the aggregate log jump as Nμ_J + √N σ_J Z
//! 4. Multiply spot by jump product: S *= ∏J_i

use super::super::process::jump_diffusion::MertonJumpProcess;
use super::super::rng::poisson::poisson_from_normal;
use super::super::traits::Discretization;

/// Jump-augmented Euler for Merton jump-diffusion.
///
/// Handles both continuous diffusion and discrete jumps.
///
/// # Factor Usage
///
/// - `z[0]`: Continuous diffusion shock
/// - `z[1]`: Poisson number of jumps (via CDF transform)
/// - `z[2]`: Aggregate jump-size shock conditional on the sampled jump count
///
/// The aggregate conditional-normal draw avoids a fixed per-step jump cap.
#[derive(Debug, Clone)]
pub struct JumpEuler;

impl Default for JumpEuler {
    fn default() -> Self {
        Self::new()
    }
}

impl JumpEuler {
    /// Create a new jump-Euler discretization.
    pub fn new() -> Self {
        Self
    }

    /// Create with a legacy jump-budget argument.
    ///
    /// The current scheme samples the aggregate conditional log jump and does
    /// not truncate jumps, so the argument is retained only for source
    /// compatibility.
    pub fn with_max_jumps(_max_jumps: usize) -> Self {
        Self::new()
    }
}

impl Discretization<MertonJumpProcess> for JumpEuler {
    fn step(
        &self,
        process: &MertonJumpProcess,
        _t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        _work: &mut [f64],
    ) {
        let params = process.params();
        let s_t = x[0];

        // Step 1: Continuous diffusion (log-Euler for positivity)
        // dS/S = (r - q - λk)dt + σ dW
        let drift_rate = params.compensated_drift();
        let sigma = params.gbm.sigma;

        let log_drift = (drift_rate - 0.5 * sigma * sigma) * dt;
        let log_diffusion = sigma * dt.sqrt() * z[0];

        let s_diffusion = s_t * (log_drift + log_diffusion).exp();

        // Step 2: Sample number of jumps
        let lambda_dt = params.lambda * dt;
        let num_jumps = if lambda_dt > 1e-10 {
            // Use z[1] to generate Poisson
            poisson_from_normal(lambda_dt, z[1])
        } else {
            0
        };

        // Step 3: Apply jumps. Conditional on N, the sum of N independent
        // normal log-jump sizes is Normal(N * μ_J, N * σ_J²).
        let mut jump_product = 1.0;
        if num_jumps > 0 {
            let n = num_jumps as f64;
            let z_jump = z.get(2).copied().unwrap_or(0.0);
            let log_jump_sum = n * params.mu_j + n.sqrt() * params.sigma_j * z_jump;
            jump_product = log_jump_sum.exp();
        }

        // Step 4: Combine diffusion and jumps
        x[0] = s_diffusion * jump_product;
    }

    fn work_size(&self, _process: &MertonJumpProcess) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::engine::McEngine;
    use super::super::super::payoff::vanilla::Forward;
    use super::super::super::process::jump_diffusion::{MertonJumpParams, MertonJumpProcess};
    use super::super::super::traits::RandomStream;
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn test_jump_euler_no_jumps() {
        // With lambda = 0, should behave like log-Euler
        let params = MertonJumpParams::new(0.05, 0.02, 0.2, 0.0, 0.0, 0.0).unwrap();
        let process = MertonJumpProcess::new(params);
        let disc = JumpEuler::new();

        let t: f64 = 0.0;
        let dt: f64 = 0.01;
        let mut x = vec![100.0];
        let z = vec![0.5, 0.0]; // Diffusion shock, no jumps
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // Should be positive and above initial (positive drift + shock)
        assert!(x[0] > 0.0);
        assert!(x[0] > 100.0);
    }

    #[test]
    fn test_jump_euler_positive_jumps() {
        // Test with positive jumps
        let params = MertonJumpParams::new(
            0.05, 0.02, 0.15, 5.0,  // lambda (expect ~5 jumps per year)
            0.02, // mu_j (small positive jumps)
            0.05, // sigma_j
        )
        .expect("valid Merton params");
        let process = MertonJumpProcess::new(params);
        let disc = JumpEuler::new();

        let t: f64 = 0.0;
        let dt: f64 = 0.1; // 10% of year → expect ~0.5 jumps
        let mut x = vec![100.0];

        // Force a jump with high Poisson draw
        let z = vec![0.0, 2.0, 1.0]; // diffusion=0, Poisson=high, jump_size=+1σ
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // Should still be positive
        assert!(x[0] > 0.0);
    }

    #[test]
    fn test_jump_euler_negative_jumps() {
        // Test with negative jumps (crashes)
        let params = MertonJumpParams::new(
            0.05, 0.02, 0.2, 1.0,  // lambda
            -0.1, // mu_j (negative jumps)
            0.15, // sigma_j
        )
        .expect("valid Merton params");
        let process = MertonJumpProcess::new(params);
        let disc = JumpEuler::new();

        let t: f64 = 0.0;
        let dt: f64 = 0.05;
        let mut x = vec![100.0];

        // No diffusion shock, potential jump
        let z = vec![0.0, 0.0, -1.0]; // jump size = mu_j - sigma_j
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // Should maintain positivity even with negative jumps
        assert!(x[0] > 0.0);
    }

    #[test]
    fn test_jump_compensation() {
        // Test that compensated drift makes sense
        let params = MertonJumpParams::new(
            0.05, 0.02, 0.2, 2.0,  // 2 jumps/year
            0.05, // positive mu_j
            0.1,
        )
        .expect("valid Merton params");

        let drift = params.compensated_drift();
        let pure_gbm_drift = params.gbm.r - params.gbm.q;

        // Compensated drift should be less than GBM drift (removed jump comp)
        // Since jumps are positive on average, we subtract
        assert!(drift < pure_gbm_drift);

        let k = params.jump_compensation();
        assert!(k > 0.0); // Positive jumps
        assert_eq!(drift, pure_gbm_drift - params.lambda * k);
    }

    #[derive(Clone)]
    struct DeterministicJumpRng;

    impl RandomStream for DeterministicJumpRng {
        fn split(&self, _id: u64) -> Option<Self> {
            Some(Self)
        }

        fn fill_u01(&mut self, out: &mut [f64]) {
            for x in out {
                *x = 0.5;
            }
        }

        fn fill_std_normals(&mut self, out: &mut [f64]) {
            for x in out.iter_mut() {
                *x = 0.0;
            }

            if !out.is_empty() {
                out[0] = 0.0;
            }
            if out.len() >= 2 {
                out[1] = 8.0;
            }
            if out.len() >= 3 {
                out[2] = 1.0;
            }
        }
    }

    #[test]
    fn test_engine_supplies_jump_size_random_factors() {
        let params = MertonJumpParams::new(0.0, 0.0, 0.0, 50.0, 0.0, 1.0).unwrap();
        let compensated_drift = params.compensated_drift();
        let process = MertonJumpProcess::new(params);
        let disc = JumpEuler::new();
        let engine = McEngine::builder()
            .num_paths(1)
            .uniform_grid(0.1, 1)
            .parallel(false)
            .build()
            .expect("engine should build");
        let payoff = Forward::long(0.0, 1.0, 1);

        let result = engine
            .price(
                &DeterministicJumpRng,
                &process,
                &disc,
                &[100.0],
                &payoff,
                Currency::USD,
                1.0,
            )
            .expect("pricing should succeed");

        let num_jumps = poisson_from_normal(50.0 * 0.1, 8.0) as f64;
        let aggregate_log_jump = num_jumps.sqrt();
        let expected_terminal_spot =
            100.0 * (compensated_drift * 0.1).exp() * aggregate_log_jump.exp();
        assert!(
            (result.mean.amount() - expected_terminal_spot).abs() < 1e-12,
            "expected terminal spot {} but got {}",
            expected_terminal_spot,
            result.mean.amount()
        );
    }

    #[test]
    fn test_jump_euler_uses_all_sampled_jumps() {
        let params = MertonJumpParams::new(0.0, 0.0, 0.0, 50.0, 0.01, 0.0).unwrap();
        let compensated_drift = params.compensated_drift();
        let process = MertonJumpProcess::new(params);
        let disc = JumpEuler::new();
        let mut x = vec![100.0];
        let mut work = vec![0.0; disc.work_size(&process)];

        // lambda * dt = 50 and z = 0 maps to the Poisson median, which the
        // large-lambda approximation returns as exactly 50 jumps. The old
        // implementation capped this at ten jump-size shocks.
        disc.step(&process, 0.0, 1.0, &mut x, &[0.0, 0.0, 0.0], &mut work);

        let expected = 100.0 * compensated_drift.exp() * (50.0_f64 * 0.01).exp();
        assert!(
            (x[0] - expected).abs() < 1e-12,
            "expected all sampled jumps to be applied: got {}, expected {}",
            x[0],
            expected
        );
    }
}
