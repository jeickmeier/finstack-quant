//! Generic HW1F Longstaff-Schwartz MC pricer for callable rate exotics.
//!
//! The pricer drives a user-supplied [`ExerciseBoundaryPayoff`] along
//! simulated short-rate paths produced by an exact HW1F discretization.
//! A two-phase algorithm is used:
//!
//! 1. **Forward pass.** For each path the harness runs the full simulation
//!    and records per-path the deterministic PV reported by
//!    [`finstack_monte_carlo::traits::Payoff::value`], as well as the short-rate,
//!    exercise value, and inactive flag at each exercise date.
//! 2. **Backward pass.** Starting from maturity, the harness regresses
//!    continuation values via [`finstack_monte_carlo::pricer::lsq::solve_least_squares`] against the
//!    [`standard_basis`] (ITM + active paths only) and rolls the per-path
//!    cashflow vector back, overwriting `cashflow[p]` with the call value
//!    whenever exercise is optimal.
//! 3. **Aggregation.** The average of the per-path cashflows is reported
//!    as the LSMC PV estimate together with a 95% confidence interval.
//!
//! Product payoffs implement [`ExerciseBoundaryPayoff`] (a supertrait of
//! [`finstack_monte_carlo::traits::Payoff`]); the harness is entirely agnostic to the product-specific
//! cashflow logic.
//!
//! # In-sample upward bias and split-sample option
//!
//! Plain Longstaff-Schwartz regresses continuation values and prices the
//! callable on the **same** path set, which biases the reported PV *upward*
//! relative to the true callable value (the optimizer sees its own noise as
//! signal). The bias is typically modest for standard swaption/Bermudan
//! setups with `num_paths ≳ 10⁴` and the default basis, but grows with richer
//! bases and fewer paths.
//!
//! Setting [`RateExoticMcConfig::oos_lsmc`](crate::instruments::rates::exotics_shared::mc_config::RateExoticMcConfig::oos_lsmc)
//! to `true` switches to a split-sample estimator: even-indexed raw streams
//! drive the regression, odd-indexed streams are priced under that fitted
//! policy. The pricing half evaluates an out-of-sample (sub-optimal but
//! noise-independent) policy and is unbiased for that policy — yielding a
//! conservative lower-side complement to the in-sample upper-biased estimate.
//! Standard error rises by roughly √2 because only half the paths drive the
//! aggregation; combine the two bounds for an unbiased bracket.

use crate::instruments::rates::exotics_shared::exercise::ExerciseBoundaryPayoff;
use crate::instruments::rates::exotics_shared::mc_config::RateExoticMcConfig;
use finstack_core::currency::Currency;
use finstack_core::Result;
use finstack_monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use finstack_monte_carlo::online_stats::OnlineStats;
use finstack_monte_carlo::pricer::lsq::solve_least_squares;
use finstack_monte_carlo::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use finstack_monte_carlo::results::MoneyEstimate;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::time_grid::TimeGrid;
use finstack_monte_carlo::traits::{Discretization, PathState, RandomStream, StateKey};

/// Generic HW1F LSMC pricer for callable rate exotics.
pub struct RateExoticHw1fLsmcPricer {
    /// Fully-specified HW1F short-rate parameters: κ, σ, and the
    /// time-dependent mean-reversion level θ(t).
    ///
    /// The simulated short rate follows `dr_t = κ·(θ(t) - r_t)·dt + σ·dW_t`.
    /// θ(t) MUST be bootstrapped from the product's discount curve (see
    /// [`crate::instruments::rates::exotics_shared::calibrate_hw1f_params`])
    /// so the simulated short rate reprices the initial curve — a constant θ
    /// makes the process a plain Vasicek that mis-reprices any non-flat curve.
    pub process_params: HullWhite1FParams,
    /// Initial short rate r(0).
    pub r0: f64,
    /// Event (coupon/observation) times driving the payoff, strictly increasing.
    pub event_times: Vec<f64>,
    /// Exercise times — must be a subset of `event_times`.
    pub exercise_times: Vec<f64>,
    /// Call-price multiplier at each exercise date (typically 1.0 = par).
    pub call_prices: Vec<f64>,
    /// Notional for scaling the call payoff.
    pub notional: f64,
    /// Runtime Monte Carlo configuration.
    pub config: RateExoticMcConfig,
    /// Currency for the returned PV estimate.
    pub currency: Currency,
}

