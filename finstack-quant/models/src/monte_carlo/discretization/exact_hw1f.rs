//! Exact discretization for Hull-White 1-factor model.
//!
//! Provides analytical solution for the OU/HW1F SDE with piecewise-constant θ(t).
//!
//! # SDE
//!
//! ```text
//! dr_t = κ[θ(t) - r_t]dt + σ dW_t
//! ```
//!
//! # Exact Solution
//!
//! For θ constant over [t, t+Δt]:
//!
//! ```text
//! r_{t+Δt} = r_t e^{-κΔt} + θ(1 - e^{-κΔt}) + σ√[(1-e^{-2κΔt})/(2κ)] Z
//! ```
//!
//! where Z ~ N(0, 1).
//!
//! When a simulation step straddles θ knots, each constant segment contributes
//! its exact exponentially weighted integral to the conditional mean.

use super::super::process::ou::HullWhite1FProcess;
use super::super::traits::Discretization;

/// Exact discretization for Hull-White 1-factor.
///
/// Uses the analytical solution to the OU SDE, providing zero discretization
/// error for the conditional distribution.
///
/// # Advantages
///
/// - Exact (no approximation error)
/// - Unconditionally stable (any step size)
/// - Fast evaluation (no iterations)
///
/// # Formula
///
/// ```text
/// r_{t+Δt} = E[r_{t+Δt}|r_t] + Std[r_{t+Δt}|r_t] * Z
///
/// where:
///   E[r_{t+Δt}|r_t] = r_t e^{-κΔt} + θ(1 - e^{-κΔt})
///   Std[r_{t+Δt}|r_t] = σ√[(1 - e^{-2κΔt}) / (2κ)]
/// ```
#[derive(Debug, Clone, Default)]
pub struct ExactHullWhite1F {
    /// Per-run cache of the `dt`-dependent step constants, populated by
    /// [`Discretization::prepare`]. `None` until the engine prepares the scheme
    /// (e.g. when stepped directly without the engine), in which case the
    /// constants are computed inline.
    prepared: Option<Hw1fStepConstants>,
}

/// Path-independent HW1F step constants for a fixed step size.
///
/// `exp_kappa_dt = e^{-κΔt}` and `std_dev = σ·√[(1−e^{−2κΔt})/(2κ)]` depend only
/// on `(κ, σ, Δt)`, so they are identical on every step of a uniform grid and
/// across every path.
#[derive(Debug, Clone, Copy)]
struct Hw1fStepConstants {
    dt: f64,
    exp_kappa_dt: f64,
    std_dev: f64,
}

impl Hw1fStepConstants {
    /// Compute the constants for one step size. Mirrors the arithmetic in
    /// [`ExactHullWhite1F::step`] exactly so cached and inline paths are
    /// bit-identical.
    #[inline]
    fn compute(kappa: f64, sigma: f64, dt: f64) -> Self {
        let exp_kappa_dt = (-kappa * dt).exp();
        let std_dev = if (kappa * dt).abs() < 1e-8 {
            sigma * dt.sqrt() * (1.0 - kappa * dt / 2.0)
        } else {
            sigma * ((1.0 - (-2.0 * kappa * dt).exp()) / (2.0 * kappa)).sqrt()
        };
        Self {
            dt,
            exp_kappa_dt,
            std_dev,
        }
    }
}

impl ExactHullWhite1F {
    /// Create a new exact HW1F discretization.
    pub fn new() -> Self {
        Self { prepared: None }
    }
}

