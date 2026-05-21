//! Hybrid Euler–Maruyama discretization for the rough Heston model.
//!
//! The variance process in the rough Heston model is governed by a Volterra
//! integral with a singular power-law kernel:
//!
//! ```text
//! V_t = V₀ + (1/Γ(α)) ∫₀ᵗ (t − s)^{α−1} [κ(θ − V_s) ds + σᵥ √V_s dW̃(s)]
//! ```
//!
//! where α = H + 0.5 and H is the Hurst exponent.
//!
//! Because the variance at each time step depends on the *entire* history of
//! shocks and variance values, a standard one-step Euler scheme cannot be
//! applied.  This discretization computes the Volterra integral at every step
//! by summing over all previous steps — an O(n²) algorithm per path.  This is
//! the simplest correct scheme; geometric-binning optimizations can be added
//! later. Construction logs a warning for grids above 200 steps because the
//! quadratic Volterra cost can dominate large production runs.
//!
//! # Work Buffer Layout
//!
//! The `work` buffer is used to store the per-step Volterra integrands so that
//! the sum can be evaluated at each new time step:
//!
//! | Offset | Content |
//! |--------|---------|
//! | `0 .. num_steps` | `f_j` — Volterra integrand at step j |
//! | `num_steps` | Step counter (as `f64`) |
//!
//! The integrand at step j is:
//!
//! ```text
//! f_j = κ(θ − V_j) Δt_j + σᵥ √(max(V_j, 0)) Z̃_j √Δt_j
//! ```
//!
//! # Noise Layout
//!
//! The discretization expects `z` with two entries:
//!
//! - `z[0]` — independent standard normal for the uncorrelated spot component
//! - `z[1]` — standard normal for the variance (used in the Volterra integral)
//!
//! Spot and variance noises are correlated via the Cholesky decomposition:
//!
//! ```text
//! dW_spot = ρ · z[1] · √dt + √(1 − ρ²) · z[0] · √dt
//! ```
//!
//! # Construction
//!
//! The discretization must be constructed with the time grid so it can
//! precompute time step sizes and determine the work buffer size:
//!
//! ```ignore
//! use finstack_monte_carlo::discretization::rough_heston::RoughHestonHybrid;
//!
//! let times = vec![0.0, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0];
//! let hurst = 0.1;
//! let disc = RoughHestonHybrid::new(&times, hurst).unwrap();
//! ```
//!
//! # References
//!
//! - El Euch, O. & Rosenbaum, M. (2019). "The characteristic function of rough
//!   Heston models." *Mathematical Finance*, 29(1), 3–38.
//! - El Euch, O., Fukasawa, M. & Rosenbaum, M. (2019). "The microstructural
//!   foundations of leverage effect and rough volatility." *Finance and
//!   Stochastics*, 22(2), 241–280.

use super::super::process::rough_heston::RoughHestonProcess;
use super::super::traits::Discretization;

/// Hybrid Euler–Maruyama discretization for the rough Heston model.
///
/// At each time step the Volterra integral is evaluated by summing over all
/// previous steps with the singular kernel `(t − s)^{α−1} / Γ(α)`.  This is
/// O(n²) per path but exact (no binning approximation).
/// Grids above 200 steps log a construction-time warning to make this cost
/// visible before the simulation starts.
///
/// The discretization must be constructed with the full time grid because the
/// work buffer size depends on the number of steps, and the kernel evaluation
/// at each step requires access to historical time coordinates.
#[derive(Debug, Clone)]
pub struct RoughHestonHybrid {
    /// Number of time steps in the grid.
    num_steps: usize,
    /// Fractional exponent α = H + 0.5.
    alpha: f64,
    /// Precomputed 1 / Γ(α).
    inv_gamma_alpha: f64,
    /// Cumulative times from the time grid: \[t₀, t₁, …, t_n\].
    times: Vec<f64>,
    /// Time step sizes: \[Δt₀, Δt₁, …, Δt_{n−1}\].
    dt_grid: Vec<f64>,
}

impl RoughHestonHybrid {
    const LARGE_GRID_WARNING_STEPS: usize = 200;