impl RateExoticHw1fLsmcPricer {
    /// Run the LSMC pricing: forward pass records path state, backward pass
    /// fits continuation values and applies optimal exercise.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `event_times` is empty or not strictly
    /// increasing, if `exercise_times` are not a subset of `event_times`,
    /// if `call_prices` length does not match `exercise_times`, or if the
    /// time-grid construction fails. Propagates errors from
    /// [`finstack_monte_carlo::pricer::lsq::solve_least_squares`].
    pub fn price<F, P>(&self, payoff_factory: F) -> Result<MoneyEstimate>
    where
        F: Fn() -> P + Sync,
        P: ExerciseBoundaryPayoff + 'static,
    {
        self.validate_inputs()?;

        let Some(&maturity) = self.event_times.last() else {
            return Err(finstack_core::Error::Validation(
                "RateExoticHw1fLsmcPricer requires at least one event time".into(),
            ));
        };

        let (grid, event_step_indices, exercise_event_indices) = build_grid_with_exercise_map(
            &self.event_times,
            &self.exercise_times,
            maturity,
            self.config.min_steps_between_events,
        )?;

        let process = HullWhite1FProcess::new(self.process_params.clone());
        let disc = ExactHullWhite1F;
        let num_steps = grid.num_steps();
        let work_size = disc.work_size(&process);
        let raw_paths = self.config.raw_stream_count();
        let multiplicity = if self.config.antithetic { 2 } else { 1 };
        let n_paths = self.config.effective_path_count();
        let n_ex = self.exercise_times.len();
        let base_rng = PhiloxRng::new(self.config.seed);
        let basis_payoff = payoff_factory();

        // Map exercise-date index -> position within event_step_indices.
        let exercise_event_pos = exercise_event_indices;

        let mut deterministic_pv = vec![0.0_f64; n_paths];
        let mut exercise_short_rates = vec![0.0_f64; n_paths * n_ex];
        let mut exercise_values = vec![0.0_f64; n_paths * n_ex];
        let mut exercise_inactive = vec![false; n_paths * n_ex];

        // Per-path scratch buffers hoisted out of the path loop; the
        // discretization step fully overwrites `work` and `z` each step,
        // so reusing them across paths is bit-identical to fresh allocations.
        let mut work = vec![0.0; work_size];
        let mut z = [0.0_f64; 1];

        let mut path_cursor: usize = 0;
        for path_id in 0..raw_paths {
            for anti in 0..multiplicity {
                let mut rng = base_rng.substream(path_id as u64);
                let mut r = self.r0;
                let mut payoff = payoff_factory();
                payoff.reset();
                let mut state = PathState::new(0, 0.0);
                state.set_key(StateKey::ShortRate, r);

                let mut next_event = 0usize;
                let mut next_exercise = 0usize;
                for step in 0..num_steps {
                    let t = grid.time(step);
                    let dt = grid.dt(step);
                    rng.fill_std_normals(&mut z);
                    if anti == 1 {
                        z[0] = -z[0];
                    }
                    disc.step(
                        &process,
                        t,
                        dt,
                        core::slice::from_mut(&mut r),
                        &z,
                        &mut work,
                    );

                    let t_next = grid.time(step + 1);
                    state.set_step_time(step + 1, t_next);
                    state.set_key(StateKey::ShortRate, r);

                    while next_event < event_step_indices.len()
                        && event_step_indices[next_event] == step + 1
                    {
                        payoff.on_event(&mut state);

                        // Record exercise-date state if this event is an exercise date.
                        if next_exercise < exercise_event_pos.len()
                            && exercise_event_pos[next_exercise] == next_event
                        {
                            let flat = path_cursor * n_ex + next_exercise;
                            exercise_short_rates[flat] = r;
                            exercise_values[flat] = payoff
                                .intrinsic_at(next_exercise, r, self.currency)
                                .amount();
                            exercise_inactive[flat] = payoff.is_path_inactive();
                            next_exercise += 1;
                        }
                        next_event += 1;
                    }
                }

                deterministic_pv[path_cursor] = payoff.value(self.currency).amount();
                path_cursor += 1;
            }
        }

        // -- Phase 2: backward LSMC induction ----------------------------------
        let mut cashflow = deterministic_pv.clone();

        // Split-sample partition. Path `p` belongs to raw stream `p / multiplicity`;
        // partitioning by stream parity keeps each antithetic pair together and
        // gives a deterministic, seed-stable train/price split. When
        // `oos_lsmc` is off, every path is treated as both train and price
        // (i.e. the classic in-sample Longstaff-Schwartz estimator).
        let oos = self.config.oos_lsmc;
        let is_train = |p: usize| !oos || (p / multiplicity).is_multiple_of(2);
        let is_price = |p: usize| !oos || !(p / multiplicity).is_multiple_of(2);

        // Regression scratch buffers hoisted out of the exercise-date loop;
        // cleared (not freed) per date so allocations are reused. Each date
        // fully repopulates them before use, so this is bit-identical.
        let mut active_paths: Vec<usize> = Vec::new();
        let mut active_basis: Vec<f64> = Vec::new();
        let mut active_continuation: Vec<f64> = Vec::new();

        for ex_idx in (0..n_ex).rev() {
            let t_ex = self.exercise_times[ex_idx];

            // Regress continuation over active paths, then apply issuer exercise.
            active_paths.clear();
            active_basis.clear();
            active_continuation.clear();
            let mut num_basis: usize = 0;

            for (p, &cf) in cashflow.iter().enumerate() {
                // In split-sample mode, only train paths feed the regression.
                // The fitted coefficients are still applied to every path below
                // so the backward rollback propagates correctly; aggregation
                // at the end then restricts to the pricing half.
                if !is_train(p) {
                    continue;
                }
                let flat = p * n_ex + ex_idx;
                if exercise_inactive[flat] {
                    continue;
                }
                let exercise_value = exercise_values[flat];
                if exercise_value <= 0.0 {
                    continue;
                }
                let r = exercise_short_rates[flat];
                let basis = basis_payoff.continuation_basis(ex_idx, t_ex, r);
                if num_basis == 0 {
                    num_basis = basis.len();
                }
                active_paths.push(p);
                active_basis.extend(basis);
                active_continuation.push(cf);
            }

            if num_basis == 0 {
                num_basis = basis_payoff.continuation_basis(ex_idx, t_ex, 0.0).len();
            }

            if active_paths.len() > num_basis + 2 {
                let coeffs = solve_least_squares(
                    &active_basis,
                    &active_continuation,
                    active_paths.len(),
                    num_basis,
                )?;
                // Apply the fitted rule to every path (in-sample mode: all
                // paths were also in `active_paths`; split-sample mode: price
                // paths get the rule out-of-sample). Recomputing basis is
                // marginally cheaper than carrying a path-id → row index map
                // and keeps the split-sample path uniform with the baseline.
                for (p, cf) in cashflow.iter_mut().enumerate() {
                    let flat = p * n_ex + ex_idx;
                    if exercise_inactive[flat] {
                        continue;
                    }
                    let exercise_value = exercise_values[flat];
                    if exercise_value <= 0.0 {
                        continue;
                    }
                    let r = exercise_short_rates[flat];
                    let basis = basis_payoff.continuation_basis(ex_idx, t_ex, r);
                    let cont_hat: f64 = basis.iter().zip(coeffs.iter()).map(|(b, c)| b * c).sum();
                    if exercise_value < cont_hat {
                        *cf = exercise_value;
                    }
                }
            } else {
                // Fallback: pathwise issuer exercise against realized cashflow.
                for (p, cf) in cashflow.iter_mut().enumerate() {
                    let flat = p * n_ex + ex_idx;
                    if exercise_inactive[flat] {
                        continue;
                    }
                    let exercise_value = exercise_values[flat];
                    if exercise_value > 0.0 && exercise_value < *cf {
                        *cf = exercise_value;
                    }
                }
            }
        }

        // -- Phase 3: aggregate ------------------------------------------------
        // In split-sample mode, only the pricing half (out-of-sample under the
        // train-fitted policy) contributes to the reported estimate.
        let mut stats = OnlineStats::new();
        for (p, &v) in cashflow.iter().enumerate() {
            if !is_price(p) {
                continue;
            }
            stats.update(v);
        }

        let n = stats.count().max(1) as f64;
        let mean = stats.mean();
        let stderr = stats.std_dev() / n.sqrt();
        let lo = mean - 1.96 * stderr;
        let hi = mean + 1.96 * stderr;
        Ok(MoneyEstimate {
            mean: finstack_core::money::Money::new(mean, self.currency),
            stderr,
            ci_95: (
                finstack_core::money::Money::new(lo, self.currency),
                finstack_core::money::Money::new(hi, self.currency),
            ),
            num_paths: stats.count(),
            num_simulated_paths: stats.count(),
            std_dev: Some(stats.std_dev()),
            median: None,
            percentile_25: None,
            percentile_75: None,
            min: None,
            max: None,
            num_skipped: 0,
        })
    }