impl Discretization<HullWhite1FProcess> for ExactHullWhite1F {
    fn step(
        &self,
        process: &HullWhite1FProcess,
        t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        _work: &mut [f64],
    ) {
        let params = process.params();
        let theta_contribution = params.theta_mean_contribution(t, dt);

        // Reuse the precomputed `dt`-dependent constants when this step's `dt`
        // matches the prepared one (exact bit match → identical value); fall
        // back to inline computation for unprepared or non-uniform grids.
        let constant_sigma =
            (params.volatility.times().len() == 1).then(|| params.volatility.values()[0]);
        let consts = match (self.prepared, constant_sigma) {
            (Some(c), Some(_)) if c.dt.to_bits() == dt.to_bits() => c,
            (_, Some(sigma)) => Hw1fStepConstants::compute(params.kappa, sigma, dt),
            _ => Hw1fStepConstants {
                dt,
                exp_kappa_dt: (-params.kappa * dt).exp(),
                std_dev: params.sigma_variance_for_step(t, dt).max(0.0).sqrt(),
            },
        };

        // Conditional mean E[r_{t+Δt}|r_t] and exact step.
        let mean = x[0] * consts.exp_kappa_dt + theta_contribution;
        x[0] = mean + consts.std_dev * z[0];
    }

    fn prepare(&mut self, process: &HullWhite1FProcess, time_grid: &crate::monte_carlo::TimeGrid) {
        if time_grid.num_steps() == 0 {
            return;
        }
        let params = process.params();
        self.prepared = if params.volatility.times().len() == 1 {
            Some(Hw1fStepConstants::compute(
                params.kappa,
                params.volatility.values()[0],
                time_grid.dt(0),
            ))
        } else {
            None
        };
    }

    fn work_size(&self, _process: &HullWhite1FProcess) -> usize {
        0 // No workspace needed
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::process::ou::{HullWhite1FParams, HullWhite1FProcess};
    use super::*;

    #[test]
    fn test_exact_hw1f_mean_reversion() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03).expect("valid Hull-White parameters");
        let process = HullWhite1FProcess::new(params);
        let disc = ExactHullWhite1F::new();

        let t: f64 = 0.0;
        let dt: f64 = 1.0;
        let mut x = vec![0.05]; // Start above mean
        let z = vec![0.0]; // No shock
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // Should move toward mean (0.03)
        assert!(x[0] < 0.05);
        assert!(x[0] > 0.03);

