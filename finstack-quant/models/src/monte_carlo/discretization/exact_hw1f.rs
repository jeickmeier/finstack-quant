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

use super::super::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use super::super::traits::Discretization;
use finstack_quant_core::math::time_grid::TimeGrid;

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
    kappa: f64,
    sigma: f64,
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
            kappa,
            sigma,
            exp_kappa_dt,
            std_dev,
        }
    }

    fn matches(self, kappa: f64, sigma: f64, dt: f64) -> bool {
        self.dt.to_bits() == dt.to_bits()
            && self.kappa.to_bits() == kappa.to_bits()
            && self.sigma.to_bits() == sigma.to_bits()
    }
}

/// Exact Hull-White transition coefficients for one prepared grid interval.
///
/// Constructed by [`ExactHullWhite1F::prepare_grid`]. The interval's calibrated
/// parameters and time bounds are frozen; applying it requires only the current
/// decimal short rate and one independent standard-normal innovation.
#[derive(Debug, Clone, Copy)]
pub struct HullWhiteStep {
    exp_kappa_dt: f64,
    theta_contribution: f64,
    std_dev: f64,
}

impl HullWhiteStep {
    fn compute(
        params: &HullWhite1FParams,
        t: f64,
        dt: f64,
        prepared: Option<Hw1fStepConstants>,
    ) -> Self {
        let theta_contribution = params.theta_mean_contribution(t, dt);
        let constant_sigma =
            (params.model.get_times().len() == 1).then(|| params.model.get_values()[0]);
        let (exp_kappa_dt, std_dev) = match (prepared, constant_sigma) {
            (Some(c), Some(sigma)) if c.matches(params.model.kappa, sigma, dt) => {
                (c.exp_kappa_dt, c.std_dev)
            }
            (_, Some(sigma)) => {
                let c = Hw1fStepConstants::compute(params.model.kappa, sigma, dt);
                (c.exp_kappa_dt, c.std_dev)
            }
            _ => (
                (-params.model.kappa * dt).exp(),
                params.sigma_variance_for_step(t, dt).max(0.0).sqrt(),
            ),
        };
        Self {
            exp_kappa_dt,
            theta_contribution,
            std_dev,
        }
    }

    /// Advance the short rate over this interval using its frozen coefficients.
    ///
    /// # Arguments
    ///
    /// * `short_rate` - Decimal short rate at this interval's left grid node;
    ///   negative rates are supported.
    /// * `standard_normal` - Standard-normal shock for this interval, with no
    ///   time scaling applied; negate the same shock for an antithetic leg.
    ///
    /// # Returns
    ///
    /// Decimal short rate at this interval's right grid node. The operation does
    /// not draw randomness or change the prepared coefficients.
    #[inline]
    #[must_use]
    pub fn apply(&self, short_rate: f64, standard_normal: f64) -> f64 {
        let mean = short_rate * self.exp_kappa_dt + self.theta_contribution;
        mean + self.std_dev * standard_normal
    }
}

/// Immutable exact Hull-White transitions in simulation-grid order.
///
/// Stores one coefficient record per grid interval, shared across all paths in
/// a valuation. Rebuild after changing the grid, volatility or calibrated theta
/// schedule; the sequence never observes subsequent changes to its inputs.
#[derive(Debug, Clone)]
pub struct PreparedHullWhiteSteps {
    steps: Vec<HullWhiteStep>,
}

impl PreparedHullWhiteSteps {
    /// Borrow the transitions in increasing grid-interval order.
    ///
    /// The slice has exactly the preparation grid's `num_steps()` entries.
    /// Entry `i` advances the rate from grid node `i` to node `i + 1`.
    #[must_use]
    pub fn get_steps(&self) -> &[HullWhiteStep] {
        &self.steps
    }
}

impl ExactHullWhite1F {
    /// Create a new exact HW1F discretization.
    pub fn new() -> Self {
        Self { prepared: None }
    }

    /// Freeze the exact transition for every interval of a simulation grid.
    ///
    /// Preparation belongs inside the valuation after its grid and calibrated
    /// parameters are final. It integrates every theta and volatility knot
    /// within each interval and stores only O(grid intervals) coefficients.
    ///
    /// # Arguments
    ///
    /// * `process` - Validated Hull-White process: mean reversion in inverse
    ///   years, volatility per square-root year, and decimal theta levels with
    ///   knot times measured from the same origin as `grid`.
    /// * `grid` - Validated simulation times in year fractions, starting at zero;
    ///   its stored interval widths are used without rounding or resampling.
    ///
    /// # Returns
    ///
    /// An owned immutable sequence with one transition per grid interval. A
    /// zero-step grid produces an empty sequence. Changing either input requires
    /// preparing a new sequence; no cache is retained across valuations.
    #[must_use]
    pub fn prepare_grid(process: &HullWhite1FProcess, grid: &TimeGrid) -> PreparedHullWhiteSteps {
        let mut disc = Self::new();
        disc.prepare(process, grid);
        let steps = (0..grid.num_steps())
            .map(|step| {
                HullWhiteStep::compute(
                    process.params(),
                    grid.time(step),
                    grid.dt(step),
                    disc.prepared,
                )
            })
            .collect();
        PreparedHullWhiteSteps { steps }
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
        let interval = HullWhiteStep::compute(process.params(), t, dt, self.prepared);
        x[0] = interval.apply(x[0], z[0]);
    }

