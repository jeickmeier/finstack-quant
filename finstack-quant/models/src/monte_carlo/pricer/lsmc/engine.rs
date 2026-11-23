//! Backward-induction [`LsmcEngine`] and regression basis construction.

use super::super::super::results::MoneyEstimate;
use super::super::lsq::regression_coefficients_with_basis;
use super::pricer::{fill_gbm_antithetic_pair, fill_gbm_row};
use super::types::*;
use crate::monte_carlo::discretization::exact::ExactGbm;
use crate::monte_carlo::estimate::{Estimate, independent_estimator_stats};
use crate::monte_carlo::pricer::basis::{BasisFunctions, BasisKind, LsmcBasis, build_lsmc_basis};
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::math::time_grid::TimeGrid;

/// LSMC pricer for American/Bermudan options.
///
/// Uses backward induction with least-squares regression to estimate
/// continuation values and optimal exercise decisions.
#[derive(Debug, Clone)]
pub struct LsmcEngine {
    pub(super) config: LsmcConfig,
}

impl LsmcEngine {
    /// Create a new LSMC engine.
    ///
    /// # Arguments
    ///
    /// * `config` - Simulation paths, exercise grid, RNG and regression settings.
    pub fn new(config: LsmcConfig) -> Self {
        Self { config }
    }

    /// Borrow the validated LSMC configuration.
    pub fn config(&self) -> &LsmcConfig {
        &self.config
    }

    /// Convenience constructor for GBM American host bindings.
    ///
    /// Uses [`LsmcConfig::every_step`] so exercise occurs at each simulated
    /// step `0..=num_steps` (Bermudan on the grid). In-sample pricing applies
    /// immediate exercise as an intrinsic floor; two-pass pricing freezes
    /// the time-zero decision using training paths.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Independent path estimators; must be at least two.
    /// * `num_steps` - Time-grid steps; also the last exercise date.
    /// * `seed` - Root RNG seed for path generation.
    /// * `use_parallel` - Whether path generation uses the rayon pool.
    /// * `antithetic` - Pair each path with its sign-flipped counterpart.
    ///
    /// # Errors
    ///
    /// Returns an error if `num_paths` is less than two, `num_steps` is zero,
    /// or the path, step, or retained-storage limits in [`LsmcConfig::new`] fail.
    pub fn gbm_american(
        num_paths: usize,
        num_steps: usize,
        seed: u64,
        use_parallel: bool,
        antithetic: bool,
    ) -> Result<Self> {
        Ok(Self::new(
            LsmcConfig::every_step(num_paths, num_steps)?
                .with_seed(seed)
                .with_parallel(use_parallel)
                .with_antithetic(antithetic),
        ))
    }