        // Check exact formula
        let expected: f64 = 0.05 * (-0.1_f64 * 1.0).exp() + 0.03 * (1.0 - (-0.1_f64 * 1.0).exp());
        assert!((x[0] - expected).abs() < 1e-10);
    }

    #[test]
    fn test_exact_hw1f_positive_shock() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03).expect("valid Hull-White parameters");
        let process = HullWhite1FProcess::new(params);
        let disc = ExactHullWhite1F::new();

        let t: f64 = 0.0;
        let dt: f64 = 0.1;
        let mut x = vec![0.03]; // Start at mean
        let z = vec![2.0]; // Positive shock
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // With positive shock, rate should increase
        assert!(x[0] > 0.03);

        // Check that volatility term is applied correctly
        let std_dev: f64 = 0.01 * ((1.0 - (-2.0_f64 * 0.1 * 0.1).exp()) / (2.0 * 0.1)).sqrt();
        let expected_move = std_dev * 2.0; // 2 standard deviations

        // Rate should be approximately mean + 2*std
        assert!((x[0] - (0.03 + expected_move)).abs() < 0.001);
    }

    #[test]
    fn test_exact_hw1f_time_dependent_theta() {
        let theta_curve = vec![0.02, 0.04];
        let theta_times = vec![0.0, 0.5];

        let params =
            HullWhite1FParams::with_time_dependent_theta(0.1, 0.01, theta_curve, theta_times)
                .expect("valid Hull-White theta schedule");
        let process = HullWhite1FProcess::new(params);
        let disc = ExactHullWhite1F::new();

        // Step in first regime (θ = 0.02)
        let t1: f64 = 0.0;
        let dt: f64 = 0.1;
        let mut x1 = vec![0.03];
        let z = vec![0.0];
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t1, dt, &mut x1, &z, &mut work);

        let expected1: f64 = 0.03 * (-0.1_f64 * 0.1).exp() + 0.02 * (1.0 - (-0.1_f64 * 0.1).exp());
        assert!((x1[0] - expected1).abs() < 1e-10);

        // Step in second regime (θ = 0.04)
        let t2: f64 = 0.6;
        let mut x2 = vec![0.03];
        disc.step(&process, t2, dt, &mut x2, &z, &mut work);

        let expected2: f64 = 0.03 * (-0.1_f64 * 0.1).exp() + 0.04 * (1.0 - (-0.1_f64 * 0.1).exp());
        assert!((x2[0] - expected2).abs() < 1e-10);

        // Should move toward different means
        assert!(x2[0] > x1[0]);
    }

    /// A step straddling a θ knot must use the exponential OU weighting.
    #[test]
    fn test_exact_hw1f_theta_exponentially_weighted_across_knot() {
        let params = HullWhite1FParams::with_time_dependent_theta(
            1.0,
            0.01,
            vec![0.02, 0.04],
            vec![0.0, 0.5],
        )
        .expect("valid Hull-White theta schedule");
        let process = HullWhite1FProcess::new(params);
        let disc = ExactHullWhite1F::new();

        // Ordinary θ averaging would incorrectly leave the mean at 3%.
        let t: f64 = 0.0;
        let dt: f64 = 1.0;
        let mut x = vec![0.03];
        let z = vec![0.0];
        let mut work = vec![0.0; disc.work_size(&process)];
        disc.step(&process, t, dt, &mut x, &z, &mut work);

        let decay = (-0.5_f64).exp();
        let expected = (0.03 * decay + 0.02 * (1.0 - decay)) * decay + 0.04 * (1.0 - decay);
        assert!(
            (x[0] - expected).abs() < 1e-12,
            "step across θ knot should equal composition of exact steps: got {}, expected {expected}",
            x[0]
        );
        assert!((expected - 0.03154818121746176).abs() < 1e-14);
    }

    #[test]
    fn exact_hw1f_multi_knot_mean_equals_composed_exact_transitions() {
        for kappa in [1e-10, 0.1, 1.0, 20.0] {
            let params = HullWhite1FParams::with_time_dependent_theta(
                kappa,
                0.01,
                vec![-0.01, 0.06, 0.02, 0.04],
                vec![0.0, 0.2, 0.5, 0.9],
            )
            .expect("valid theta curve");
            let process = HullWhite1FProcess::new(params);
            let disc = ExactHullWhite1F::new();
            let mut single = [0.03];
            disc.step(&process, 0.1, 1.0, &mut single, &[0.0], &mut []);
            let mut composed = [0.03];
            for (start, end) in [(0.1, 0.2), (0.2, 0.5), (0.5, 0.9), (0.9, 1.1)] {
                disc.step(&process, start, end - start, &mut composed, &[0.0], &mut []);
            }
            assert!((single[0] - composed[0]).abs() < 1e-14, "κ={kappa}");
        }
    }

    #[test]
    fn exact_step_uses_integrated_piecewise_sigma_variance() {
        let params = HullWhite1FParams::with_piecewise_sigma(
            0.1,
            vec![0.0, 1.0],
            vec![0.01, 0.02],
            vec![0.03],
            vec![0.0],
        )
        .expect("valid Hull-White schedules");
        let process = HullWhite1FProcess::new(params);
        let disc = ExactHullWhite1F::new();
        let mut x = vec![0.03];
        let mut work = vec![];
        let z = vec![1.0];

        disc.step(&process, 0.0, 2.0, &mut x, &z, &mut work);

        let expected_mean = 0.03;
        let first = 0.01_f64.powi(2) * ((-0.2_f64).exp() - (-0.4_f64).exp()) / 0.2;
        let second = 0.02_f64.powi(2) * (1.0 - (-0.2_f64).exp()) / 0.2;
        let expected = expected_mean + (first + second).sqrt();
        assert!((x[0] - expected).abs() < 1.0e-12);
    }
}
