//! Generic HW1F Monte Carlo orchestrator for rate exotic products.
//!
//! The pricer is generic over a user-supplied [`finstack_quant_models::monte_carlo::traits::Payoff`] that:
//! 1. Exposes event-times (year fractions from valuation date) via construction.
//! 2. Consumes [`finstack_quant_models::monte_carlo::traits::PathState`] updates at each simulation step, reading the
//!    short rate and recording on-path discounted cashflows.
//! 3. Returns the accumulated PV via [`finstack_quant_models::monte_carlo::traits::Payoff::value`] in the requested currency.
//!
//! The pricer handles: time-grid construction aligned to event dates,
//! HW1F process + exact discretization, RNG streams with antithetic
//! variates, and cross-path averaging with 95% CIs.
//!
//! At every step the pricer also accumulates the pathwise money-market
//! numeraire `B(t) = exp(∫₀ᵗ r ds)` (trapezoidal rule, see
//! [`crate::instruments::rates::hw1f::bank_account`]) and exposes it
//! to payoffs through `StateKey::BankAccount`. Payoffs must discount
//! simulated cashflows with this pathwise factor — not the deterministic
//! time-0 curve DF, which would drop the payoff/numeraire correlation.

use crate::instruments::rates::hw1f::bank_account::bank_step_factor;
use crate::instruments::rates::hw1f::mc_config::{RateExoticMcConfig, SampleSplit};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;
use finstack_quant_models::monte_carlo::discretization::exact_hw1f::ExactHullWhite1F;
use finstack_quant_models::monte_carlo::process::ou::{HullWhite1FParams, HullWhite1FProcess};
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::{
    Discretization, PathState, Payoff, RandomStream, StateKey,
};
use finstack_quant_models::monte_carlo::OnlineStats;
use finstack_quant_models::monte_carlo::TimeGrid;

/// HW1F Monte Carlo pricer for path-dependent rate exotics without exercise.
///
/// The pricer drives a user-supplied [`finstack_quant_models::monte_carlo::traits::Payoff`] along simulated short-rate paths
/// produced by an exact HW1F discretization. The payoff is responsible for all
/// product-specific cashflow accumulation (including discounting); the pricer
/// only aggregates the per-path PVs into a [`MoneyEstimate`].
pub struct RateExoticHw1fMcPricer {
    /// Fully-specified HW1F short-rate parameters: κ, σ, and the
    /// time-dependent mean-reversion level θ(t).
    ///
    /// The simulated short rate follows `dr_t = κ·(θ(t) - r_t)·dt + σ·dW_t`.
    /// θ(t) MUST be bootstrapped from the product's discount curve (see
    /// [`crate::instruments::rates::hw1f::prepare_hw1f_params`])
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
            return Err(finstack_quant_core::Error::Validation(
                "RateExoticHw1fMcPricer requires at least one event time".into(),
            ));
        };
        for pair in self.event_times.windows(2) {
            if pair[1] <= pair[0] {
                return Err(finstack_quant_core::Error::Validation(
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
        let disc = ExactHullWhite1F::new();
        let num_steps = grid.num_steps();
        let work_size = disc.work_size(&process);
        let raw_paths = self.config.raw_stream_count();
        let base_rng = PhiloxRng::new(self.config.seed);

        let mut path_values = Vec::with_capacity(self.config.effective_path_count());

        // Per-path scratch buffers hoisted out of the path loop; the
        // discretization step fully overwrites `work` and `z` each step,
        // so reusing them across paths is bit-identical to fresh allocations.
        let mut work = vec![0.0; work_size];
        let mut z = [0.0_f64; 1];

        let multiplicity = self.config.split().multiplicity;
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
                        payoff.on_event(&mut state)?;
                        next_event += 1;
                    }
                }

                path_values.push(payoff.value(self.currency)?.amount());
            }
        }

        money_estimate_from_pairs(&path_values, self.config.split(), 1.0, self.currency)
    }
}

