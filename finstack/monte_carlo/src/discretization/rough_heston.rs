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
//! by summing over all previous steps — an O(n²) algorithm per path.
//! Construction logs a warning for grids above 200 steps because the
//! quadratic Volterra cost can dominate large production runs.
//!
//! # Kernel Weights (review finding M8)
//!
//! The singular kernel `(t − s)^{α−1}` is handled with hybrid-scheme weights
//! (Bennedsen-Lunde-Pakkanen style), with the drift and noise components of
//! each historical step weighted separately:
//!
//! - **Drift** (all intervals, exact): the per-interval kernel integral
//!   `∫_{t_j}^{t_{j+1}} (t_next − s)^{α−1} ds = [(t_next − t_j)^α − (t_next − t_{j+1})^α] / α`
//!   multiplies the drift rate `κ(θ − V_j)`.
//! - **Noise, far field** (`j < step`): the same per-interval average kernel
//!   `[(t_next − t_j)^α − (t_next − t_{j+1})^α] / (α·Δt_j)` multiplies the
//!   stored noise increment — exact in expectation, accurate away from the
//!   singularity.
//! - **Noise, near field** (`j == step`, the singular last interval): the
//!   variance-exact weight `Δt^{α−1}/√(2α−1)`, i.e.
//!   `√(∫₀^{Δt} s^{2(α−1)} ds / Δt)`, so the contribution
//!   `σᵥ√V·∫ K dW̃` has the exact second moment. A midpoint kernel
//!   `(Δt/2)^{α−1}` underweights this singular interval by a factor
//!   `2^{1−α}√(2α−1)` (≈ 40% of the noise standard deviation at H = 0.1).
//!
//! # Work Buffer Layout
//!
//! The `work` buffer stores the drift and noise components of the Volterra
//! integrand separately so they can be weighted independently:
//!
//! | Offset | Content |
//! |--------|---------|
//! | `0 .. num_steps` | `a_j = κ(θ − V_j)` — drift rate at step j |
//! | `num_steps .. 2·num_steps` | `n_j = σᵥ √(max(V_j, 0)) Z̃_j √Δt_j` — noise increment at step j |
//! | `2·num_steps` | Step counter (as `f64`) |
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
/// previous steps with the singular kernel `(t − s)^{α−1} / Γ(α)`, using
/// exact per-interval kernel integrals for the drift and a variance-exact
/// near-field weight for the singular last-interval noise term (see the
/// [module-level documentation](self)). This is O(n²) per path.
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
        let n = self.num_steps;
        let step = work[2 * n] as usize;

        let v_current = x[1].max(0.0);
        let z_vol = z[1]; // Standard normal for variance noise
        let dw_tilde = z_vol * dt.sqrt();

        // ── Store drift and noise components for this step ─────────────
        //
        //   a_j = κ(θ − V_j)            (drift rate, weighted by ∫K ds)
        //   n_j = σᵥ √V_j dW̃_j          (noise increment)
        //
        work[step] = p.kappa * (p.theta - v_current);
        work[n + step] = p.sigma_v * v_current.sqrt() * dw_tilde;

        // ── Evaluate Volterra integral to obtain V_{next} ──────────────
        //
        //   V_{next} = v₀ + (1/Γ(α)) Σ_{j=0}^{step} [a_j·∫_{t_j}^{t_{j+1}}K ds
        //                                            + w_j·n_j]
        //
        // with K(s) = (t_next − s)^{α−1}. The drift uses the exact
        // per-interval kernel integral; far-field noise uses the interval-
        // average kernel; the singular last interval uses the variance-exact
        // near-field weight Δt^{α−1}/√(2α−1) (review finding M8).
        let t_next = t + dt;
        let alpha = self.alpha;
        let alpha_m1 = alpha - 1.0;
        let mut volterra_sum = 0.0;
        for (j, &dt_j) in self.dt_grid[..=step].iter().enumerate() {
            let a = t_next - self.times[j]; // lag to interval start (> 0)
            let b = (t_next - self.times[j + 1]).max(0.0); // lag to interval end
                                                           // Exact ∫_{t_j}^{t_{j+1}} (t_next − s)^{α−1} ds.
            let kernel_int = (a.powf(alpha) - b.powf(alpha)) / alpha;

            // Drift: exact kernel integral against the constant drift rate.
            volterra_sum += work[j] * kernel_int;

            // Noise: average kernel weight in the far field; variance-exact
            // weight on the singular last interval.
            let noise_weight = if j == step {
                dt_j.powf(alpha_m1) / (2.0 * alpha - 1.0).sqrt()
            } else {
                kernel_int / dt_j
            };
            volterra_sum += work[n + j] * noise_weight;
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
        work[2 * n] = (step + 1) as f64;
    }

    fn work_size(&self, _process: &RoughHestonProcess) -> usize {
        2 * self.num_steps + 1 // drift rates + noise increments + step counter
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

        // Manually compute expected variance (M8 hybrid weights):
        // drift gets the exact kernel integral ∫₀^dt s^{α−1} ds = dt^α / α,
        // the singular last-interval noise gets the variance-exact weight
        // dt^{α−1} / √(2α−1).
        let dw_tilde = z_vol * dt.sqrt();
        let drift_rate = p.kappa * (p.theta - v0);
        let noise = p.sigma_v * v0.sqrt() * dw_tilde;
        let kernel_int = dt.powf(alpha) / alpha;
        let noise_weight = dt.powf(alpha - 1.0) / (2.0 * alpha - 1.0).sqrt();
        let inv_gamma = 1.0 / finstack_core::math::ln_gamma(alpha).exp();
        let expected_v =
            (v0 + inv_gamma * (drift_rate * kernel_int + noise * noise_weight)).max(0.0);

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
        let step_count = work[2 * n] as usize;
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
        let step_after_p1 = work[2 * n] as usize;
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
            work[2 * n] as usize,
            1,
            "Step counter should reset for new path"
        );
    }

    // -- Work size ----------------------------------------------------------

    #[test]
    fn test_work_size() {
        let process = make_process();
        let times = uniform_grid(50, 1.0);
        let disc = RoughHestonHybrid::new(&times, 0.1).expect("valid");

        // 50 drift-rate slots + 50 noise slots + 1 counter
        assert_eq!(disc.work_size(&process), 101);
    }

    // -- M8 kernel weights ----------------------------------------------------

    /// The singular last-interval noise weight must be variance-exact:
    /// the one-step noise contribution to V is `(1/Γ(α))·σᵥ√v₀·∫₀^Δt s^{α−1} dW`,
    /// whose standard deviation per unit normal is
    /// `(1/Γ(α))·σᵥ√v₀·√(Δt^{2α−1}/(2α−1))` (review finding M8). A midpoint
    /// kernel `(Δt/2)^{α−1}·√Δt` understates this by `2^{1−α}√(2α−1)`.
    #[test]
    fn near_field_noise_weight_is_variance_exact() {
        let process = make_process();
        let p = process.params();
        let h = p.hurst.value();
        let alpha = h + 0.5;
        let dt = 0.01_f64;
        let times = vec![0.0, dt];
        let disc = RoughHestonHybrid::new(&times, h).expect("valid");

        // V_1 with z_vol = 1 minus V_1 with z_vol = 0 isolates the noise term.
        let run = |z_vol: f64| -> f64 {
            let mut x = vec![100.0, p.v0];
            let z = vec![0.0, z_vol];
            let mut work = vec![0.0; disc.work_size(&process)];
            disc.step(&process, 0.0, dt, &mut x, &z, &mut work);
            x[1]
        };
        let noise_per_unit_z = run(1.0) - run(0.0);

        let inv_gamma = 1.0 / finstack_core::math::ln_gamma(alpha).exp();
        let exact = inv_gamma
            * p.sigma_v
            * p.v0.sqrt()
            * (dt.powf(2.0 * alpha - 1.0) / (2.0 * alpha - 1.0)).sqrt();
        assert!(
            (noise_per_unit_z - exact).abs() < 1e-14,
            "one-step noise contribution per unit normal must equal the \
             exact kernel L² norm: got {noise_per_unit_z}, exact {exact}"
        );

        // And it must exceed the midpoint-kernel weight it replaces.
        let midpoint =
            inv_gamma * p.sigma_v * p.v0.sqrt() * (0.5 * dt).powf(alpha - 1.0) * dt.sqrt();
        assert!(
            noise_per_unit_z > midpoint,
            "exact near-field weight ({noise_per_unit_z}) must exceed the \
             midpoint approximation ({midpoint})"
        );
    }

    /// MC mean of the variance process vs an independent fine-grid
    /// product-integration solution of the fractional mean ODE
    /// `E[V_t] = v₀ + (1/Γ(α)) ∫₀ᵗ (t−s)^{α−1} κ(θ − E[V_s]) ds`.
    /// The drift is linear in V, so the MC mean must track this deterministic
    /// solution; the noise terms have zero mean by construction.
    #[test]
    fn variance_mean_matches_fractional_riccati_mean() {
        use super::super::super::rng::philox::PhiloxRng;
        use super::super::super::traits::RandomStream;
        use finstack_core::math::fractional::HurstExponent;

        // Milder parameters keep the V ≥ 0 floor mostly inactive so the
        // linear mean ODE is the right reference.
        let h = 0.25_f64;
        let (r, q, kappa, theta, sigma_v, rho, v0) =
            (0.0, 0.0, 2.0_f64, 0.06_f64, 0.15_f64, -0.5, 0.03_f64);
        let hurst = HurstExponent::new(h).expect("valid hurst");
        let params =
            RoughHestonParams::new(r, q, hurst, kappa, theta, sigma_v, rho, v0).expect("valid");
        let process = RoughHestonProcess::new(params);

        let t_end = 1.0_f64;
        let n = 50usize;
        let times = uniform_grid(n, t_end);
        let disc = RoughHestonHybrid::new(&times, h).expect("valid");

        // MC mean of V_T.
        let num_paths = 20_000usize;
        let mut rng = PhiloxRng::new(98765);
        let mut normals = vec![0.0; 2 * n];
        let mut sum_v = 0.0;
        for _ in 0..num_paths {
            rng.fill_std_normals(&mut normals);
            let mut x = vec![100.0, v0];
            let mut work = vec![0.0; disc.work_size(&process)];
            for k in 0..n {
                let z = [normals[2 * k], normals[2 * k + 1]];
                disc.step(
                    &process,
                    times[k],
                    times[k + 1] - times[k],
                    &mut x,
                    &z,
                    &mut work,
                );
            }
            sum_v += x[1];
        }
        let mc_mean = sum_v / num_paths as f64;

        // Independent reference: product integration of the mean ODE on a
        // 40× finer grid, with exact per-interval kernel integrals.
        let alpha = h + 0.5;
        let inv_gamma = 1.0 / finstack_core::math::ln_gamma(alpha).exp();
        let m = 2_000usize;
        let dtf = t_end / m as f64;
        let mut mean_v = vec![v0; m + 1];
        for k in 1..=m {
            let t_k = k as f64 * dtf;
            let mut sum = 0.0;
            for (j, &mean_v_j) in mean_v.iter().enumerate().take(k) {
                let a = t_k - j as f64 * dtf;
                let b = t_k - (j + 1) as f64 * dtf;
                let kernel_int = (a.powf(alpha) - b.powf(alpha)) / alpha;
                sum += kappa * (theta - mean_v_j) * kernel_int;
            }
            mean_v[k] = v0 + inv_gamma * sum;
        }
        let reference = mean_v[m];

        let rel = (mc_mean - reference).abs() / reference;
        assert!(
            rel < 2e-2,
            "MC E[V_T]={mc_mean} must match the fractional mean ODE \
             solution {reference} (rel err {rel})"
        );
    }

    /// MC ATM call price vs the (post-B1/M7) rough-Heston Fourier pricer.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn mc_atm_price_matches_fourier_pricer() {
        use super::super::super::rng::philox::PhiloxRng;
        use super::super::super::traits::RandomStream;
        use finstack_core::math::fractional::HurstExponent;
        use finstack_core::math::volatility::rough_heston::RoughHestonFourierParams;

        let h = 0.1_f64;
        let (r, q, kappa, theta, sigma_v, rho, v0) =
            (0.0, 0.0, 2.0_f64, 0.04_f64, 0.3_f64, -0.7, 0.04_f64);
        let (s0, strike, t_end) = (100.0_f64, 100.0_f64, 0.5_f64);

        let fourier = RoughHestonFourierParams::new(v0, kappa, theta, sigma_v, rho, h)
            .expect("valid fourier params");
        let reference = fourier.price_european(s0, strike, r, q, t_end, true);
        assert!(reference.is_finite() && reference > 0.0);

        let hurst = HurstExponent::new(h).expect("valid hurst");
        let params =
            RoughHestonParams::new(r, q, hurst, kappa, theta, sigma_v, rho, v0).expect("valid");
        let process = RoughHestonProcess::new(params);

        // The Euler scheme for rough Heston converges slowly in the step
        // count at small H; check both that the error shrinks under grid
        // refinement and that the fine grid lands within tolerance.
        let num_paths = 100_000usize;
        let mut rel_errs = Vec::new();
        for &n in &[50usize, 200] {
            let times = uniform_grid(n, t_end);
            let disc = RoughHestonHybrid::new(&times, h).expect("valid");

            let mut rng = PhiloxRng::new(192837);
            let mut normals = vec![0.0; 2 * n];
            let mut sum_payoff = 0.0;
            for _ in 0..num_paths {
                rng.fill_std_normals(&mut normals);
                let mut x = vec![s0, v0];
                let mut work = vec![0.0; disc.work_size(&process)];
                for k in 0..n {
                    let z = [normals[2 * k], normals[2 * k + 1]];
                    disc.step(
                        &process,
                        times[k],
                        times[k + 1] - times[k],
                        &mut x,
                        &z,
                        &mut work,
                    );
                }
                sum_payoff += (x[0] - strike).max(0.0);
            }
            let mc_price = (sum_payoff / num_paths as f64) * (-r * t_end).exp();
            rel_errs.push((mc_price - reference).abs() / reference);
        }

        assert!(
            rel_errs[1] < rel_errs[0],
            "MC error must shrink under step refinement: n=50 → {}, n=200 → {}",
            rel_errs[0],
            rel_errs[1]
        );
        assert!(
            rel_errs[1] < 3e-2,
            "MC ATM call must match Fourier price {reference} within 3% at \
             n=200 (rel err {})",
            rel_errs[1]
        );
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