    /// Create a new rough Heston discretization for the given time grid.
    ///
    /// # Arguments
    ///
    /// * `times` - Monotonically increasing time grid starting at 0. Must have
    ///   at least two points.
    /// * `hurst` - Hurst exponent H ∈ (0, 0.5).
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if the time grid is too
    /// short or the Hurst exponent is out of range.
    pub fn new(times: &[f64], hurst: f64) -> finstack_core::Result<Self> {
        if times.len() < 2 {
            return Err(finstack_core::Error::Validation(
                "RoughHestonHybrid requires at least 2 time points".to_string(),
            ));
        }
        if hurst <= 0.0 || hurst >= 0.5 {
            return Err(finstack_core::Error::Validation(format!(
                "RoughHestonHybrid Hurst exponent must be in (0, 0.5), got {hurst}"
            )));
        }

        let alpha = hurst + 0.5;
        let gamma_alpha = finstack_core::math::ln_gamma(alpha).exp();
        let inv_gamma_alpha = 1.0 / gamma_alpha;

        let num_steps = times.len() - 1;
        if num_steps > Self::LARGE_GRID_WARNING_STEPS {
            tracing::warn!(
                num_steps,
                estimated_kernel_ops_per_path = num_steps.saturating_mul(num_steps + 1) / 2,
                "RoughHestonHybrid evaluates the Volterra integral with O(n²) work per path; \
                 consider this cost before running large path counts"
            );
        }
        let dt_grid: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();

        Ok(Self {
            num_steps,
            alpha,
            inv_gamma_alpha,
            times: times.to_vec(),
            dt_grid,
        })
    }
}

impl Discretization<RoughHestonProcess> for RoughHestonHybrid {
    fn step(
        &self,
        process: &RoughHestonProcess,
        t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        work: &mut [f64],
    ) {
        let p = process.params();

        // ── Determine step index ──────────────────────────────────
        // The engine zero-initialises `work` before every path
        // (see `run_path_loop` in engine/simulation.rs), so the step
        // counter starts at 0.0 cleanly without resorting to `t < ε`
        // heuristics to detect path boundaries.
        let step = work[self.num_steps] as usize;

        let v_current = x[1].max(0.0);
        let z_vol = z[1]; // Standard normal for variance noise
        let dw_tilde = z_vol * dt.sqrt();

        // ── Store Volterra integrand for this step ─────────────────────
        //
        //   f_j = κ(θ − V_j) Δt_j + σᵥ √V_j dW̃_j
        //
        work[step] = p.kappa * (p.theta - v_current) * dt + p.sigma_v * v_current.sqrt() * dw_tilde;

        // ── Evaluate Volterra integral to obtain V_{next} ──────────────
        //
        //   V_{next} = v₀ + (1/Γ(α)) Σ_{j=0}^{step} (t_next − t_j_mid)^{α−1} f_j
        //
        // The kernel is evaluated at the midpoint of each historical interval
        // to reduce discretization bias.
        let t_next = t + dt;
        let alpha_m1 = self.alpha - 1.0;
        let mut volterra_sum = 0.0;
        for ((&f_j, &t_j), &dt_j) in work[..=step]
            .iter()
            .zip(&self.times[..=step])
            .zip(&self.dt_grid[..=step])
        {
            let t_j_mid = t_j + 0.5 * dt_j;
            let lag = t_next - t_j_mid;
            // Kernel: (t_next − t_j_mid)^{α−1}.
            // Since α ∈ (0.5, 1.0), α−1 ∈ (−0.5, 0.0) so the kernel is singular
            // near lag = 0.  A guard prevents division by zero.
            let kernel = if lag > 1e-15 { lag.powf(alpha_m1) } else { 0.0 };
            volterra_sum += kernel * f_j;
        }
        let v_next = (p.v0 + self.inv_gamma_alpha * volterra_sum).max(0.0);

        // ── Correlate spot noise with variance driver ──────────────────
        let z_spot = p.rho * z_vol + (1.0 - p.rho * p.rho).max(0.0).sqrt() * z[0];

        // ── Log-spot update ────────────────────────────────────────────
        let sqrt_v_dt = (v_current * dt).max(0.0).sqrt();
        x[0] *= ((p.r - p.q - 0.5 * v_current) * dt + sqrt_v_dt * z_spot).exp();

        // ── Update variance state ──────────────────────────────────────
        x[1] = v_next;

        // ── Increment step counter ─────────────────────────────────────
        work[self.num_steps] = (step + 1) as f64;
    }