    fn validate_inputs(&self) -> Result<()> {
        if self.event_times.is_empty() {
            return Err(finstack_core::Error::Validation(
                "RateExoticHw1fLsmcPricer requires at least one event time".into(),
            ));
        }
        if self.exercise_times.is_empty() {
            return Err(finstack_core::Error::Validation(
                "RateExoticHw1fLsmcPricer requires at least one exercise time".into(),
            ));
        }
        if self.exercise_times.len() != self.call_prices.len() {
            return Err(finstack_core::Error::Validation(format!(
                "exercise_times ({}) and call_prices ({}) length mismatch",
                self.exercise_times.len(),
                self.call_prices.len(),
            )));
        }
        for pair in self.event_times.windows(2) {
            if pair[1] <= pair[0] {
                return Err(finstack_core::Error::Validation(
                    "event_times must be strictly increasing".into(),
                ));
            }
        }
        for &t in &self.exercise_times {
            if !self.event_times.iter().any(|&e| (e - t).abs() < 1e-12) {
                return Err(finstack_core::Error::Validation(format!(
                    "exercise time {t} is not in event_times",
                )));
            }
        }
        Ok(())
    }
}

/// Return type of [`build_grid_with_exercise_map`]: `(grid, event_step_indices, exercise_event_indices)`.
type GridWithExerciseMap = (TimeGrid, Vec<usize>, Vec<usize>);

