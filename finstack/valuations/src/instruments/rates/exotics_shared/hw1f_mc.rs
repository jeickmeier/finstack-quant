//! Generic HW1F Monte Carlo orchestrator for rate exotic products.
//!
//! The pricer is generic over a user-supplied [`finstack_monte_carlo::traits::Payoff`] that:
//! 1. Exposes event-times (year fractions from valuation date) via construction.
//! 2. Consumes [`finstack_monte_carlo::traits::PathState`] updates at each simulation step, reading the
//!    short rate and recording on-path discounted cashflows.
//! 3. Returns the accumulated PV via [`finstack_monte_carlo::traits::Payoff::value`] in the requested currency.
//!
//! The pricer handles: time-grid construction aligned to event dates,
//! HW1F process + exact discretization, RNG streams with antithetic
//! variates, and cross-path averaging with 95% CIs.
//!
//! At every step the pricer also accumulates the pathwise money-market
//! numeraire `B(t) = exp(∫₀ᵗ r ds)` (trapezoidal rule, see
//! [`crate::instruments::rates::exotics_shared::bank_account`]) and exposes it
//! to payoffs through [`StateKey::BankAccount`]. Payoffs must discount
//! simulated cashflows with this pathwise factor — not the deterministic
//! time-0 curve DF, which would drop the payoff/numeraire correlation.

use crate::instruments::rates::exotics_shared::bank_account::bank_step_factor;
use crate::instruments::rates::exotics_shared::mc_config::RateExoticMcConfig;
use finstack_core::currency::Currency;
use finstack_core::Result;
use finstack_monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use finstack_monte_carlo::online_stats::OnlineStats;
use finstack_monte_carlo::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use finstack_monte_carlo::results::MoneyEstimate;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::time_grid::TimeGrid;
use finstack_monte_carlo::traits::{Discretization, PathState, Payoff, RandomStream, StateKey};

/// HW1F Monte Carlo pricer for path-dependent rate exotics without exercise.
///
/// The pricer drives a user-supplied [`finstack_monte_carlo::traits::Payoff`] along simulated short-rate paths
/// produced by an exact HW1F discretization. The payoff is responsible for all
/// product-specific cashflow accumulation (including discounting); the pricer
/// only aggregates the per-path PVs into a [`MoneyEstimate`].
pub struct RateExoticHw1fMcPricer {
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
    /// Event times (year fractions), strictly increasing and strictly positive.
    pub event_times: Vec<f64>,
    /// Runtime Monte Carlo configuration (paths, seed, antithetic, step density).
    pub config: RateExoticMcConfig,
    /// Currency for the returned PV estimate.
    pub currency: Currency,
}

impl RateExoticHw1fMcPricer {
    /// Run the simulation, invoking `payoff_factory` once per path to obtain a
    /// fresh payoff accumulator.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `event_times` is empty or is not strictly
    /// increasing and positive, or if the internal time grid cannot be built.
    pub fn price<F, P>(&self, payoff_factory: F) -> Result<MoneyEstimate>
    where
        F: Fn() -> P + Sync,
        P: Payoff + 'static,
    {
        let Some(&maturity) = self.event_times.last() else {
            return Err(finstack_core::Error::Validation(
                "RateExoticHw1fMcPricer requires at least one event time".into(),
            ));
        };
        for pair in self.event_times.windows(2) {
            if pair[1] <= pair[0] {
                return Err(finstack_core::Error::Validation(
                    "RateExoticHw1fMcPricer event_times must be strictly increasing".into(),
                ));
            }
        }

        let (grid, event_step_indices) = build_event_aligned_grid(
            &self.event_times,
            maturity,
            self.config.min_steps_between_events,
        )?;

        let process = HullWhite1FProcess::new(self.process_params.clone());
        let disc = ExactHullWhite1F;
        let num_steps = grid.num_steps();
        let work_size = disc.work_size(&process);
        let raw_paths = self.config.raw_stream_count();
        let base_rng = PhiloxRng::new(self.config.seed);

        let mut stats = OnlineStats::new();

        // Per-path scratch buffers hoisted out of the path loop; the
        // discretization step fully overwrites `work` and `z` each step,
        // so reusing them across paths is bit-identical to fresh allocations.
        let mut work = vec![0.0; work_size];
        let mut z = [0.0_f64; 1];

        for path_id in 0..raw_paths {
            let multiplicity = if self.config.antithetic { 2 } else { 1 };
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
                        next_event += 1;
                    }
                }

                let pv = payoff.value(self.currency).amount();
                stats.update(pv);
            }
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
}