    fn prepare(&mut self, process: &HullWhite1FProcess, time_grid: &TimeGrid) {
        let params = process.params();
        self.prepared = if time_grid.num_steps() > 0 && params.model.get_times().len() == 1 {
            Some(Hw1fStepConstants::compute(
                params.model.kappa,
                params.model.get_values()[0],
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

    // Frozen pre-preparation arithmetic. Keep this test oracle independent of
    // the production coefficient constructor so moving work cannot change bits.
    fn reference_coefficients(params: &HullWhite1FParams, t: f64, dt: f64) -> [f64; 3] {
        let theta = params.theta_mean_contribution(t, dt);
        let kappa = params.model.kappa;
        let decay = (-kappa * dt).exp();
        let std_dev = if params.model.get_times().len() == 1 {
            let sigma = params.model.get_values()[0];
            if (kappa * dt).abs() < 1e-8 {
                sigma * dt.sqrt() * (1.0 - kappa * dt / 2.0)
            } else {
                sigma * ((1.0 - (-2.0 * kappa * dt).exp()) / (2.0 * kappa)).sqrt()
            }
        } else {
            params.sigma_variance_for_step(t, dt).max(0.0).sqrt()
        };
        [decay, theta, std_dev]
    }

    fn assert_prepared_matches_reference(process: &HullWhite1FProcess, grid: &TimeGrid) {
        let prepared = ExactHullWhite1F::prepare_grid(process, grid);
        assert_eq!(prepared.get_steps().len(), grid.num_steps());
        let direct = ExactHullWhite1F::new();
        for initial_rate in [-0.03, 0.0, 0.07] {
            let mut rate = initial_rate;
            let mut direct_rate = [initial_rate];
            let mut expected = initial_rate;
            for (step, transition) in prepared.get_steps().iter().enumerate() {
                let t = grid.time(step);
                let dt = grid.dt(step);
                let coefficients = reference_coefficients(process.params(), t, dt);
                assert_eq!(
                    [
                        transition.exp_kappa_dt,
                        transition.theta_contribution,
                        transition.std_dev,
                    ]
                    .map(f64::to_bits),
                    coefficients.map(f64::to_bits),
                    "coefficient mismatch at interval {step}"
                );
                let innovation = [-1.25, 0.0, 0.875, -0.125][step % 4];
                let mean = expected * coefficients[0] + coefficients[1];
                expected = mean + coefficients[2] * innovation;
                rate = transition.apply(rate, innovation);
                direct.step(process, t, dt, &mut direct_rate, &[innovation], &mut []);
                assert_eq!(rate.to_bits(), expected.to_bits(), "interval {step}");
                assert_eq!(
                    direct_rate[0].to_bits(),
                    expected.to_bits(),
                    "interval {step}"
                );
            }
        }
    }

    #[test]
    fn prepared_hw1f_coefficients_and_paths_preserve_direct_bits() {
        let grids = [
            TimeGrid::uniform(1.0, 12).expect("uniform grid"),
            TimeGrid::from_times(vec![0.0, 0.125, 0.25, 0.5, 0.75, 1.0])
                .expect("knot-aligned grid"),
            TimeGrid::from_times(vec![0.0, 0.05, 0.4, 1.0, 2.0])
                .expect("grid straddling several knots"),
        ];
        // The last grid includes a unit interval, covering both sides of the
        // existing kappa*dt threshold without changing its approximation.
        for kappa in [1e-10, 0.999999999e-8, 1e-8, 1.000000001e-8, 0.1, 20.0] {
            for piecewise_sigma in [false, true] {
                let params = if piecewise_sigma {
                    HullWhite1FParams::with_piecewise_sigma(
                        kappa,
                        vec![0.0, 0.125, 0.5],
                        vec![0.01, 0.025, 0.007],
                        vec![-0.02, 0.05, 0.01, -0.03],
                        vec![0.0, 0.1, 0.2, 0.75],
                    )
                } else {
                    HullWhite1FParams::with_time_dependent_theta(
                        kappa,
                        0.01,
                        vec![-0.02, 0.05, 0.01, -0.03],
                        vec![0.0, 0.1, 0.2, 0.75],
                    )
                }
                .expect("valid parameter schedules");
                let process = HullWhite1FProcess::new(params);
                for grid in &grids {
                    assert_prepared_matches_reference(&process, grid);
                }
            }
        }
    }

    #[test]
    fn prepared_hw1f_rebuilds_for_changed_grids_and_parameters() {
        let grids = [
            TimeGrid::from_times(vec![0.0, 0.25, 0.5, 1.0]).expect("first grid"),
            TimeGrid::from_times(vec![0.0, 0.5, 0.75, 1.0]).expect("same-length changed grid"),
            TimeGrid::from_times(vec![0.0]).expect("zero-step grid"),
            TimeGrid::uniform(2.0, 5).expect("different-length grid"),
        ];
        let parameters = [
            HullWhite1FParams::new(0.1, 0.01, -0.02).expect("constant parameters"),
            HullWhite1FParams::with_time_dependent_theta(
                0.2,
                0.03,
                vec![0.04, -0.03],
                vec![0.0, 0.4],
            )
            .expect("changed kappa, sigma and theta"),
            HullWhite1FParams::with_piecewise_sigma(
                0.2,
                vec![0.0, 0.3],
                vec![0.03, 0.015],
                vec![-0.01, 0.02, 0.05],
                vec![0.0, 0.25, 0.6],
            )
            .expect("changed sigma and theta schedules"),
        ];
        for params in parameters {
            let process = HullWhite1FProcess::new(params);
            for grid in &grids {
                assert_prepared_matches_reference(&process, grid);
            }
            // A previously prepared sequence remains an owned snapshot after
            // constructing a different sequence and passing an empty grid.
            let original = ExactHullWhite1F::prepare_grid(&process, &grids[0]);
            let original_bits: Vec<_> = original
                .get_steps()
                .iter()
                .map(|step| step.apply(0.03, -0.75).to_bits())
                .collect();
            let empty = ExactHullWhite1F::prepare_grid(&process, &grids[2]);
            assert!(empty.get_steps().is_empty());
            let rebuilt = ExactHullWhite1F::prepare_grid(&process, &grids[0]);
            let rebuilt_bits: Vec<_> = rebuilt
                .get_steps()
                .iter()
                .map(|step| step.apply(0.03, -0.75).to_bits())
                .collect();
            assert_eq!(original_bits, rebuilt_bits);
        }
    }

    #[test]
    fn direct_hw1f_cache_checks_process_and_clears_empty_preparation() {
        let initial = HullWhite1FProcess::new(
            HullWhite1FParams::new(0.1, 0.01, 0.03).expect("initial parameters"),
        );
        let grid = TimeGrid::uniform(1.0, 4).expect("grid");
        let mut disc = ExactHullWhite1F::new();
        disc.prepare(&initial, &grid);
        assert!(disc.prepared.is_some());
        let processes = [
            initial.clone(),
            HullWhite1FProcess::new(
                HullWhite1FParams::new(0.2, 0.01, 0.03).expect("changed kappa"),
            ),
            HullWhite1FProcess::new(
                HullWhite1FParams::new(0.1, 0.02, 0.03).expect("changed sigma"),
            ),
            HullWhite1FProcess::new(
                HullWhite1FParams::with_piecewise_sigma(
                    0.1,
                    vec![0.0, 0.2],
                    vec![0.01, 0.025],
                    vec![-0.01, 0.04],
                    vec![0.0, 0.3],
                )
                .expect("changed sigma and theta schedules"),
            ),
        ];
        for process in &processes {
            // Equal widths, different starts and arbitrary off-grid intervals
            // must all use the current process rather than stale coefficients.
            for (t, dt) in [(0.0, 0.25), (0.25, 0.25), (0.13, 0.17)] {
                let mut actual = [-0.02];
                let mut expected = actual;
                disc.step(process, t, dt, &mut actual, &[0.7], &mut []);
                ExactHullWhite1F::new().step(process, t, dt, &mut expected, &[0.7], &mut []);
                assert_eq!(actual[0].to_bits(), expected[0].to_bits());
            }
        }
        let empty = TimeGrid::from_times(vec![0.0]).expect("zero-step grid");
        disc.prepare(&initial, &empty);
        assert!(disc.prepared.is_none());
        disc.prepare(&processes[2], &grid);
        assert!(disc.prepared.is_some());
        let mut actual = [0.03];
        let mut expected = actual;
        disc.step(&processes[2], 0.0, 0.25, &mut actual, &[-1.0], &mut []);
        ExactHullWhite1F::new().step(&processes[2], 0.0, 0.25, &mut expected, &[-1.0], &mut []);
        assert_eq!(actual[0].to_bits(), expected[0].to_bits());
        disc.prepare(&processes[3], &grid);
        assert!(disc.prepared.is_none());
    }
}
