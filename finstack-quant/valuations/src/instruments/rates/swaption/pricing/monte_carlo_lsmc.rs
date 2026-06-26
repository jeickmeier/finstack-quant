//! Longstaff-Schwartz Monte Carlo pricer for Bermudan swaptions.
//!
//! Extends the LSMC framework to price Bermudan swaptions where exercise decisions
//! depend on forward swap rates computed from Hull-White short rate simulations.
//!
//! # Features
//!
//! - Hull-White 1-factor short rate simulation with exact discretization
//! - Longstaff-Schwartz backward induction with optimal exercise decisions
//! - Variance reduction via antithetic variates
//! - Polynomial and Laguerre basis functions for regression
//!
//! # Usage
//!
//! ```text
//! use finstack_quant_valuations::instruments::rates::swaption::pricing::monte_carlo_lsmc::{
//!     SwaptionLsmcPricer, SwaptionLsmcConfig,
//! };
//! use finstack_quant_monte_carlo::pricer::basis::PolynomialBasis;
//! use finstack_quant_monte_carlo::process::ou::{HullWhite1FProcess, HullWhite1FParams};
//!
//! let hw_params = HullWhite1FParams::new(0.03, 0.01, 0.03);
//! let hw_process = HullWhite1FProcess::new(hw_params);
//! let config = SwaptionLsmcConfig::default();
//!
//! let pricer = SwaptionLsmcPricer::with_config(config, hw_process);
//! ```

use super::monte_carlo_payoff::{BermudanSwaptionPayoff, SwaptionType};
use super::swap_rate_utils::{ForwardSwapRate, HullWhiteBondPrice};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;
use finstack_quant_monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use finstack_quant_monte_carlo::estimate::Estimate;
use finstack_quant_monte_carlo::online_stats::OnlineStats;
use finstack_quant_monte_carlo::pricer::basis::BasisFunctions;
use finstack_quant_monte_carlo::pricer::lsmc::LsmcConfig;
use finstack_quant_monte_carlo::pricer::lsq::solve_least_squares;
use finstack_quant_monte_carlo::process::ou::HullWhite1FProcess;
use finstack_quant_monte_carlo::results::MoneyEstimate;
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::time_grid::TimeGrid;
use finstack_quant_monte_carlo::traits::{Discretization, RandomStream};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for Bermudan swaption LSMC pricing.
///
/// # Default Values
///
/// | Parameter | Default | Description |
/// |-----------|---------|-------------|
/// | num_paths | 50,000 | Number of Monte Carlo paths |
/// | seed | 42 | Random seed for reproducibility |
/// | basis_degree | 3 | Polynomial degree for regression |
/// | antithetic | true | Use antithetic variates |
#[derive(Debug, Clone)]
pub struct SwaptionLsmcConfig {
    /// Number of Monte Carlo paths.
    ///
    /// More paths improve accuracy but increase computation time.
    /// Typical values: 10,000 (fast), 50,000 (standard), 100,000+ (high precision)
    pub num_paths: usize,

    /// Random seed for reproducibility.
    ///
    /// Using the same seed produces identical results across runs.
    pub seed: u64,

    /// Polynomial degree for basis functions in regression.
    ///
    /// Higher degrees can capture more complex continuation value surfaces
    /// but may overfit with limited ITM paths.
    /// Typical values: 2-4
    pub basis_degree: usize,

    /// Use antithetic variates for variance reduction.
    ///
    /// Generates (Z, -Z) path pairs which reduces variance by exploiting
    /// negative correlation between paired paths.
    pub antithetic: bool,
}

impl Default for SwaptionLsmcConfig {
    fn default() -> Self {
        let defaults = &finstack_quant_monte_carlo::registry::embedded_defaults_or_panic()
            .rust
            .swaption_lsmc;
        Self {
            num_paths: defaults.num_paths,
            seed: defaults.seed,
            basis_degree: defaults.basis_degree,
            antithetic: defaults.antithetic,
        }
    }
}

