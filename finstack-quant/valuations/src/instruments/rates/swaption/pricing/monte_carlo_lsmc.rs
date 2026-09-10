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
//! use finstack_quant_valuations::instruments::rates::hw1f::RateExoticMcConfig;
//! use finstack_quant_valuations::instruments::rates::swaption::pricing::monte_carlo_lsmc::SwaptionLsmcPricer;
//! use finstack_quant_models::monte_carlo::process::ou::{HullWhite1FProcess, HullWhite1FParams};
//!
//! let hw_params = HullWhite1FParams::new(0.03, 0.01, 0.03).expect("valid parameters");
//! let hw_process = HullWhite1FProcess::new(hw_params);
//!
//! let pricer = SwaptionLsmcPricer::with_config(RateExoticMcConfig::default(), hw_process);
//! ```

use crate::instruments::rates::hw1f::mc_config::RateExoticMcConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;
use finstack_quant_models::monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use finstack_quant_models::monte_carlo::estimate::Estimate;
use finstack_quant_models::monte_carlo::pricer::basis::BasisFunctions;
use finstack_quant_models::monte_carlo::pricer::lsq::solve_least_squares;
use finstack_quant_models::monte_carlo::process::ou::HullWhite1FProcess;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::{Discretization, RandomStream};
use finstack_quant_models::monte_carlo::OnlineStats;
use finstack_quant_models::monte_carlo::TimeGrid;

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
    /// Monte Carlo configuration (`num_paths`, `seed` and `antithetic` are
    /// read here; the regression basis is supplied per call).
    config: RateExoticMcConfig,
    /// Hull-White process parameters
    hw_process: HullWhite1FProcess,
}

impl SwaptionLsmcPricer {
    /// Create a new pricer.
    ///
    /// # Arguments
    ///
    /// * `config` - Monte Carlo settings; `num_paths` total paths (halved into
    ///   `(Z, -Z)` pairs when `antithetic` is set), `seed` the Philox root seed.
    /// * `hw_process` - Hull-White 1F process with curve-calibrated θ(t).
    pub fn with_config(config: RateExoticMcConfig, hw_process: HullWhite1FProcess) -> Self {
        Self { config, hw_process }
    }

    /// Price a Bermudan swaption using a custom time grid with exact exercise indices.
    ///
    /// This variant allows precise alignment of the time grid with exercise dates,
    /// avoiding the rounding errors that can occur with uniform grids.
    ///
    /// # Arguments
    ///
    /// * `exercise_value` - Callback receiving the grid index and decimal short rate; returns the par swap rate, fixed-leg annuity and total immediate exercise value in `currency`.
    /// * `initial_short_rate` - Initial short rate r(0)
    /// * `time_grid` - Custom time grid (should include exercise dates exactly)
    /// * `exercise_indices` - Exact step indices for exercise dates
    /// * `basis` - Basis functions for regression
    /// * `currency` - Currency for result
    ///
    /// # Returns
    ///
    /// Statistical estimate of Bermudan swaption value
    #[allow(clippy::too_many_arguments)]
    pub fn price_bermudan_with_grid<B, E>(
        &self,
        exercise_value: E,
        initial_short_rate: f64,
        time_grid: &TimeGrid,
        exercise_indices: &[usize],
        basis: &B,
        currency: Currency,
    ) -> Result<MoneyEstimate>
    where
        B: BasisFunctions,
        E: Fn(usize, f64) -> Result<(f64, f64, f64)>,
    {
        // Step 1: Generate short rate paths using the custom time grid
        let paths = self.generate_rate_paths_with_grid(initial_short_rate, time_grid)?;

        // Step 2: Backward induction with exact exercise indices
        let values = self.backward_induction_swaption_grid(
            &paths,
            &exercise_value,
            exercise_indices,
            basis,
            time_grid,
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

        MoneyEstimate::from_estimate(estimate, currency)
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
        if self.config.antithetic {
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

    /// Perform backward induction for swaptions using a time grid.
    ///
    /// # Discounting Convention
    ///
    /// This pricer discounts realised cashflows by the **pathwise
    /// money-market numéraire** `B(t)` using the shared Hull-White bank-account
    /// accumulator, consistently with the short-rate dynamics used to
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
    /// the exercise-value callback to reconstruct the
    /// model-consistent zero-coupon bond prices `P(t, T)` — that use is a
    /// curve calibration input, not a payoff-discounting choice.
    ///
    /// See `lsmc.rs` for the flat-rate discounting approach.
    ///
    #[allow(clippy::too_many_arguments)]
    fn backward_induction_swaption_grid<B, E>(
        &self,
        paths: &[Vec<f64>],
        exercise_value: &E,
        exercise_steps: &[usize],
        basis: &B,
        time_grid: &TimeGrid,
    ) -> Result<Vec<f64>>
    where
        B: BasisFunctions,
        E: Fn(usize, f64) -> Result<(f64, f64, f64)>,
    {
        let num_paths = paths.len();

        // Pathwise money-market numéraire B(t) at every grid point, one
        // accumulator per simulated path. Discounting uses ratios of these
        // factors (the stochastic discount factor), not the deterministic
        // market discount curve.
        let bank_factors: Vec<Vec<f64>> = paths
            .iter()
            .map(|path| {
                crate::instruments::rates::hw1f::bank_account::accumulate_bank_factors(
                    path, time_grid,
                )
            })
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
        let mut regression_immediate = Vec::with_capacity(paths.len() / 2);
        let mut regression_indices = Vec::with_capacity(paths.len() / 2);

        for &exercise_step in &sorted_exercise_steps {
            if exercise_step >= paths[0].len() {
                continue;
            }

            // Clear buffers for this exercise date (reuse capacity)
            regression_x.clear();
            regression_annuity.clear();
            regression_y.clear();
            regression_indices.clear();
            regression_immediate.clear();

            for (i, path) in paths.iter().enumerate() {
                let r_t = path[exercise_step];

                let (swap_rate, annuity, immediate_value) = exercise_value(exercise_step, r_t)?;

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
                    regression_immediate.push(immediate_value);
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
                    let immediate_value = regression_immediate[j];
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