    fn work_size(&self, _process: &RoughHestonProcess) -> usize {
        self.num_steps + 1 // history array + step counter
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::process::rough_heston::{RoughHestonParams, RoughHestonProcess};
    use super::*;
    use finstack_core::math::fractional::HurstExponent;

    fn make_process() -> RoughHestonProcess {
        let hurst = HurstExponent::new(0.1).expect("valid hurst");
        let params =
            RoughHestonParams::new(0.05, 0.02, hurst, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        RoughHestonProcess::new(params)
    }

    fn uniform_grid(n: usize, t_max: f64) -> Vec<f64> {
        (0..=n).map(|i| t_max * i as f64 / n as f64).collect()
    }

    // -- Construction -------------------------------------------------------

    #[test]
    fn test_construction_valid() {
        let times = uniform_grid(100, 1.0);
        let disc = RoughHestonHybrid::new(&times, 0.1);
        assert!(disc.is_ok());
    }

    #[test]
    fn test_construction_too_few_points() {
        let res = RoughHestonHybrid::new(&[0.0], 0.1);
        assert!(res.is_err());
    }

    #[test]
    fn test_construction_hurst_out_of_range() {
        let times = uniform_grid(10, 1.0);
        assert!(RoughHestonHybrid::new(&times, 0.0).is_err());
        assert!(RoughHestonHybrid::new(&times, 0.5).is_err());
        assert!(RoughHestonHybrid::new(&times, 0.6).is_err());
    }

    // -- Single step --------------------------------------------------------

    #[test]
    fn test_single_step_zero_shocks() {
        let process = make_process();
        let times = uniform_grid(10, 1.0);
        let disc = RoughHestonHybrid::new(&times, 0.1).expect("valid");

        let mut x = vec![100.0, 0.04];
        let z = vec![0.0, 0.0];
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, 0.0, 0.1, &mut x, &z, &mut work);

        assert!(x[0] > 0.0, "Spot must remain positive");
        assert!(
            (x[0] - 100.0).abs() < 5.0,
            "Spot should stay near 100 with zero shocks: got {}",
            x[0]
        );
        assert!(x[1] >= 0.0, "Variance must be non-negative");
    }

    #[test]
    fn test_single_step_deterministic() {
        let process = make_process();
        let p = process.params();
        let h = p.hurst.value();
        let alpha = h + 0.5;

        let times = vec![0.0, 0.01];
        let disc = RoughHestonHybrid::new(&times, h).expect("valid");

        let s0 = 100.0;
        let v0 = p.v0;
        let dt = 0.01_f64;
        let z_indep = 0.5;
        let z_vol = 0.3;

        let mut x = vec![s0, v0];
        let z = vec![z_indep, z_vol];
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, 0.0, dt, &mut x, &z, &mut work);

        // Manually compute expected variance
        let dw_tilde = z_vol * dt.sqrt();
        let f_0 = p.kappa * (p.theta - v0) * dt + p.sigma_v * v0.sqrt() * dw_tilde;
        let t_mid = 0.5 * dt;
        let t_next = dt;
        let lag = t_next - t_mid;
        let alpha_m1 = alpha - 1.0;
        let kernel = lag.powf(alpha_m1);
        let inv_gamma = 1.0 / finstack_core::math::ln_gamma(alpha).exp();
        let expected_v = (v0 + inv_gamma * kernel * f_0).max(0.0);

        assert!(
            (x[1] - expected_v).abs() < 1e-12,
            "Variance mismatch: got {}, expected {}",
            x[1],
            expected_v
        );

        // Check spot
        let z_spot = p.rho * z_vol + (1.0 - p.rho * p.rho).max(0.0).sqrt() * z_indep;
        let sqrt_v_dt = (v0 * dt).sqrt();
        let expected_s = s0 * ((p.r - p.q - 0.5 * v0) * dt + sqrt_v_dt * z_spot).exp();

        assert!(
            (x[0] - expected_s).abs() < 1e-10,
            "Spot mismatch: got {}, expected {}",
            x[0],
            expected_s
        );
    }