impl SwaptionLsmcConfig {
    /// Create a new configuration with specified parameters.
    pub fn new(num_paths: usize, seed: u64) -> Self {
        Self {
            num_paths,
            seed,
            ..Default::default()
        }
    }

    /// Set basis function degree.
    pub fn with_basis_degree(mut self, degree: usize) -> Self {
        self.basis_degree = degree;
        self
    }

    /// Enable/disable antithetic variates.
    pub fn with_antithetic(mut self, enabled: bool) -> Self {
        self.antithetic = enabled;
        self
    }

    /// Convert to internal LsmcConfig.
    pub fn to_lsmc_config(&self) -> LsmcConfig {
        LsmcConfig {
            num_paths: self.num_paths,
            seed: self.seed,
            exercise_dates: Vec::new(),
            use_parallel: false, // LSMC is complex, default to serial
        }
    }

    /// Build a time grid that includes all exercise dates exactly.
    ///
    /// Creates a grid with exercise dates as exact grid points, plus
    /// optional refinement points between them.
    ///
    /// # Arguments
    ///
    /// * `exercise_times` - Exercise times in years (sorted)
    /// * `maturity` - Final maturity time
    /// * `min_steps_between` - Minimum steps between grid points (default: 1)
    pub fn build_exercise_aligned_grid(
        exercise_times: &[f64],
        maturity: f64,
        min_steps_between: usize,
    ) -> Result<(TimeGrid, Vec<usize>)> {
        let mut times = vec![0.0];
        let mut exercise_indices = Vec::with_capacity(exercise_times.len());

        for &ex_time in exercise_times {
            if ex_time <= 0.0 || ex_time > maturity {
                continue;
            }

            // Add refinement points between last time and this exercise date
            // times is initialized with vec![0.0] and only grows, so last() always succeeds
            let last_time = times[times.len() - 1];
            if min_steps_between > 0 && ex_time > last_time + 1e-10 {
                let dt = (ex_time - last_time) / (min_steps_between + 1) as f64;
                for i in 1..=min_steps_between {
                    let t = last_time + dt * i as f64;
                    if (t - ex_time).abs() > 1e-10 {
                        times.push(t);
                    }
                }
            }

            // Add the exercise date exactly
            let current_last = times[times.len() - 1];
            if (ex_time - current_last).abs() > 1e-10 {
                times.push(ex_time);
            }
            exercise_indices.push(times.len() - 1);
        }

        // Add maturity if not already present
        let final_last = times[times.len() - 1];
        if (final_last - maturity).abs() > 1e-10 {
            times.push(maturity);
        }

        let grid = TimeGrid::from_times(times)?;
        Ok((grid, exercise_indices))
    }
}

// ============================================================================
// Swaption-Specific Basis Functions
// ============================================================================

/// Extended basis functions including annuity.
///
/// Basis: {1, S, S², ..., A, S×A}
///
/// Including the annuity as a state variable can improve regression quality
/// for swaptions where both rate level and annuity affect option value.
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct ExtendedSwaptionBasis {
    /// Polynomial degree for swap rate
    degree: usize,
    /// Include annuity as additional basis
    include_annuity: bool,
}

#[cfg(test)]
impl ExtendedSwaptionBasis {
    /// Create extended basis with annuity terms.
    pub fn new(degree: usize, include_annuity: bool) -> Self {
        Self {
            degree,
            include_annuity,
        }
    }

    fn evaluate_with_annuity(&self, swap_rate: f64, annuity: f64, out: &mut [f64]) {
        debug_assert_eq!(out.len(), self.num_basis());

        let mut power = 1.0;
        for basis in out.iter_mut().take(self.degree + 1) {
            *basis = power;
            power *= swap_rate;
        }

        if self.include_annuity {
            out[self.degree + 1] = annuity;
            out[self.degree + 2] = swap_rate * annuity;
        }
    }
}

#[cfg(test)]
impl BasisFunctions for ExtendedSwaptionBasis {
    fn num_basis(&self) -> usize {
        let base = self.degree + 1;
        if self.include_annuity {
            base + 2 // Add A and S×A terms
        } else {
            base
        }
    }

