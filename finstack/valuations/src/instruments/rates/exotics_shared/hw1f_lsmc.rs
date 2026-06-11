//! Generic HW1F Longstaff-Schwartz MC pricer for callable rate exotics.
//!
//! The pricer drives a user-supplied [`ExerciseBoundaryPayoff`] along
//! simulated short-rate paths produced by an exact HW1F discretization.
//! A two-phase algorithm is used:
//!
//! 1. **Forward pass.** For each path the harness runs the full simulation
//!    (accumulating the pathwise money-market numeraire `B(t)`, exposed to
//!    payoffs via [`StateKey::BankAccount`]) and records per-path the time-0
//!    PV reported by [`finstack_monte_carlo::traits::Payoff::value`], as well
//!    as the short rate, undiscounted exercise value, bank factor, and
//!    inactive flag at each exercise date.
//! 2. **Backward pass.** Starting from maturity, the harness decomposes each
//!    path's value into a pre-exercise component (cashflows on or before the
//!    exercise date, via [`ExerciseBoundaryPayoff::value_after`]) and a
//!    post-exercise component. It regresses the at-exercise continuation of
//!    the *post-exercise* component (`(cashflow[p] − pre[p]) · B_p(t_ex)`) via
//!    [`finstack_monte_carlo::pricer::lsq::solve_least_squares`] against the
//!    [`standard_basis`] (ITM + active paths only) and rolls the per-path
//!    cashflow vector back, overwriting only the post-exercise portion with
//!    the pathwise-discounted call value (`cashflow[p] = pre[p] +
//!    exercise_value / B_p(t_ex)`) whenever exercise is optimal — coupons
//!    paid before the call date are neither regressed nor dropped.
//!    Discounting with ratios of the pathwise `B(t)` (rather than the
//!    deterministic curve DF) keeps the payoff/numeraire correlation that the
//!    short-rate model exists to capture.
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