/// Build grid + map each exercise time to its position within
/// `event_step_indices` (i.e., the index of the event that coincides with
/// the exercise date).
fn build_grid_with_exercise_map(
    event_times: &[f64],
    exercise_times: &[f64],
    maturity: f64,
    min_steps: usize,
) -> Result<GridWithExerciseMap> {
    let (grid, event_step_indices) =
        super::hw1f_mc::build_event_aligned_grid(event_times, maturity, min_steps)?;
    let mut exercise_event_indices = Vec::with_capacity(exercise_times.len());
    for &t in exercise_times {
        let pos = event_times
            .iter()
            .position(|&e| (e - t).abs() < 1e-12)
            .ok_or_else(|| {
                finstack_core::Error::Validation(format!("exercise time {t} not in event_times"))
            })?;
        exercise_event_indices.push(pos);
    }
    Ok((grid, event_step_indices, exercise_event_indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::exotics_shared::standard_basis;
    use finstack_core::money::Money;
    use finstack_monte_carlo::traits::Payoff;

    /// Inert payoff that reports PV = notional (no coupons, no exercise benefit).
    #[derive(Debug, Clone)]
    struct ParPayoff {
        notional: f64,
    }
    impl Payoff for ParPayoff {
        fn on_event(&mut self, _s: &mut PathState) {}
        fn value(&self, ccy: Currency) -> Money {
            Money::new(self.notional, ccy)
        }
        fn reset(&mut self) {}
    }
    impl ExerciseBoundaryPayoff for ParPayoff {
        fn intrinsic_at(&self, _i: usize, _r: f64, ccy: Currency) -> Money {
            Money::new(self.notional, ccy)
        }
        fn continuation_basis(&self, _i: usize, t: f64, r: f64) -> Vec<f64> {
            standard_basis(t, r)
        }
    }

    #[test]
    fn noexercise_equals_par() {
        let pricer = RateExoticHw1fLsmcPricer {
            process_params: HullWhite1FParams::new(0.05, 0.001, 0.0),
            r0: 0.03,
            event_times: vec![1.0, 2.0],
            exercise_times: vec![1.0, 2.0],
            call_prices: vec![1.0, 1.0],
            notional: 1_000_000.0,
            config: RateExoticMcConfig {
                num_paths: 200,
                ..Default::default()
            },
            currency: Currency::USD,
        };
        let est = pricer
            .price(|| ParPayoff {
                notional: 1_000_000.0,
            })
            .expect("ok");
        // With call_price=1.0, call_value == notional, and deterministic PV is
        // also notional. Issuer is indifferent; cashflow[p] stays at notional.
        assert!((est.mean.amount() - 1_000_000.0).abs() < 1e-6);
    }

    /// Split-sample (out-of-sample) LSMC should also reach par on the degenerate
    /// no-benefit-to-exercise setup, and report stats over the pricing half only.
    #[test]
    fn oos_lsmc_noexercise_equals_par_and_uses_half_paths() {
        let pricer = RateExoticHw1fLsmcPricer {
            process_params: HullWhite1FParams::new(0.05, 0.001, 0.0),
            r0: 0.03,
            event_times: vec![1.0, 2.0],
            exercise_times: vec![1.0, 2.0],
            call_prices: vec![1.0, 1.0],
            notional: 1_000_000.0,
            config: RateExoticMcConfig {
                num_paths: 400,
                antithetic: false, // partition by raw stream parity is easiest to verify without anti
                oos_lsmc: true,
                ..Default::default()
            },
            currency: Currency::USD,
        };
        let est = pricer
            .price(|| ParPayoff {
                notional: 1_000_000.0,
            })
            .expect("ok");
        // Same indifference argument as in-sample: cashflow[p] stays at notional.
        assert!((est.mean.amount() - 1_000_000.0).abs() < 1e-6);
        // The pricing half is half the path count: 200 of 400.
        assert_eq!(est.num_paths, 200);
    }
}