    fn evaluate(&self, swap_rate: f64, out: &mut [f64]) {
        self.evaluate_with_annuity(swap_rate, 1.0, out);
    }

    fn evaluate_with_aux(&self, state: f64, aux: Option<f64>, out: &mut [f64]) {
        self.evaluate_with_annuity(state, aux.unwrap_or(1.0), out);
    }
}

fn regression_with_aux_basis<B: BasisFunctions>(
    swap_rates: &[f64],
    aux_values: &[f64],
    continuation_values: &[f64],
    basis: &B,
) -> Result<Vec<f64>> {
    debug_assert_eq!(swap_rates.len(), aux_values.len());
    debug_assert_eq!(swap_rates.len(), continuation_values.len());

    let n = swap_rates.len();
    let k = basis.num_basis();
    let mut design = vec![0.0; n * k];
    let mut basis_vals = vec![0.0; k];

    for i in 0..n {
        basis.evaluate_with_aux(swap_rates[i], Some(aux_values[i]), &mut basis_vals);
        for j in 0..k {
            design[i * k + j] = basis_vals[j];
        }
    }

    let coeffs = solve_least_squares(&design, continuation_values, n, k)?;

    let mut predictions = vec![0.0; n];
    for i in 0..n {
        basis.evaluate_with_aux(swap_rates[i], Some(aux_values[i]), &mut basis_vals);
        predictions[i] = basis_vals
            .iter()
            .zip(coeffs.iter())
            .map(|(basis_val, coeff)| basis_val * coeff)
            .sum();
    }

    Ok(predictions)
}

/// LSMC pricer for Bermudan swaptions.
///
/// Uses backward induction with least-squares regression, similar to equity LSMC,
/// but computes exercise values from forward swap rates instead of spot prices.
///
/// # Features
///
/// - Hull-White 1F short rate simulation
/// - Polynomial basis functions for regression
/// - Optional antithetic variates for variance reduction
pub struct SwaptionLsmcPricer {
    /// Internal LSMC configuration
    config: LsmcConfig,
    /// Hull-White process parameters
    hw_process: HullWhite1FProcess,
    /// Extended configuration
    swaption_config: SwaptionLsmcConfig,
}

impl SwaptionLsmcPricer {
    /// Create a new pricer with full configuration.
    pub fn with_config(
        swaption_config: SwaptionLsmcConfig,
        hw_process: HullWhite1FProcess,
    ) -> Self {
        let config = swaption_config.to_lsmc_config();
        Self {
            config,
            hw_process,
            swaption_config,
        }
    }

    /// Price a Bermudan swaption using a custom time grid with exact exercise indices.
    ///
    /// This variant allows precise alignment of the time grid with exercise dates,
    /// avoiding the rounding errors that can occur with uniform grids.
    ///
    /// # Arguments
    ///
    /// * `payoff` - Bermudan swaption payoff specification
    /// * `initial_short_rate` - Initial short rate r(0)
    /// * `time_grid` - Custom time grid (should include exercise dates exactly)
    /// * `exercise_indices` - Exact step indices for exercise dates
    /// * `basis` - Basis functions for regression
    /// * `discount_curve_fn` - Function to get discount factors DF(t) for time t
    /// * `currency` - Currency for result
    ///
    /// # Returns
    ///
    /// Statistical estimate of Bermudan swaption value
    #[allow(clippy::too_many_arguments)]
    pub fn price_bermudan_with_grid<B, F>(
        &self,
        payoff: &BermudanSwaptionPayoff,
        initial_short_rate: f64,
        time_grid: &TimeGrid,
        exercise_indices: &[usize],
        basis: &B,
        discount_curve_fn: F,
        currency: Currency,
    ) -> Result<MoneyEstimate>
    where
        B: BasisFunctions,
        F: Fn(f64) -> f64 + Send + Sync,
    {
        // Step 1: Generate short rate paths using the custom time grid
        let paths = self.generate_rate_paths_with_grid(initial_short_rate, time_grid)?;

        // Step 2: Backward induction with exact exercise indices
        let values = self.backward_induction_swaption_grid(
            &paths,
            payoff,
            exercise_indices,
            basis,
            time_grid,
            &discount_curve_fn,
        )?;

        // Step 3: Compute statistics
        let mut stats = OnlineStats::new();
        for &value in &values {
            stats.update(value);
        }

        let estimate = Estimate::new(
            stats.mean(),
            stats.stderr(),
            stats.confidence_interval(0.05),
            values.len(),
        )
        .with_std_dev(stats.std_dev());

        Ok(MoneyEstimate::from_estimate(estimate, currency))
    }