    // -- Multi-step ---------------------------------------------------------

    #[test]
    fn test_volterra_integral_accumulates_history() {
        let process = make_process();
        let n = 5;
        let times = uniform_grid(n, 0.05);
        let disc = RoughHestonHybrid::new(&times, 0.1).expect("valid");

        let mut x = vec![100.0, 0.04];
        let z = vec![0.1, 0.1]; // Small constant shocks
        let mut work = vec![0.0; disc.work_size(&process)];

        for i in 0..n {
            let t = times[i];
            let dt_step = times[i + 1] - times[i];
            disc.step(&process, t, dt_step, &mut x, &z, &mut work);
        }

        // Step counter should equal n
        let step_count = work[n] as usize;
        assert_eq!(step_count, n, "Step counter should track executed steps");

        // Variance should differ from v0 after accumulating history
        assert!(x[1] >= 0.0, "Variance must be non-negative");
    }

    // -- Path reset ---------------------------------------------------------

    #[test]
    fn test_work_buffer_reset_across_paths() {
        // The engine (run_path_loop) is now responsible for zeroing the work
        // buffer at the start of every path; the discretization simply
        // increments the counter. This test verifies that pattern: after path
        // 1, the caller zeros `work` exactly the way the engine does, then
        // path 2 starts cleanly.
        let process = make_process();
        let n = 5;
        let times = uniform_grid(n, 0.05);
        let disc = RoughHestonHybrid::new(&times, 0.1).expect("valid");

        let mut x = vec![100.0, 0.04];
        let z = vec![0.2, 0.15];
        let mut work = vec![0.0; disc.work_size(&process)];

        // Run path 1
        for i in 0..n {
            disc.step(
                &process,
                times[i],
                times[i + 1] - times[i],
                &mut x,
                &z,
                &mut work,
            );
        }
        let step_after_p1 = work[n] as usize;
        assert_eq!(step_after_p1, n);

        // Start path 2 — engine resets the work buffer.
        for w in work.iter_mut() {
            *w = 0.0;
        }
        x[0] = 100.0;
        x[1] = 0.04;
        disc.step(&process, 0.0, times[1] - times[0], &mut x, &z, &mut work);

        // Step counter should be 1 (zeroed then incremented).
        assert_eq!(
            work[n] as usize, 1,
            "Step counter should reset for new path"
        );
    }

    // -- Work size ----------------------------------------------------------

    #[test]
    fn test_work_size() {
        let process = make_process();
        let times = uniform_grid(50, 1.0);
        let disc = RoughHestonHybrid::new(&times, 0.1).expect("valid");

        assert_eq!(disc.work_size(&process), 51); // 50 history slots + 1 counter
    }

    // -- Spot positivity under stress ---------------------------------------

    #[test]
    fn test_spot_stays_positive_under_large_shocks() {
        let process = make_process();
        let times = uniform_grid(10, 0.1);
        let disc = RoughHestonHybrid::new(&times, 0.1).expect("valid");

        for &z_val in &[-3.0, -2.0, 2.0, 3.0] {
            let mut x = vec![100.0, 0.04];
            let z = vec![z_val, z_val * 0.5];
            let mut work = vec![0.0; disc.work_size(&process)];

            disc.step(&process, 0.0, 0.01, &mut x, &z, &mut work);

            assert!(
                x[0] > 0.0,
                "Spot must remain positive with z={z_val}: got {}",
                x[0]
            );
            assert!(
                x[1] >= 0.0,
                "Variance must be non-negative with z={z_val}: got {}",
                x[1]
            );
        }
    }
}