use crate::instruments::rates::exotics_shared::bank_account::bank_step_factor;
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
    ///
    /// The at-exercise call amounts themselves live on the payoff
    /// ([`ExerciseBoundaryPayoff::intrinsic_at`]); the harness never scales
    /// or prices the call independently.
    pub exercise_times: Vec<f64>,
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
    /// or if the time-grid construction fails. Propagates errors from
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
        let mut exercise_banks = vec![1.0_f64; n_paths * n_ex];
        let mut exercise_inactive = vec![false; n_paths * n_ex];
        // Time-0 pathwise PV of cashflows paid on or *before* each exercise
        // date (`value() - value_after(ex_idx)`). These are kept regardless
        // of the exercise decision: they are excluded from the continuation
        // regression target and re-added after an exercise overwrite.
        let mut pre_exercise_pv = vec![0.0_f64; n_paths * n_ex];

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
                let mut bank = 1.0_f64;
                let mut payoff = payoff_factory();
                payoff.reset();
                let mut state = PathState::new(0, 0.0);
                state.set_key(StateKey::ShortRate, r);
                state.set_key(StateKey::BankAccount, bank);

                let mut next_event = 0usize;
                let mut next_exercise = 0usize;
                for step in 0..num_steps {
                    let t = grid.time(step);
                    let dt = grid.dt(step);
                    rng.fill_std_normals(&mut z);
                    if anti == 1 {
                        z[0] = -z[0];
                    }
                    let r_prev = r;
                    disc.step(
                        &process,
                        t,
                        dt,
                        core::slice::from_mut(&mut r),
                        &z,
                        &mut work,
                    );
                    bank *= bank_step_factor(r_prev, r, dt);

                    let t_next = grid.time(step + 1);
                    state.set_step_time(step + 1, t_next);
                    state.set_key(StateKey::ShortRate, r);
                    state.set_key(StateKey::BankAccount, bank);

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
                            exercise_banks[flat] = bank;
                            exercise_inactive[flat] = payoff.is_path_inactive();
                            next_exercise += 1;
                        }
                        next_event += 1;
                    }
                }

                let full_value = payoff.value(self.currency).amount();
                deterministic_pv[path_cursor] = full_value;
                for ex in 0..n_ex {
                    pre_exercise_pv[path_cursor * n_ex + ex] =
                        full_value - payoff.value_after(ex, self.currency).amount();
                }
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
                // Regression target is the *at-exercise* continuation value of
                // the cashflows the issuer can still extinguish by calling:
                // the post-exercise portion of the time-0 cashflow, compounded
                // forward by the pathwise numeraire B(t_ex). Coupons paid on
                // or before t_ex are sunk and excluded from the target.
                active_continuation.push((cf - pre_exercise_pv[flat]) * exercise_banks[flat]);
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
                    // `cont_hat` estimates the at-exercise continuation value
                    // of the post-exercise cashflows; the (undiscounted)
                    // exercise value is compared on the same basis. On
                    // exercise, only the post-exercise portion is replaced by
                    // the call amount (rolled to time 0 with the pathwise
                    // numeraire); coupons already paid before t_ex are kept.
                    let cont_hat: f64 = basis.iter().zip(coeffs.iter()).map(|(b, c)| b * c).sum();
                    if exercise_value < cont_hat {
                        *cf = pre_exercise_pv[flat] + exercise_value / exercise_banks[flat];
                    }
                }
            } else {
                // Fallback: pathwise issuer exercise against the realized
                // post-exercise cashflow (both sides compared at the exercise
                // date). Pre-exercise coupons are sunk and kept either way.
                for (p, cf) in cashflow.iter_mut().enumerate() {
                    let flat = p * n_ex + ex_idx;
                    if exercise_inactive[flat] {
                        continue;
                    }
                    let exercise_value = exercise_values[flat];
                    let post_at_ex = (*cf - pre_exercise_pv[flat]) * exercise_banks[flat];
                    if exercise_value > 0.0 && exercise_value < post_at_ex {
                        *cf = pre_exercise_pv[flat] + exercise_value / exercise_banks[flat];
                    }
                }
            }
        }

        // -- Phase 3: aggregate ------------------------------------------------
        // In split-sample mode, only the pricing half (out-of-sample under the
        // train-fitted policy) contributes to the reported estimate.
        //
        // Antithetic legs share a stream and are negatively correlated, so
        // they are not i.i.d. samples: each adjacent (original, antithetic)
        // pair is averaged into one sample so the reported stderr reflects
        // the pair variance rather than understating it. Pairs occupy
        // consecutive `path_cursor` slots and never straddle the train/price
        // split (the split is by raw-stream parity).
        let mut stats = OnlineStats::new();
        for (pair_idx, chunk) in cashflow.chunks(multiplicity).enumerate() {
            if !is_price(pair_idx * multiplicity) {
                continue;
            }
            let pair_avg = chunk.iter().sum::<f64>() / chunk.len() as f64;
            stats.update(pair_avg);
        }

        let n = stats.count().max(1) as f64;
        let aggregated_paths = stats.count() * multiplicity;
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
            num_paths: aggregated_paths,
            num_simulated_paths: aggregated_paths,
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

    use finstack_monte_carlo::traits::StateKey;

    /// Zero-coupon-bond payoff: pays `notional` at maturity (the last event),
    /// discounted pathwise via the bank-account numeraire exposed by the
    /// harness. Callable at par at every exercise date via `intrinsic_at`,
    /// which (per the trait contract) returns the *undiscounted* at-exercise
    /// value.
    #[derive(Debug, Clone)]
    struct ParPayoff {
        notional: f64,
        bank_at_last_event: f64,
    }
    impl Payoff for ParPayoff {
        fn on_event(&mut self, s: &mut PathState) {
            self.bank_at_last_event = s.get_key(StateKey::BankAccount).unwrap_or(1.0);
        }
        fn value(&self, ccy: Currency) -> Money {
            Money::new(self.notional / self.bank_at_last_event, ccy)
        }
        fn reset(&mut self) {
            self.bank_at_last_event = 1.0;
        }
    }
    impl ExerciseBoundaryPayoff for ParPayoff {
        fn intrinsic_at(&self, _i: usize, _r: f64, ccy: Currency) -> Money {
            Money::new(self.notional, ccy)
        }
        fn continuation_basis(&self, _i: usize, t: f64, r: f64) -> Vec<f64> {
            standard_basis(t, r)
        }
    }

    fn par_payoff() -> ParPayoff {
        ParPayoff {
            notional: 1_000_000.0,
            bank_at_last_event: 1.0,
        }
    }

    /// With θ = r0 the short rate stays near r0 and the at-exercise
    /// continuation (par discounted from maturity) is always *below* par, so
    /// the issuer never calls: the LSMC PV is the pathwise-discounted ZCB
    /// price `notional · E[1/B(2)] ≈ notional · e^{−r0·2}`.
    #[test]
    fn noexercise_equals_discounted_par() {
        let pricer = RateExoticHw1fLsmcPricer {
            process_params: HullWhite1FParams::new(0.05, 0.001, 0.03),
            r0: 0.03,
            event_times: vec![1.0, 2.0],
            exercise_times: vec![1.0, 2.0],
            config: RateExoticMcConfig {
                num_paths: 2_000,
                ..Default::default()
            },
            currency: Currency::USD,
        };
        let est = pricer.price(par_payoff).expect("ok");
        let expected = 1_000_000.0 * (-0.03_f64 * 2.0).exp();
        assert!(
            (est.mean.amount() - expected).abs() < 0.002 * expected,
            "mean {} should be near discounted par {expected}",
            est.mean.amount()
        );
    }

    /// Coupon-bearing payoff: pays `coupon` at the first event (t=1.0) and
    /// `notional` at the last event (t=2.0); callable at par at t=1.5.
    /// `value_after` excludes the coupon (paid before the exercise date).
    #[derive(Debug, Clone)]
    struct CouponThenBulletPayoff {
        coupon: f64,
        notional: f64,
        event_idx: usize,
        bank_at_coupon: f64,
        bank_at_maturity: f64,
    }
    impl Payoff for CouponThenBulletPayoff {
        fn on_event(&mut self, s: &mut PathState) {
            let bank = s.get_key(StateKey::BankAccount).unwrap_or(1.0);
            match self.event_idx {
                0 => self.bank_at_coupon = bank,
                2 => self.bank_at_maturity = bank,
                _ => {}
            }
            self.event_idx += 1;
        }
        fn value(&self, ccy: Currency) -> Money {
            Money::new(
                self.coupon / self.bank_at_coupon + self.notional / self.bank_at_maturity,
                ccy,
            )
        }
        fn reset(&mut self) {
            self.event_idx = 0;
            self.bank_at_coupon = 1.0;
            self.bank_at_maturity = 1.0;
        }
    }
    impl ExerciseBoundaryPayoff for CouponThenBulletPayoff {
        fn intrinsic_at(&self, _i: usize, _r: f64, ccy: Currency) -> Money {
            Money::new(self.notional, ccy)
        }
        fn continuation_basis(&self, _i: usize, t: f64, r: f64) -> Vec<f64> {
            standard_basis(t, r)
        }
        fn value_after(&self, _i: usize, ccy: Currency) -> Money {
            Money::new(self.notional / self.bank_at_maturity, ccy)
        }
    }

    fn coupon_payoff() -> CouponThenBulletPayoff {
        CouponThenBulletPayoff {
            coupon: 50_000.0,
            notional: 1_000_000.0,
            event_idx: 0,
            bank_at_coupon: 1.0,
            bank_at_maturity: 1.0,
        }
    }

    fn coupon_pricer(r0: f64) -> RateExoticHw1fLsmcPricer {
        RateExoticHw1fLsmcPricer {
            process_params: HullWhite1FParams::new(0.05, 0.001, r0),
            r0,
            event_times: vec![1.0, 1.5, 2.0],
            exercise_times: vec![1.5],
            config: RateExoticMcConfig {
                num_paths: 2_000,
                ..Default::default()
            },
            currency: Currency::USD,
        }
    }

    /// Negative rates make the at-exercise continuation (par discounted at a
    /// negative rate) exceed par, so the issuer always calls at t=1.5. The
    /// coupon paid at t=1.0 — before the call date — must survive the
    /// exercise overwrite: PV = c·e^{0.02·1} + N·e^{0.02·1.5} (always-exercise
    /// analytic bound). The pre-fix harness dropped the coupon on call.
    #[test]
    fn coupon_before_exercise_survives_call() {
        let est = coupon_pricer(-0.02).price(coupon_payoff).expect("ok");
        let expected = 50_000.0 * (0.02_f64).exp() + 1_000_000.0 * (0.03_f64).exp();
        assert!(
            (est.mean.amount() - expected).abs() < 0.002 * expected,
            "mean {} should be near always-exercise bound {expected}",
            est.mean.amount()
        );
    }

    /// Positive rates make the continuation (par discounted) below par, so
    /// the issuer never calls: PV = c·e^{−0.03·1} + N·e^{−0.03·2}
    /// (no-exercise analytic bound).
    #[test]
    fn coupon_payoff_noexercise_equals_analytic() {
        let est = coupon_pricer(0.03).price(coupon_payoff).expect("ok");
        let expected = 50_000.0 * (-0.03_f64).exp() + 1_000_000.0 * (-0.06_f64).exp();
        assert!(
            (est.mean.amount() - expected).abs() < 0.002 * expected,
            "mean {} should be near no-exercise bound {expected}",
            est.mean.amount()
        );
    }

    /// Split-sample (out-of-sample) LSMC should also reach the discounted-par
    /// value on the no-call setup, and report stats over the pricing half only.
    #[test]
    fn oos_lsmc_noexercise_equals_discounted_par_and_uses_half_paths() {
        let pricer = RateExoticHw1fLsmcPricer {
            process_params: HullWhite1FParams::new(0.05, 0.001, 0.03),
            r0: 0.03,
            event_times: vec![1.0, 2.0],
            exercise_times: vec![1.0, 2.0],
            config: RateExoticMcConfig {
                num_paths: 400,
                antithetic: false, // partition by raw stream parity is easiest to verify without anti
                oos_lsmc: true,
                ..Default::default()
            },
            currency: Currency::USD,
        };
        let est = pricer.price(par_payoff).expect("ok");
        let expected = 1_000_000.0 * (-0.03_f64 * 2.0).exp();
        assert!(
            (est.mean.amount() - expected).abs() < 0.005 * expected,
            "mean {} should be near discounted par {expected}",
            est.mean.amount()
        );
        // The pricing half is half the path count: 200 of 400.
        assert_eq!(est.num_paths, 200);
    }
}