    /// Generate short rate paths using a custom time grid.
    ///
    /// If antithetic variates are enabled, generates paired paths (Z, -Z)
    /// which reduces variance through negative correlation.
    fn generate_rate_paths_with_grid(
        &self,
        initial_rate: f64,
        time_grid: &TimeGrid,
    ) -> Result<Vec<Vec<f64>>> {
        if self.swaption_config.antithetic {
            self.generate_antithetic_paths_with_grid(initial_rate, time_grid)
        } else {
            self.generate_standard_paths_with_grid(initial_rate, time_grid)
        }
    }

    /// Generate standard (non-antithetic) paths using a time grid.
    fn generate_standard_paths_with_grid(
        &self,
        initial_rate: f64,
        time_grid: &TimeGrid,
    ) -> Result<Vec<Vec<f64>>> {
        let disc = ExactHullWhite1F::new();
        let rng = PhiloxRng::new(self.config.seed);
        let num_steps = time_grid.num_steps();

        let mut paths = Vec::with_capacity(self.config.num_paths);

        for path_id in 0..self.config.num_paths {
            let mut path_rng = rng.substream(path_id as u64);
            let mut rate_path = Vec::with_capacity(num_steps + 1);
            let mut state = vec![initial_rate];
            let mut z = vec![0.0];
            let mut work = vec![];

            rate_path.push(initial_rate);

            for step in 0..num_steps {
                let t = time_grid.time(step);
                let dt = time_grid.dt(step);

                path_rng.fill_std_normals(&mut z);
                disc.step(&self.hw_process, t, dt, &mut state, &z, &mut work);

                rate_path.push(state[0]);
            }

            paths.push(rate_path);
        }

        Ok(paths)
    }

    /// Generate antithetic path pairs (Z, -Z) for variance reduction using a time grid.
    ///
    /// For each random draw Z, generates two paths:
    /// - Original path using Z
    /// - Antithetic path using -Z
    ///
    /// This exploits the negative correlation to reduce variance.
    fn generate_antithetic_paths_with_grid(
        &self,
        initial_rate: f64,
        time_grid: &TimeGrid,
    ) -> Result<Vec<Vec<f64>>> {
        let disc = ExactHullWhite1F::new();
        let rng = PhiloxRng::new(self.config.seed);
        let num_steps = time_grid.num_steps();

        // With antithetic, we need half the random numbers
        let num_pairs = self.config.num_paths / 2;
        let mut paths = Vec::with_capacity(self.config.num_paths);

        for pair_id in 0..num_pairs {
            let mut path_rng = rng.substream(pair_id as u64);

            // Generate all random draws for this pair in a single call. The
            // Box-Muller generator caches the second element of each pair in
            // `spare_normal`, so one length-`num_steps` fill yields exactly the
            // same sequence as `num_steps` length-1 fills did — but without the
            // per-step heap allocation.
            let mut z_draws: Vec<f64> = vec![0.0; num_steps];
            path_rng.fill_std_normals(&mut z_draws);

            // Reusable single-factor innovation buffer (HW1F: one factor).
            let mut z_step = [0.0_f64];

            // Original path using +Z
            let mut state_orig = vec![initial_rate];
            let mut rate_path_orig = Vec::with_capacity(num_steps + 1);
            rate_path_orig.push(initial_rate);

            let mut work = vec![];
            for (step, &z_val) in z_draws.iter().enumerate() {
                let t = time_grid.time(step);
                let dt = time_grid.dt(step);

                z_step[0] = z_val;
                disc.step(&self.hw_process, t, dt, &mut state_orig, &z_step, &mut work);
                rate_path_orig.push(state_orig[0]);
            }

            // Antithetic path using -Z
            let mut state_anti = vec![initial_rate];
            let mut rate_path_anti = Vec::with_capacity(num_steps + 1);
            rate_path_anti.push(initial_rate);

            for (step, &z_val) in z_draws.iter().enumerate() {
                let t = time_grid.time(step);
                let dt = time_grid.dt(step);

                z_step[0] = -z_val; // Negate the random draw
                disc.step(&self.hw_process, t, dt, &mut state_anti, &z_step, &mut work);
                rate_path_anti.push(state_anti[0]);
            }

            paths.push(rate_path_orig);
            paths.push(rate_path_anti);
        }

        // Handle odd number of paths
        if self.config.num_paths % 2 == 1 {
            let mut path_rng = rng.substream(num_pairs as u64);
            let mut state = vec![initial_rate];
            let mut rate_path = Vec::with_capacity(num_steps + 1);
            rate_path.push(initial_rate);

            let mut z = vec![0.0];
            let mut work = vec![];
            for step in 0..num_steps {
                let t = time_grid.time(step);
                let dt = time_grid.dt(step);

                path_rng.fill_std_normals(&mut z);
                disc.step(&self.hw_process, t, dt, &mut state, &z, &mut work);
                rate_path.push(state[0]);
            }
            paths.push(rate_path);
        }

        Ok(paths)
    }