/// Build a time grid with steps aligned to event dates, returning the step
/// indices where each event lands.
///
/// The grid inserts `min_steps_between_events` sub-steps between consecutive
/// events (or more, proportional to the gap), so each event time lies on a
/// node of the returned [`TimeGrid`].
pub(super) fn build_event_aligned_grid(
    event_times: &[f64],
    maturity: f64,
    min_steps_between: usize,
) -> Result<(TimeGrid, Vec<usize>)> {
    let min_steps = min_steps_between.max(1);
    let mut times = vec![0.0_f64];
    let mut prev = 0.0_f64;
    let mut event_indices = Vec::with_capacity(event_times.len());

    for &event_t in event_times {
        if event_t <= prev {
            return Err(finstack_core::Error::Validation(format!(
                "event_times must be strictly increasing and positive, got {event_t} after {prev}"
            )));
        }
        let gap = event_t - prev;
        let n_sub = min_steps.max((gap * 12.0).ceil() as usize);
        let dt = gap / n_sub as f64;
        for k in 1..=n_sub {
            times.push(prev + k as f64 * dt);
        }
        event_indices.push(times.len() - 1);
        prev = event_t;
    }

    if maturity > prev + 1e-12 {
        let gap = maturity - prev;
        let n_sub = min_steps.max((gap * 12.0).ceil() as usize);
        let dt = gap / n_sub as f64;
        for k in 1..=n_sub {
            times.push(prev + k as f64 * dt);
        }
    }

    let grid = TimeGrid::from_times(times)
        .map_err(|e| finstack_core::Error::Validation(format!("time grid build failed: {e}")))?;
    Ok((grid, event_indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::money::Money;

    /// Trivial "pays 1.0 at the first event" payoff used for end-to-end sanity checks.
    #[derive(Debug, Clone, Default)]
    struct ZcbPayoff {
        paid: f64,
    }
    impl Payoff for ZcbPayoff {
        fn on_event(&mut self, _s: &mut PathState) {
            self.paid = 1.0;
        }
        fn value(&self, ccy: Currency) -> Money {
            Money::new(self.paid, ccy)
        }
        fn reset(&mut self) {
            self.paid = 0.0;
        }
    }

    /// Pays 1.0 at the (single) event, discounted with the pathwise
    /// bank-account numeraire exposed by the harness.
    #[derive(Debug, Clone, Default)]
    struct PathwiseZcbPayoff {
        pv: f64,
    }
    impl Payoff for PathwiseZcbPayoff {
        fn on_event(&mut self, s: &mut PathState) {
            let bank = s.get_key(StateKey::BankAccount).unwrap_or(1.0);
            self.pv = 1.0 / bank;
        }
        fn value(&self, ccy: Currency) -> Money {
            Money::new(self.pv, ccy)
        }
        fn reset(&mut self) {
            self.pv = 0.0;
        }
    }

    /// The harness's incremental bank-account accumulation must reproduce the
    /// money-market numeraire: with θ = r0 and σ → 0 the short rate stays at
    /// r0, so a unit cashflow at T = 1 discounts to exactly e^{−r0} (the
    /// trapezoidal rule is exact for a constant rate).
    #[test]
    fn pathwise_bank_account_discounts_zcb() {
        let r0 = 0.03;
        let pricer = RateExoticHw1fMcPricer {
            process_params: HullWhite1FParams::new(0.05, 1e-12, r0),
            r0,
            event_times: vec![1.0],
            config: RateExoticMcConfig {
                num_paths: 16,
                ..Default::default()
            },
            currency: Currency::USD,
        };
        let est = pricer.price(PathwiseZcbPayoff::default).expect("ok");
        assert!(
            (est.mean.amount() - (-r0).exp()).abs() < 1e-10,
            "pathwise ZCB {} should equal e^(-r0) = {}",
            est.mean.amount(),
            (-r0).exp()
        );
    }

    #[test]
    fn trivial_payoff_equals_one() {
        let pricer = RateExoticHw1fMcPricer {
            process_params: HullWhite1FParams::new(0.05, 0.01, 0.0),
            r0: 0.03,
            event_times: vec![1.0],
            config: RateExoticMcConfig {
                num_paths: 200,
                ..Default::default()
            },
            currency: Currency::USD,
        };
        let est = pricer.price(ZcbPayoff::default).expect("ok");
        assert!((est.mean.amount() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn event_grid_alignment() {
        let (grid, idx) = build_event_aligned_grid(&[1.0, 2.0, 3.0], 3.0, 4).expect("ok");
        assert_eq!(idx.len(), 3);
        for (i, &step) in idx.iter().enumerate() {
            let expected = [1.0, 2.0, 3.0][i];
            assert!((grid.time(step) - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn non_monotone_events_error() {
        assert!(build_event_aligned_grid(&[1.0, 0.5], 1.0, 4).is_err());
    }
}