    /// Price a Bermudan-style option on the configured exercise grid.
    ///
    /// Early exercise is decided on `exercise_dates` (typically `1..=num_steps`).
    /// If index `0` is included, the average of path present values is floored at
    /// `exercise_value(initial_spot)`. If that intrinsic binds, the mean is
    /// replaced by the intrinsic value while stderr and sample standard
    /// deviation stay those of the unfloored sample. The 95% CI is the
    /// unfloored interval with its lower bound clamped to intrinsic.
    ///
    /// # Arguments
    ///
    /// * `inputs` - GBM process, initial spot, maturity in years, step count,
    ///   exercise payoff, regression basis and flat discount rate; see
    ///   [`LsmcInputs`].
    /// * `currency` - Currency for result
    ///
    /// # Returns
    ///
    /// Statistical estimate of the Bermudan value, including immediate exercise
    /// only when index `0` belongs to the exercise schedule.
    pub fn price<B>(&self, inputs: &LsmcInputs<'_, B>, currency: Currency) -> Result<MoneyEstimate>
    where
        B: BasisFunctions + ?Sized,
    {
        Ok(self.price_internal(inputs, currency, false)?.estimate)
    }

    pub(super) fn price_internal<B>(
        &self,
        inputs: &LsmcInputs<'_, B>,
        currency: Currency,
        diagnostics: bool,
    ) -> Result<LsmcResult>
    where
        B: BasisFunctions + ?Sized,
    {
        let &LsmcInputs {
            process,
            initial_spot,
            time_to_maturity,
            num_steps,
            exercise,
            ..
        } = inputs;
        let paths = self.generate_paths_with_seed(
            process,
            initial_spot,
            time_to_maturity,
            num_steps,
            self.config.seed,
        )?;

        let TrainingResult {
            values,
            exercise_boundary,
            ..
        } = self.train_exercise_policy(&paths, inputs, diagnostics)?;

        let intrinsic_floor = self
            .config
            .exercise_dates
            .contains(&0)
            .then(|| exercise.exercise_value(initial_spot));
        let estimate = self.summarize_present_values(&values, intrinsic_floor, currency)?;
        Ok(LsmcResult {
            estimate,
            exercise_boundary,
        })
    }

    /// Generate Monte Carlo paths with an explicit seed override.
    ///
    /// Used by the two-pass API to draw an independent path set for out-of-sample
    /// pricing while reusing the configuration's path count and parallelism.
    pub(super) fn generate_paths_with_seed(
        &self,
        process: &GbmProcess,
        initial_spot: f64,
        time_to_maturity: f64,
        num_steps: usize,
        seed: u64,
    ) -> Result<PathMatrix> {
        if !initial_spot.is_finite() || initial_spot <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "LSMC initial_spot must be positive and finite, got {initial_spot}"
            )));
        }
        let path_values =
            checked_path_values(self.config.num_paths, num_steps, self.config.antithetic)?;
        let time_grid = TimeGrid::uniform(time_to_maturity, num_steps)?;
        let rng = PhiloxRng::new(seed);
        let disc = ExactGbm::new();
        let stride = num_steps + 1;
        let antithetic = self.config.antithetic;
        let mut data = vec![0.0; path_values];
        let chunk = if antithetic { 2 * stride } else { stride };
        let fill_chunk = |(path_id, rows): (usize, &mut [f64])| {
            let mut path_rng = rng.substream(path_id as u64);
            if antithetic {
                fill_gbm_antithetic_pair(
                    &disc,
                    process,
                    initial_spot,
                    &time_grid,
                    num_steps,
                    &mut path_rng,
                    rows.split_at_mut(stride),
                )
            } else {
                fill_gbm_row(
                    &disc,
                    process,
                    initial_spot,
                    &time_grid,
                    num_steps,
                    &mut path_rng,
                    rows,
                )
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        if self.config.use_parallel {
            use rayon::prelude::*;
            data.par_chunks_mut(chunk)
                .enumerate()
                .try_for_each(fill_chunk)?;
            return Ok(PathMatrix { data, stride });
        }

        data.chunks_mut(chunk)
            .enumerate()
            .try_for_each(fill_chunk)?;
        Ok(PathMatrix { data, stride })
    }

    /// Average antithetic pairs and optionally apply the in-sample intrinsic floor.
    ///
    /// When the floor binds, stderr and sample standard deviation are kept
    /// from the unfloored sample. The published CI is the unfloored interval
    /// with its lower bound clamped to `intrinsic`.
    pub(super) fn summarize_present_values(
        &self,
        path_pvs: &[f64],
        intrinsic_floor: Option<f64>,
        currency: Currency,
    ) -> finstack_quant_core::Result<MoneyEstimate> {
        let stats = independent_estimator_stats(path_pvs, self.config.antithetic)?;
        let mut estimate = Estimate::from_stats(&stats)?;
        let sample_mean = stats.get_mean()?.unwrap_or(f64::NAN);
        if let Some(intrinsic) = intrinsic_floor.filter(|&value| sample_mean < value) {
            let (lo, hi) = estimate.ci_95;
            let lower = lo.max(intrinsic);
            estimate.mean = intrinsic;
            estimate.ci_95 = (lower, hi.max(lower));
        }
        if self.config.antithetic {
            estimate = estimate.with_num_simulated_paths(2 * stats.get_count());
        }

        MoneyEstimate::from_estimate(estimate, currency)
    }

    /// Two-pass step 1: fit a frozen exercise policy on a training path set.
    ///
    /// Generates `num_paths` training paths with the configured seed, runs
    /// backward induction, and records the per-exercise-date regression
    /// coefficients without computing a price. Use [`Self::price_with_policy`]
    /// to apply the returned policy to a fresh, independent path set.
    ///
    /// # Errors
    ///
    /// Returns an error when path generation or any regression solve fails.
    ///
    /// # Arguments
    ///
    /// * `inputs` - GBM process, initial spot, maturity in years, step count,
    ///   exercise payoff, regression basis and flat discount rate of the
    ///   training run; see [`LsmcInputs`].
    pub fn fit_exercise_policy<B>(&self, inputs: &LsmcInputs<'_, B>) -> Result<ExercisePolicy>
    where
        B: BasisFunctions + ?Sized,
    {
        let (policy, _) = self.fit_exercise_policy_internal(inputs, false)?;
        Ok(policy)
    }

    pub(super) fn fit_exercise_policy_internal<B>(
        &self,
        inputs: &LsmcInputs<'_, B>,
        diagnostics: bool,
    ) -> Result<(ExercisePolicy, Vec<ExerciseBoundaryPoint>)>
    where
        B: BasisFunctions + ?Sized,
    {
        let &LsmcInputs {
            process,
            initial_spot,
            time_to_maturity,
            num_steps,
            ..
        } = inputs;
        let paths = self.generate_paths_with_seed(
            process,
            initial_spot,
            time_to_maturity,
            num_steps,
            self.config.seed,
        )?;
        let TrainingResult {
            policy,
            exercise_boundary,
            ..
        } = self.train_exercise_policy(&paths, inputs, diagnostics)?;
        Ok((policy, exercise_boundary))
    }

    /// Two-pass step 2: price using a frozen [`ExercisePolicy`] on independent paths.
    ///
    /// `pricing_seed` selects the RNG seed used to draw the pricing path set.
    /// It must differ from the seed that produced `policy` to obtain an
    /// out-of-sample (unbiased) estimate; passing the same seed reproduces the
    /// in-sample result.
    ///
    /// `num_steps` and `basis.num_basis()` must match the values used to fit
    /// the policy. The time-zero exercise decision is also frozen in `policy`;
    /// the independent pricing sample never selects immediate exercise or
    /// floors the estimated continuation value at intrinsic.
    ///
    /// # Errors
    ///
    /// Returns an error if `num_steps` or basis size disagree with the policy
    /// or if path generation fails.
    ///
    /// # Arguments
    ///
    /// * `inputs` - GBM process, initial spot, maturity in years, step count,
    ///   exercise payoff, regression basis and flat discount rate of the
    ///   pricing run; `num_steps` and the basis size must match `policy`. See
    ///   [`LsmcInputs`].
    /// * `policy` - Frozen exercise policy from [`Self::fit_exercise_policy`].
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    /// * `pricing_seed` - Deterministic random seed used to reproduce Monte Carlo paths.
    pub fn price_with_policy<B>(
        &self,
        inputs: &LsmcInputs<'_, B>,
        policy: &ExercisePolicy,
        currency: Currency,
        pricing_seed: u64,
    ) -> Result<MoneyEstimate>
    where
        B: BasisFunctions + ?Sized,
    {
        let &LsmcInputs {
            process,
            initial_spot,
            time_to_maturity,
            num_steps,
            exercise,
            basis,
            discount_rate,
        } = inputs;
        if !discount_rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "LSMC discount_rate must be finite, got {discount_rate}"
            )));
        }
        if policy.num_steps != num_steps {
            return Err(finstack_quant_core::Error::Validation(format!(
                "ExercisePolicy num_steps ({}) does not match pricing num_steps ({})",
                policy.num_steps, num_steps
            )));
        }
        if policy.num_basis != basis.num_basis() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "ExercisePolicy num_basis ({}) does not match basis size ({})",
                policy.num_basis,
                basis.num_basis()
            )));
        }

        let paths = self.generate_paths_with_seed(
            process,
            initial_spot,
            time_to_maturity,
            num_steps,
            pricing_seed,
        )?;

        let values = self.apply_policy_to_paths(
            &paths,
            exercise,
            basis,
            policy,
            PolicyTiming {
                discount_rate,
                time_to_maturity,
                num_steps,
            },
        );

        self.summarize_present_values(&values, None, currency)
    }

    /// Convenience: run the full two-pass workflow with disjoint seeds.
    ///
    /// Fits an exercise policy on a training run seeded with the pricer's
    /// configured seed, then prices on a fresh run seeded with `pricing_seed`.
    /// Returns an unbiased estimate of the fitted policy's value, including
    /// its time-zero decision. A suboptimal policy can underprice the optimal
    /// Bermudan value. Equivalent to
    /// calling [`Self::fit_exercise_policy`] followed by
    /// [`Self::price_with_policy`].
    ///
    /// # Errors
    ///
    /// Returns an error if `pricing_seed == self.config.seed` (the two passes
    /// would share paths and the result would be biased), or if either pass
    /// fails.
    ///
    /// # Arguments
    ///
    /// * `inputs` - GBM process, initial spot, maturity in years, step count,
    ///   exercise payoff, regression basis and flat discount rate shared by
    ///   both passes; see [`LsmcInputs`].
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    /// * `pricing_seed` - Deterministic random seed used to reproduce Monte Carlo paths.
    pub fn price_unbiased<B>(
        &self,
        inputs: &LsmcInputs<'_, B>,
        currency: Currency,
        pricing_seed: u64,
    ) -> Result<MoneyEstimate>
    where
        B: BasisFunctions + ?Sized,
    {
        Ok(self
            .price_unbiased_internal(inputs, currency, pricing_seed, false)?
            .estimate)
    }

    pub(super) fn price_unbiased_internal<B>(
        &self,
        inputs: &LsmcInputs<'_, B>,
        currency: Currency,
        pricing_seed: u64,
        diagnostics: bool,
    ) -> Result<LsmcResult>
    where
        B: BasisFunctions + ?Sized,
    {
        if pricing_seed == self.config.seed {
            return Err(finstack_quant_core::Error::Validation(
                "price_unbiased requires pricing_seed != configured training seed; \
                 sharing paths between regression fitting and pricing reintroduces in-sample bias"
                    .to_string(),
            ));
        }

        let (policy, exercise_boundary) = self.fit_exercise_policy_internal(inputs, diagnostics)?;
        let estimate = self.price_with_policy(inputs, &policy, currency, pricing_seed)?;
        Ok(LsmcResult {
            estimate,
            exercise_boundary,
        })
    }

    /// Fit continuation coefficients and retain the training-path values.
    ///
    /// # Discounting Convention
    ///
    /// Uses exponential discounting with a flat rate: `exp(-r * t)`.
    /// Contrast with Swaption LSMC (`swaption_lsmc.rs`), which uses discount
    /// factors from a yield curve (`df_t / df_0`).
    pub(super) fn train_exercise_policy<B>(
        &self,
        paths: &PathMatrix,
        inputs: &LsmcInputs<'_, B>,
        diagnostics: bool,
    ) -> Result<TrainingResult>
    where
        B: BasisFunctions + ?Sized,
    {
        let &LsmcInputs {
            exercise,
            basis,
            discount_rate,
            time_to_maturity,
            num_steps,
            ..
        } = inputs;
        if !discount_rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "LSMC discount_rate must be finite, got {discount_rate}"
            )));
        }
        if self.config.exercise_dates.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "exercise_dates must have at least one element".to_string(),
            ));
        }
        if let Some(&bad) = self.config.exercise_dates.iter().find(|&&d| d > num_steps) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "exercise_dates contain {bad} which exceeds num_steps={num_steps}; each date \
                 must satisfy 0 <= date <= num_steps"
            )));
        }
        let num_paths = paths.num_paths();
        let dt = time_to_maturity / num_steps as f64;

        let mut cashflows = vec![0.0; num_paths];
        let mut exercise_times = vec![time_to_maturity; num_paths];
        for (i, cf) in cashflows.iter_mut().enumerate() {
            *cf = exercise.exercise_value(paths.row(i)[num_steps]);
        }

        let mut sorted_exercise_dates = self.config.exercise_dates.clone();
        sorted_exercise_dates.sort_unstable();
        sorted_exercise_dates.dedup();
        sorted_exercise_dates.reverse();

        let mut regression_x: Vec<f64> = Vec::with_capacity(num_paths / 2);
        let mut regression_y: Vec<f64> = Vec::with_capacity(num_paths / 2);
        let mut regression_indices: Vec<usize> = Vec::with_capacity(num_paths / 2);
        let mut basis_vals = vec![0.0; basis.num_basis()];
        let mut coefficients_by_date: Vec<(usize, Vec<f64>)> = Vec::new();
        let mut exercise_boundary = Vec::new();

        for &exercise_step in &sorted_exercise_dates {
            if exercise_step == 0 || exercise_step >= num_steps {
                continue;
            }
            let t = exercise_step as f64 * dt;

            regression_x.clear();
            regression_y.clear();
            regression_indices.clear();

            for i in 0..num_paths {
                let path = paths.row(i);
                let spot = path[exercise_step];
                let immediate = exercise.exercise_value(spot);
                if immediate > 0.0 {
                    let time_to_cashflow = exercise_times[i] - t;
                    let discounted_cf = cashflows[i] * (-discount_rate * time_to_cashflow).exp();
                    regression_x.push(spot);
                    regression_y.push(discounted_cf);
                    regression_indices.push(i);
                }
            }

            if regression_x.len() <= basis.num_basis() + 10 {
                tracing::debug!(
                    exercise_step,
                    itm_paths = regression_x.len(),
                    "LSMC fit_exercise_policy: insufficient ITM paths, skipping date"
                );
                if diagnostics {
                    exercise_boundary.push(ExerciseBoundaryPoint {
                        step: exercise_step,
                        time: t,
                        itm_paths: regression_x.len(),
                        exercised_paths: 0,
                        critical_spot: None,
                        continuation_coeffs: Vec::new(),
                    });
                }
                continue;
            }
            match regression_coefficients_with_basis(&regression_x, &regression_y, basis) {
                Ok(coeffs) => {
                    // Subsequent earlier-date regressions must see the updated Y.
                    let mut exercised_paths = 0;
                    for &i in &regression_indices {
                        let spot = paths.row(i)[exercise_step];
                        basis.evaluate(spot, &mut basis_vals);
                        let mut continuation = 0.0;
                        for k in 0..coeffs.len() {
                            continuation += coeffs[k] * basis_vals[k];
                        }
                        let immediate = exercise.exercise_value(spot);
                        if immediate > continuation {
                            cashflows[i] = immediate;
                            exercise_times[i] = t;
                            exercised_paths += 1;
                        }
                    }
                    if diagnostics {
                        exercise_boundary.push(ExerciseBoundaryPoint {
                            step: exercise_step,
                            time: t,
                            itm_paths: regression_x.len(),
                            exercised_paths,
                            critical_spot: critical_spot(
                                &mut regression_x,
                                exercise,
                                basis,
                                &coeffs,
                                &mut basis_vals,
                            ),
                            continuation_coeffs: coeffs.clone(),
                        });
                    }
                    coefficients_by_date.push((exercise_step, coeffs));
                }
                Err(err) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "LSMC regression failed at step {exercise_step} with {} ITM paths: {err}",
                        regression_x.len()
                    )));
                }
            }
        }

        coefficients_by_date.sort_by_key(|(step, _)| *step);
        exercise_boundary.reverse();

        let present_values: Vec<f64> = cashflows
            .iter()
            .zip(&exercise_times)
            .map(|(&cashflow, &time)| cashflow * (-discount_rate * time).exp())
            .collect();
        let mut continuation = OnlineStats::new();
        for &value in &present_values {
            continuation.update(value)?;
        }
        let exercise_at_start = self.config.exercise_dates.contains(&0)
            && continuation
                .get_mean()?
                .is_some_and(|mean| exercise.exercise_value(paths.row(0)[0]) >= mean);

        Ok(TrainingResult {
            values: present_values,
            policy: ExercisePolicy {
                exercise_at_start,
                coefficients_by_date,
                num_basis: basis.num_basis(),
                num_steps,
            },
            exercise_boundary,
        })
    }

    /// Apply a frozen exercise policy forward in time on independent paths.
    ///
    /// Walks each path step by step, exercising at the first interior date
    /// where `immediate > continuation = β · basis(spot)`; otherwise the path
    /// receives the terminal European payoff. This forward sweep cannot reuse
    /// the path-set's own discounted cashflows (those would inject in-sample
    /// bias), which is the whole point of the two-pass scheme.
    pub(super) fn apply_policy_to_paths<B>(
        &self,
        paths: &PathMatrix,
        exercise: &AmericanExercise,
        basis: &B,
        policy: &ExercisePolicy,
        timing: PolicyTiming,
    ) -> Vec<f64>
    where
        B: BasisFunctions + ?Sized,
    {
        let dt = timing.time_to_maturity / timing.num_steps as f64;

        let mut basis_vals = vec![0.0; basis.num_basis()];
        let mut present_values = Vec::with_capacity(paths.num_paths());

        for i in 0..paths.num_paths() {
            let path = paths.row(i);
            if policy.exercise_at_start {
                present_values.push(exercise.exercise_value(path[0]));
                continue;
            }
            let mut exercised = false;
            let mut path_pv = 0.0;

            for (step, coeffs) in &policy.coefficients_by_date {
                let s = *step;
                if s == 0 || s >= timing.num_steps {
                    continue;
                }
                let spot = path[s];
                let immediate = exercise.exercise_value(spot);
                if immediate <= 0.0 {
                    continue;
                }
                basis.evaluate(spot, &mut basis_vals);
                let mut continuation = 0.0;
                for k in 0..coeffs.len() {
                    continuation += coeffs[k] * basis_vals[k];
                }
                if immediate > continuation {
                    let t = s as f64 * dt;
                    path_pv = immediate * (-timing.discount_rate * t).exp();
                    exercised = true;
                    break;
                }
            }

            if !exercised {
                let terminal = exercise.exercise_value(path[timing.num_steps]);
                path_pv = terminal * (-timing.discount_rate * timing.time_to_maturity).exp();
            }

            present_values.push(path_pv);
        }

        present_values
    }
}

pub(super) fn lsmc_basis(kind: BasisKind, degree: usize, strike: f64) -> Result<LsmcBasis> {
    build_lsmc_basis(kind, degree, strike)
}