    /// Accumulate the pathwise money-market numéraire `B(t)` along a
    /// simulated short-rate path.
    ///
    /// `B(t)` is the value of the money-market account
    /// `B(t) = exp(∫₀ᵗ r(s) ds)`, discretised on the simulation grid.
    ///
    /// The integral over each step is approximated with the **trapezoidal
    /// rule**:
    ///
    /// ```text
    /// B(t_0) = 1,
    /// B(t_{k+1}) = B(t_k) · exp( ½·(r(t_k) + r(t_{k+1}))·Δt_k )
    /// ```
    ///
    /// The exact-HW1F transition gives both endpoints `r(t_k)` and
    /// `r(t_{k+1})` of every interval, so the cheap left-endpoint Riemann sum
    /// `exp(r(t_k)·Δt_k)` it previously used left an avoidable O(Δt) bias in
    /// the discount factor — the short rate moves materially within a step on
    /// a coarse exercise-aligned grid. The trapezoidal rule is O(Δt²) and uses
    /// only the path values already simulated. (The fully exact integrated
    /// short rate needs the joint law of `(r(t), ∫r ds)`, which lives in the
    /// Monte-Carlo discretisation layer.)
    ///
    /// The returned vector has one entry per grid point (`num_steps + 1`).
    /// LSM continuation values and the final present value are discounted
    /// by *ratios* of these pathwise factors — the stochastic discount
    /// factor the Hull-White model produces — rather than by the
    /// deterministic market discount curve. Discounting by the
    /// deterministic curve would ignore the correlation between the
    /// stochastic discount factor and the swap payoff and bias the
    /// exercise boundary.
    fn accumulate_bank_factors(rate_path: &[f64], time_grid: &TimeGrid) -> Vec<f64> {
        // Shared trapezoidal accumulation, also used by the HW1F exotic
        // MC/LSMC harnesses so every short-rate MC discounts consistently.
        crate::instruments::rates::exotics_shared::bank_account::accumulate_bank_factors(
            rate_path, time_grid,
        )
    }