/// Aggregate per-path present values into a [`MoneyEstimate`].
///
/// Antithetic legs share a stream and are negatively correlated, so they are
/// not i.i.d. samples: each adjacent `(original, antithetic)` pair (a chunk of
/// `split.multiplicity` consecutive entries of `path_values`) is averaged into
/// one sample so the reported standard error reflects the pair variance
/// rather than understating it. In split-sample mode only the pricing half
/// contributes to the estimate. `scale` multiplies every sample (e.g. the
/// terminal-measure `P(0, T_N)` of the LMM engine); pass `1.0` when the values
/// are already time-0 present values.
pub(crate) fn money_estimate_from_pairs(
    path_values: &[f64],
    split: SampleSplit,
    scale: f64,
    currency: Currency,
) -> finstack_quant_core::Result<MoneyEstimate> {
    let multiplicity = split.multiplicity;
    let mut stats = OnlineStats::new();
    for (pair_idx, chunk) in path_values.chunks(multiplicity).enumerate() {
        if !split.is_price(pair_idx * multiplicity) {
            continue;
        }
        let pair_avg = chunk.iter().sum::<f64>() / chunk.len() as f64;
        stats.update(pair_avg * scale);
    }

    let n = stats.count().max(1) as f64;
    let aggregated_paths = stats.count() * multiplicity;
    let mean = stats.mean();
    let stderr = stats.std_dev() / n.sqrt();
    let lo = mean - 1.96 * stderr;
    let hi = mean + 1.96 * stderr;
    Ok(MoneyEstimate {
        mean: finstack_quant_core::money::Money::new(mean, currency)?,
        stderr,
        ci_95: (
            finstack_quant_core::money::Money::new(lo, currency)?,
            finstack_quant_core::money::Money::new(hi, currency)?,
        ),
        num_paths: aggregated_paths,
        num_simulated_paths: aggregated_paths,
        std_dev: Some(stats.std_dev()),
        median: None,
        percentile_25: None,
        percentile_75: None,
        min: None,
        max: None,
    })
}

/// Build a time grid with steps aligned to event dates, returning the step
/// indices where each event lands.
///
/// The grid inserts at least `min_steps_between` sub-steps between
/// consecutive events (more for long gaps: roughly monthly), so each event
/// time lands exactly on a node of the returned [`TimeGrid`]. The trailing
/// segment up to `maturity` is subdivided the same way. Shared by the HW1F
/// exotic, HW1F Bermudan LSMC and LMM Bermudan engines.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if `event_times` are not
/// strictly increasing and positive.
pub(crate) fn build_event_aligned_grid(
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
            return Err(finstack_quant_core::Error::Validation(format!(
                "event_times must be strictly increasing and positive, got {event_t} after {prev}"
            )));
        }
        push_subdivided_segment(&mut times, prev, event_t, min_steps);
        event_indices.push(times.len() - 1);
        prev = event_t;
    }

    if maturity > prev + 1e-12 {
        push_subdivided_segment(&mut times, prev, maturity, min_steps);
    }

    let grid = TimeGrid::from_times(times).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("time grid build failed: {e}"))
    })?;
    Ok((grid, event_indices))
}

/// Append the interior nodes and the exact end point of `(from, to]`.
fn push_subdivided_segment(times: &mut Vec<f64>, from: f64, to: f64, min_steps: usize) {
    let gap = to - from;
    let n_sub = min_steps.max((gap * 12.0).ceil() as usize);
    let dt = gap / n_sub as f64;
    for k in 1..n_sub {
        times.push(from + k as f64 * dt);
    }
    times.push(to);
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::money::Money;

    /// Trivial "pays 1.0 at the first event" payoff used for end-to-end sanity checks.
    #[derive(Debug, Clone, Default)]
    struct ZcbPayoff {
        paid: f64,
    }
    impl Payoff for ZcbPayoff {
        fn on_event(&mut self, _s: &mut PathState) -> finstack_quant_core::Result<()> {
            self.paid = 1.0;
            Ok(())
        }
        fn value(&self, ccy: Currency) -> finstack_quant_core::Result<Money> {
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
        fn on_event(&mut self, s: &mut PathState) -> finstack_quant_core::Result<()> {
            let bank = s.get_key(StateKey::BankAccount).unwrap_or(1.0);
            self.pv = 1.0 / bank;
            Ok(())
        }
        fn value(&self, ccy: Currency) -> finstack_quant_core::Result<Money> {
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
            process_params: HullWhite1FParams::new(0.05, 1e-12, r0)
                .expect("valid Hull-White parameters"),
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
            process_params: HullWhite1FParams::new(0.05, 0.01, 0.0)
                .expect("valid Hull-White parameters"),
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

    /// Antithetic pairs must be aggregated as one sample each. For a payoff
    /// monotone in the shocks (a pathwise-discounted ZCB), antithetic
    /// variates reduce variance, so the pair-averaged stderr must come in at
    /// or below the plain i.i.d. stderr at the same effective path count.
    #[test]
    fn antithetic_pair_stderr_below_iid() {
        let r0 = 0.03;
        let make = |antithetic: bool| RateExoticHw1fMcPricer {
            process_params: HullWhite1FParams::new(0.05, 0.01, r0)
                .expect("valid Hull-White parameters"),
            r0,
            event_times: vec![1.0],
            config: RateExoticMcConfig {
                num_paths: 4_000,
                antithetic,
                ..Default::default()
            },
            currency: Currency::USD,
        };
        let anti = make(true).price(PathwiseZcbPayoff::default).expect("ok");
        let iid = make(false).price(PathwiseZcbPayoff::default).expect("ok");
        assert_eq!(anti.num_paths, 4_000);
        assert!(
            anti.stderr <= iid.stderr,
            "antithetic pair stderr {} should not exceed i.i.d. stderr {}",
            anti.stderr,
            iid.stderr
        );
    }
}