    /// Perform backward induction for swaptions using a time grid.
    ///
    /// # Discounting Convention
    ///
    /// This pricer discounts realised cashflows by the **pathwise
    /// money-market numéraire** `B(t)` (see [`accumulate_bank_factors`]),
    /// consistently with the Hull-White short-rate dynamics used to
    /// simulate the paths. A continuation value carried back from a future
    /// exercise step `t'` to the current step `t` is multiplied by the
    /// pathwise ratio `B(t) / B(t')`, and the time-0 present value is
    /// `X(t_exercise) / B(t_exercise)` (with `B(0) = 1`).
    ///
    /// Discounting instead by the deterministic market discount factor
    /// would replace the stochastic discount factor with its expectation
    /// `E[1/B(t)] ≈ DF(t)`, dropping the payoff/numéraire correlation that
    /// a short-rate model exists to capture and biasing the optimal
    /// exercise boundary.
    ///
    /// The market discount curve is still consulted inside
    /// [`HullWhiteBondPrice`] / [`ForwardSwapRate`] to reconstruct the
    /// model-consistent zero-coupon bond prices `P(t, T)` — that use is a
    /// curve calibration input, not a payoff-discounting choice.
    ///
    /// See `lsmc.rs` for the flat-rate discounting approach.
    ///
    /// [`accumulate_bank_factors`]: Self::accumulate_bank_factors
    #[allow(clippy::too_many_arguments)]
    fn backward_induction_swaption_grid<B, F>(
        &self,
        paths: &[Vec<f64>],
        payoff: &BermudanSwaptionPayoff,
        exercise_steps: &[usize],
        basis: &B,
        time_grid: &TimeGrid,
        discount_curve_fn: &F,
    ) -> Result<Vec<f64>>
    where
        B: BasisFunctions,
        F: Fn(f64) -> f64 + Send + Sync,
    {
        let num_paths = paths.len();
        let params = self.hw_process.params();

        // Pathwise money-market numéraire B(t) at every grid point, one
        // accumulator per simulated path. Discounting uses ratios of these
        // factors (the stochastic discount factor), not the deterministic
        // market discount curve.
        let bank_factors: Vec<Vec<f64>> = paths
            .iter()
            .map(|path| Self::accumulate_bank_factors(path, time_grid))
            .collect();

        // Cashflow tracking. `exercise_step` is the grid step of the
        // currently-optimal exercise decision for each path; it indexes
        // `bank_factors` to fetch the pathwise numéraire B(t_exercise).
        // Unexercised paths keep a zero cashflow, so their B(t_exercise)
        // is irrelevant — seed the step with 0 (B = 1) as a safe default.
        let mut cashflows = vec![0.0; num_paths];
        let mut exercise_step_of = vec![0usize; num_paths];

        // Initialize with terminal values (if not exercised, value is zero)
        // For swaptions, terminal value is zero if not exercised

        // Backward induction through exercise dates
        let mut sorted_exercise_steps = exercise_steps.to_vec();
        sorted_exercise_steps.sort_unstable();
        sorted_exercise_steps.reverse(); // Go backward

        // Pre-allocate regression buffers to avoid reallocations
        let mut regression_x = Vec::with_capacity(paths.len() / 2); // Swap rates
        let mut regression_annuity = Vec::with_capacity(paths.len() / 2);
        let mut regression_y = Vec::with_capacity(paths.len() / 2); // Discounted continuation values
        let mut regression_indices = Vec::with_capacity(paths.len() / 2);

        for &exercise_step in &sorted_exercise_steps {
            if exercise_step >= paths[0].len() - 1 {
                continue;
            }

            // Get exact time from grid instead of computing from step * dt
            let t = time_grid.time(exercise_step);

            // Clear buffers for this exercise date (reuse capacity)
            regression_x.clear();
            regression_annuity.clear();
            regression_y.clear();
            regression_indices.clear();

            for (i, path) in paths.iter().enumerate() {
                let r_t = path[exercise_step];

                // Compute forward swap rate
                let swap_rate = ForwardSwapRate::compute(
                    params,
                    r_t,
                    t,
                    &payoff.swap_schedule,
                    discount_curve_fn,
                );

                // Compute exercise value: (S(t) - K) * A(t) * N for payer
                let swap_value = match payoff.option_type {
                    SwaptionType::Payer => swap_rate - payoff.strike,
                    SwaptionType::Receiver => payoff.strike - swap_rate,
                };

                // Compute annuity for proper scaling
                let mut annuity = 0.0;
                for (j, &payment_time_j) in payoff.swap_schedule.payment_dates.iter().enumerate() {
                    if payment_time_j > t {
                        let p_j = HullWhiteBondPrice::bond_price(
                            params,
                            r_t,
                            t,
                            payment_time_j,
                            discount_curve_fn,
                        );
                        let tau_j = payoff.swap_schedule.accrual_fractions[j];
                        annuity += tau_j * p_j;
                    }
                }

                let immediate_value = swap_value.max(0.0) * annuity * payoff.notional;

                // Only regress on ITM paths
                if immediate_value > 1e-6 {
                    // Discount the realised future cashflow back to this
                    // exercise step by the PATHWISE numéraire ratio
                    // B(t) / B(t_future). Both B values are taken from the
                    // same path, so this is the stochastic discount factor
                    // the Hull-White model produces — not the deterministic
                    // market discount factor, which would drop the
                    // payoff/numéraire correlation.
                    let b_now = bank_factors[i][exercise_step];
                    let b_future = bank_factors[i][exercise_step_of[i]];
                    let discounted_cf = if b_future > 0.0 {
                        cashflows[i] * b_now / b_future
                    } else {
                        0.0
                    };

                    regression_x.push(swap_rate);
                    regression_annuity.push(annuity);
                    regression_y.push(discounted_cf);
                    regression_indices.push(i);
                }
            }

            // Perform regression if we have enough ITM paths
            if regression_x.len() > basis.num_basis() + 10 {
                let continuation_values = regression_with_aux_basis(
                    &regression_x,
                    &regression_annuity,
                    &regression_y,
                    basis,
                )?;

                // Exercise decision
                for (j, &i) in regression_indices.iter().enumerate() {
                    // Reuse the swap rate and annuity already computed in the
                    // regression-collection pass above. `regression_x` /
                    // `regression_annuity` were pushed in the same order as
                    // `regression_indices`, so index `j` corresponds to path
                    // `i`. This avoids re-running `ForwardSwapRate::compute`
                    // (bond prices + finite-difference forwards) and the annuity
                    // bond-price loop a second time per ITM path.
                    let swap_rate = regression_x[j];
                    let annuity = regression_annuity[j];

                    let swap_value = match payoff.option_type {
                        SwaptionType::Payer => swap_rate - payoff.strike,
                        SwaptionType::Receiver => payoff.strike - swap_rate,
                    };

                    let immediate_value = swap_value.max(0.0) * annuity * payoff.notional;
                    let continuation = continuation_values[j];

                    // Exercise if immediate value > continuation value.
                    // Record the exercise *step* so the final present value
                    // can fetch the pathwise numéraire B(t_exercise).
                    if immediate_value > continuation {
                        cashflows[i] = immediate_value;
                        exercise_step_of[i] = exercise_step;
                    }
                }
            }
        }

        // Discount all cashflows to present by the pathwise money-market
        // numéraire: PV = X(t_exercise) / B(t_exercise), with B(0) = 1.
        let mut present_values = vec![0.0; num_paths];
        for i in 0..num_paths {
            let b_exercise = bank_factors[i][exercise_step_of[i]];
            present_values[i] = if b_exercise > 0.0 {
                cashflows[i] / b_exercise
            } else {
                0.0
            };
        }

        Ok(present_values)
    }
}

#[cfg(test)]
mod tests {
    // Tests for swap rate utilities are now in swap_rate_utils.rs
    // This module focuses on testing the LSMC swaption pricer itself

    use super::ExtendedSwaptionBasis;
    use finstack_quant_monte_carlo::pricer::basis::BasisFunctions;

    #[test]
    fn extended_basis_uses_real_annuity_terms() {
        let basis = ExtendedSwaptionBasis::new(2, true);
        let mut values = vec![0.0; basis.num_basis()];

        basis.evaluate_with_annuity(0.05, 3.5, &mut values);

        assert_eq!(values[0], 1.0);
        assert_eq!(values[1], 0.05);
        assert_eq!(values[2], 0.05 * 0.05);
        assert_eq!(values[3], 3.5);
        assert_eq!(values[4], 0.05 * 3.5);
    }
}
