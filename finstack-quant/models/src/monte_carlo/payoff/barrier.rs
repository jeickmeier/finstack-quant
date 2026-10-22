//! Barrier option payoffs with explicit monitoring semantics.
//!
//! Continuous barriers use an exact log-Brownian-bridge crossing test between
//! simulation steps. Discrete barriers observe the simulated level only at
//! their contractual observation steps and never interpolate crossings between
//! observations.
//!
//! # Local volatility under stochastic-vol models
//!
//! The bridge correction needs an instantaneous volatility to turn a
//! (spot, next_spot) pair into a barrier hit probability. When the
//! [`PathState`] carries a stochastic variance
//! (e.g. Heston's `VARIANCE` key), [`BarrierOptionPayoff::on_event`]
//! substitutes `sqrt(variance)` (including zero) for the configured flat [`BarrierOptionPayoff::sigma`].
//! Freezing the local variance over the step approximates stochastic-volatility crossings;
//! it requires time-step convergence. This makes the reported payoff consistent with the path-level dynamics
//! under stochastic-vol models; for deterministic-vol processes the state
//! carries no variance entry and the configured sigma is used unchanged.
//!
//! # Brownian-bridge modelling assumptions
//!
//! The bridge formula behind the continuous crossing test is derived under
//! the assumption that, conditional on `(S_t, S_{t+Δt})`, `ln S_u` is a
//! linear Brownian bridge on `[t, t+Δt]` with **constant** instantaneous volatility `σ`. This is exact for GBM
//! with deterministic (piecewise-constant) volatility on the interval and is
//! widely used as a first-order correction for:
//!
//! - Local-volatility models, provided `σ` is set to an effective local
//!   volatility over the step (bias is `O(σ'·Δt)`).
//! - Stochastic-volatility models (Heston, SABR, …) only over sub-steps
//!   short enough that the variance process is approximately frozen.
//!
//! It is **not** appropriate for jump-diffusion models, for rough
//! volatility (fBM-driven) paths, or for multi-factor processes where the
//! underlying has non-trivial correlation with other state variables. In
//! those cases prefer finer monitoring or a model-specific exit
//! distribution.

use crate::monte_carlo::traits::PathState;
use crate::monte_carlo::traits::Payoff;
use crate::types::OptionType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::BarrierDirection;
use finstack_quant_core::types::BarrierType;

/// Continuity-correction coefficient `β = −ζ(1/2)/√(2π)`.
///
/// Under discrete monitoring with equal time steps, evaluating a *continuous*
/// barrier formula at the shifted barrier `B · exp(±β · σ · √Δt)` reproduces
/// the discretely-monitored price up to `o(1/√n)` error in the number of
/// monitoring points. The shift moves the barrier *away* from spot (down for
/// a down barrier, up for an up barrier), because discrete monitoring misses
/// crossings and so knocks out less often than continuous monitoring.
///
/// The adjusted barrier is `B' = B · exp(±β · σ · √Δt)`: `exp(−…)` for a
/// down barrier, `exp(+…)` for an up barrier.
///
/// # Numerical value
///
/// Using `ζ(1/2) = -1.4603545088095868…` and `√(2π) = 2.5066282746310002…`
/// gives `β ≈ 0.5825971579390106`, i.e. full f64 precision.
///
/// References:
/// - Broadie, Glasserman & Kou (1997). "A Continuity Correction for Discrete
///   Barrier Options." *Mathematical Finance*, 7(4), 325–349.
/// - Gobet & Miri (2001). "Weak approximation of averaged diffusion
///   processes" (extension to local-volatility models; same leading
///   coefficient β).
pub const GOBET_MIRI_BETA: f64 = 0.582_597_157_939_010_6;

/// Simulation-step representation of a barrier's monitoring contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarrierMonitoring {
    /// Continuously monitor from `start_step` through maturity.
    Continuous {
        /// First simulation step at which monitoring is active.
        start_step: usize,
    },
    /// Observe only at the listed simulation steps.
    Discrete {
        /// Strictly increasing simulation steps corresponding to contractual dates.
        observation_steps: Vec<usize>,
    },
}

impl BarrierMonitoring {
    /// Construct a discrete monitoring schedule with sorted, deduplicated steps.
    ///
    /// # Arguments
    ///
    /// * `observation_steps` - Zero-based simulation steps at which the barrier
    ///   or extremum is observed; sorted and deduplicated on construction.
    #[must_use]
    pub fn discrete(mut observation_steps: Vec<usize>) -> Self {
        observation_steps.sort_unstable();
        observation_steps.dedup();
        Self::Discrete { observation_steps }
    }

    /// Return `self` with any discrete `observation_steps` sorted and deduplicated.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if let Self::Discrete {
            ref mut observation_steps,
        } = self
        {
            observation_steps.sort_unstable();
            observation_steps.dedup();
        }
        self
    }

    /// Return whether the path is observed at simulation step `step`.
    ///
    /// # Arguments
    ///
    /// * `step` - Zero-based simulation event step. Continuous monitoring
    ///   observes every step from `start_step` on; discrete monitoring observes
    ///   only the listed steps.
    pub fn observes(&self, step: usize) -> bool {
        match self {
            Self::Continuous { start_step } => step >= *start_step,
            Self::Discrete { observation_steps } => {
                if observation_steps.is_sorted() {
                    observation_steps.binary_search(&step).is_ok()
                } else {
                    observation_steps.contains(&step)
                }
            }
        }
    }
}

/// Barrier option payoff with bridge correction.
///
/// A generic barrier option (Call or Put) with a barrier that can knock in or out,
/// and an optional rebate paid at maturity if the option is not active (e.g. knocked out).
#[derive(Debug, Clone)]
pub struct BarrierOptionPayoff {
    /// Strike price
    pub strike: f64,
    /// Barrier level
    pub barrier: f64,
    /// Barrier type
    pub barrier_type: BarrierType,
    /// Option type (Call/Put)
    pub option_type: OptionType,
    /// Rebate amount (paid at maturity if option deactivated/not-activated)
    pub rebate: Option<f64>,
    /// Notional
    pub notional: f64,
    /// Maturity step
    pub maturity_step: usize,
    /// Fallback volatility for continuous Brownian-bridge crossing tests. Must
    /// match the simulating process's diffusion volatility for deterministic-vol
    /// processes; ignored when the path state carries a variance key.
    pub sigma: f64,
    /// Time steps for each simulation interval.
    pub step_dts: Vec<f64>,
    /// Contractual monitoring convention mapped onto the simulation grid.
    pub monitoring: BarrierMonitoring,
    /// When `Some(r)`, knock-out rebates are paid **at the hit time** τ and
    /// compounded forward to maturity at the continuously compounded rate
    /// `r`, so the engine's maturity discount factor `DF(T)` nets to the
    /// correct `DF(τ)`. `None` keeps the at-expiry convention. See
    /// [`Self::with_rebate_at_hit`].
    rebate_at_hit_rate: Option<f64>,
    /// Year fraction from t = 0 to `maturity_step` (Σ step_dts), used to
    /// grow at-hit rebates forward to maturity.
    total_time: f64,

    terminal_spot: f64,
    barrier_hit: bool,
    /// Barrier state restored by [`Self::reset`] before each simulated path.
    initial_barrier_hit: bool,
    previous_spot: f64,
    /// Year fraction at which the barrier was first hit (valid only when
    /// `barrier_hit` is true). The bridge detects intra-interval crossings,
    /// so τ is recorded at the end of the crossing interval — an O(Δt)
    /// approximation consistent with the rest of the monitoring.
    hit_time: f64,
}

impl BarrierOptionPayoff {
    /// Create a new barrier option payoff.
    ///
    /// Rebate timing is at-expiry unless [`Self::with_rebate_at_hit`] is
    /// chained. Valuations product defaults are `AtHit`.
    ///
    /// # Arguments
    ///
    /// * `strike` - Option strike in the same units as the simulated spot.
    /// * `barrier` - Knock-in or knock-out level in the same units as spot.
    /// * `barrier_type` - Up/down and in/out classification of the barrier.
    /// * `option_type` - Call or put intrinsic evaluated at expiry when the
    ///   option is active.
    /// * `rebate` - Cash amount paid when the option is inactive (knocked
    ///   out, or a knock-in that never hits). Paid at expiry unless
    ///   [`Self::with_rebate_at_hit`] is chained.
    /// * `notional` - Linear payoff scaling applied to the intrinsic or rebate.
    /// * `maturity_step` - Path step index at which the payoff observes `S_T`.
    /// * `sigma` - Fallback Black volatility (annualized, decimal) used by
    ///   the Brownian-bridge hit test when the path state has no variance
    ///   key. For deterministic-vol processes (e.g. GBM) it **must equal the
    ///   simulating process's diffusion volatility**: the payoff has no
    ///   handle on the process, so a mismatch is not detectable here and
    ///   silently biases the bridge crossing probability. Under
    ///   stochastic-vol models (e.g. Heston) the path's `sqrt(variance)` is
    ///   used instead and this value is ignored.
    /// * `time_grid` - Simulation grid whose step year-fractions size the
    ///   continuous bridge intervals and the at-hit forward-compounding horizon.
    /// * `monitoring` - Continuous start step or discrete contractual
    ///   observation steps mapped onto `time_grid`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        strike: f64,
        barrier: f64,
        barrier_type: BarrierType,
        option_type: OptionType,
        rebate: Option<f64>,
        notional: f64,
        maturity_step: usize,
        sigma: f64,
        time_grid: &TimeGrid,
        monitoring: BarrierMonitoring,
    ) -> Self {
        let step_dts: Vec<f64> = time_grid.dts().to_vec();
        let total_time: f64 = step_dts.iter().take(maturity_step).sum();
        let monitoring = monitoring.normalized();
        Self {
            strike,
            barrier,
            barrier_type,
            option_type,
            rebate,
            notional,
            maturity_step,
            sigma,
            step_dts,
            monitoring,
            rebate_at_hit_rate: None,
            total_time,
            terminal_spot: 0.0,
            barrier_hit: false,
            initial_barrier_hit: false,
            previous_spot: 0.0,
            hit_time: 0.0,
        }
    }

    /// Seed the payoff with an externally observed historical barrier breach.
    #[must_use]
    pub fn with_observed_barrier_breached(mut self, breached: bool) -> Self {
        self.barrier_hit = breached;
        self.initial_barrier_hit = breached;
        if breached {
            self.hit_time = 0.0;
        }
        self
    }

    /// Pay knock-out rebates at the hit time instead of at expiry.
    ///
    /// `rate` is the continuously compounded flat rate used to compound the
    /// rebate forward from the hit time τ to maturity T, so the engine's
    /// single maturity discount factor nets to `DF(τ)`:
    /// `DF(T) · R·e^{r(T−τ)} = R·DF(τ)`. This is the dominant market
    /// convention for knock-out rebates. Knock-in rebates (paid when the
    /// barrier is never touched) are unaffected — there is no hit time and
    /// they pay at expiry by definition.
    ///
    /// # Arguments
    ///
    /// * `rate` - Continuously compounded flat rate (decimal, annualized) used
    ///   to compound a knock-out rebate from the hit time τ to maturity T so
    ///   the engine's maturity discount nets to `DF(τ)`. Knock-in rebates are
    ///   unaffected.
    #[must_use]
    pub fn with_rebate_at_hit(mut self, rate: f64) -> Self {
        self.rebate_at_hit_rate = Some(rate);
        self
    }

    /// Check if option is active based on barrier status.
    fn is_active(&self) -> bool {
        if self.barrier_type.is_knock_out() {
            !self.barrier_hit
        } else {
            self.barrier_hit
        }
    }
}

impl Payoff for BarrierOptionPayoff {
    fn supports_lrm_greeks(&self) -> bool {
        self.maturity_step > 0
            && matches!(self.monitoring, BarrierMonitoring::Discrete { .. })
            && !self.monitoring.observes(0)
    }

    fn needs_uniform_random(&self) -> bool {
        matches!(self.monitoring, BarrierMonitoring::Continuous { .. })
    }

    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        if state.step > self.maturity_step {
            return Ok(());
        }

        let current_spot = super::require_finite_state(state.spot(), "SPOT", state.step)?;
        let breached = |spot: f64| match self.barrier_type.direction() {
            BarrierDirection::Up => spot >= self.barrier,
            BarrierDirection::Down => spot <= self.barrier,
        };

        if state.step == 0 {
            self.previous_spot = current_spot;
            let observe_initial = match &self.monitoring {
                BarrierMonitoring::Continuous { start_step } => *start_step == 0,
                BarrierMonitoring::Discrete { observation_steps } => {
                    observation_steps.binary_search(&0).is_ok()
                }
            };
            if !self.barrier_hit && observe_initial && breached(current_spot) {
                self.barrier_hit = true;
                self.hit_time = 0.0;
            }
            if self.maturity_step == 0 {
                self.terminal_spot = current_spot;
            }
            return Ok(());
        }

        if !self.barrier_hit {
            let hit = match &self.monitoring {
                BarrierMonitoring::Continuous { start_step } if state.step >= *start_step => {
                    if state.step == *start_step {
                        breached(current_spot)
                    } else {
                        let dt = super::require_finite_state(
                            self.step_dts.get(state.step - 1).copied(),
                            "step_dts",
                            state.step,
                        )?;
                        let local_sigma = match state.variance() {
                            Some(v) if v.is_finite() && v >= 0.0 => v.sqrt(),
                            Some(v) => {
                                return Err(finstack_quant_core::Error::Validation(format!(
                                    "Barrier path variance must be finite and non-negative, got {v}"
                                )));
                            }
                            None => self.sigma,
                        };
                        check_barrier_hit(
                            self.previous_spot,
                            current_spot,
                            self.barrier,
                            self.barrier_type.direction(),
                            local_sigma,
                            dt,
                            state.uniform_random(),
                        )
                    }
                }
                BarrierMonitoring::Continuous { .. } => false,
                BarrierMonitoring::Discrete { observation_steps } => {
                    observation_steps.binary_search(&state.step).is_ok() && breached(current_spot)
                }
            };
            if hit {
                self.barrier_hit = true;
                self.hit_time = state.time;
            }
        }

        self.previous_spot = current_spot;
        if state.step == self.maturity_step {
            self.terminal_spot = current_spot;
        }
        Ok(())
    }

    fn value(&self, currency: Currency) -> finstack_quant_core::Result<Money> {
        if self.is_active() {
            let intrinsic = self.option_type.intrinsic(self.terminal_spot, self.strike);
            Money::new(intrinsic * self.notional, currency)
        } else {
            // Rebate payment. With at-hit timing configured and an actual
            // hit (knock-out), compound the rebate forward from τ to
            // maturity so the engine's DF(T) nets to DF(τ). A knock-in that
            // never touched the barrier has no hit time and pays at expiry.
            let mut rebate_amount = self.rebate.unwrap_or(0.0);
            if let Some(rate) = self.rebate_at_hit_rate {
                if self.barrier_hit {
                    rebate_amount *= (rate * (self.total_time - self.hit_time)).exp();
                }
            }
            Money::new(rebate_amount * self.notional, currency)
        }
    }

    fn reset(&mut self) {
        self.terminal_spot = 0.0;
        self.barrier_hit = self.initial_barrier_hit;
        self.previous_spot = 0.0;
        self.hit_time = 0.0;
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

/// Compute probability of hitting a barrier between two observations.
///
/// For a Brownian bridge from `S(t)` to `S(t+Δt)`, compute the probability
/// that the path crosses barrier `B` at some point in `[t, t+Δt]` assuming
/// log-normal GBM dynamics with constant volatility `σ`.
///
/// # Arguments
///
/// * `s_t` - Underlying spot observed at the start of the monitoring interval,
///   in the same price units as `barrier`.
/// * `s_t_dt` - Spot at time t+Δt
/// * `barrier` - Continuously monitored barrier level in the same price units
///   as the two spot observations.
/// * `sigma` - Instantaneous volatility over the step (constant assumption)
/// * `dt` - Positive monitoring-interval length in years; non-positive values
///   return zero unless discrete observations already cross the barrier.
///
/// # Returns
///
/// Probability of hitting the barrier in `[t, t+Δt]`.
///
/// # Algorithm
///
/// For log-space Brownian motion `X = ln(S)`, the closed-form hit
/// probability of the Brownian bridge (see Glasserman 2003, §6.4) is
///
/// ```text
/// p_hit ≈ exp(−2 · ln(S_t/B) · ln(S_{t+Δt}/B) / (σ² · Δt))
/// ```
///
/// when both `S_t` and `S_{t+Δt}` are on the same side of `B`; otherwise a
/// crossing is guaranteed.
#[must_use]
fn bridge_hit_probability(s_t: f64, s_t_dt: f64, barrier: f64, sigma: f64, dt: f64) -> f64 {
    // A barrier between the two observations is a guaranteed crossing
    // regardless of σ or Δt, so this check must run before the
    // degenerate-input guard: σ = 0 or Δt = 0 must not turn a definite
    // discrete crossing into probability 0.
    let min_s = s_t.min(s_t_dt);
    let max_s = s_t.max(s_t_dt);
    if barrier >= min_s && barrier <= max_s {
        return 1.0;
    }

    if dt <= 0.0 || sigma <= 0.0 {
        return 0.0;
    }

    // Both observations on same side of barrier
    if (s_t > barrier && s_t_dt > barrier) || (s_t < barrier && s_t_dt < barrier) {
        // Use log-space approximation
        let ln_ratio_t = (s_t / barrier).ln();
        let ln_ratio_t_dt = (s_t_dt / barrier).ln();

        // Brownian bridge formula
        let numerator = -2.0 * ln_ratio_t * ln_ratio_t_dt;
        let denominator = sigma * sigma * dt;

        if numerator / denominator < -20.0 {
            // Avoid numerical issues for very small probabilities
            return 0.0;
        }

        (numerator / denominator).exp().min(1.0)
    } else {
        // Observations on different sides - must have crossed
        1.0
    }
}

/// Check if a barrier was hit using bridge correction.
///
/// # Arguments
///
/// * `s_t` - Underlying spot at the start of the monitoring interval, in the
///   same price units as `barrier`.
/// * `s_t_dt` - Spot at time t+Δt
/// * `barrier` - Up or down barrier level in the same price units as the spot
///   observations.
/// * `barrier_type` - Type of barrier (up or down)
/// * `sigma` - Constant annualized volatility assumed over the interval.
/// * `dt` - Monitoring-interval length in years used by the bridge correction.
/// * `uniform_random` - Uniform random number in [0, 1)
///
/// # Returns
///
/// true if barrier was hit, false otherwise
#[allow(clippy::too_many_arguments)]
fn check_barrier_hit(
    s_t: f64,
    s_t_dt: f64,
    barrier: f64,
    barrier_type: BarrierDirection,
    sigma: f64,
    dt: f64,
    uniform_random: f64,
) -> bool {
    match barrier_type {
        BarrierDirection::Up => {
            if s_t >= barrier || s_t_dt >= barrier {
                return true;
            }
            let p_hit = bridge_hit_probability(s_t, s_t_dt, barrier, sigma, dt);
            // Textbook inverse-CDF test: hit iff u < p. `fill_u01`
            // guarantees u ∈ (0, 1), so no boundary guard is needed.
            uniform_random < p_hit
        }
        BarrierDirection::Down => {
            if s_t <= barrier || s_t_dt <= barrier {
                return true;
            }
            let p_hit = bridge_hit_probability(s_t, s_t_dt, barrier, sigma, dt);
            // Textbook inverse-CDF test: hit iff u < p. `fill_u01`
            // guarantees u ∈ (0, 1), so no boundary guard is needed.
            uniform_random < p_hit
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::traits::state_keys;
    use finstack_quant_core::math::time_grid::TimeGrid;

    fn create_path_state(step: usize, time: f64, spot: f64, uniform_random: f64) -> PathState {
        let mut state = PathState::new(step, time);
        state.set(state_keys::SPOT, spot);
        state.set_uniform_random(uniform_random);
        state
    }

    fn barrier_hit_at_initial_spot(barrier_type: BarrierType, spot: f64) -> bool {
        let grid = TimeGrid::uniform(1.0, 1).expect("grid should build");
        let mut payoff = BarrierOptionPayoff::new(
            100.0,
            100.0,
            barrier_type,
            OptionType::Call,
            None,
            1.0,
            1,
            0.2,
            &grid,
            BarrierMonitoring::Continuous { start_step: 0 },
        );
        payoff
            .on_event(&mut create_path_state(0, 0.0, spot, 0.5))
            .expect("valid payoff event");
        payoff.barrier_hit
    }

    #[test]
    fn barrier_type_serde_accepts_only_canonical_snake_case() {
        let barrier_type: BarrierType =
            serde_json::from_str("\"up_and_out\"").expect("canonical spelling should deserialize");
        assert_eq!(barrier_type, BarrierType::UpAndOut);
        assert_eq!(
            serde_json::to_string(&barrier_type).expect("barrier type should serialize"),
            "\"up_and_out\""
        );
        assert!(serde_json::from_str::<BarrierType>("\"UpAndOut\"").is_err());
    }

    #[test]
    fn initial_barrier_touch_uses_non_strict_inequalities_for_all_variants() {
        for barrier_type in [BarrierType::UpAndOut, BarrierType::UpAndIn] {
            assert!(barrier_hit_at_initial_spot(barrier_type, 100.0));
            assert!(!barrier_hit_at_initial_spot(barrier_type, 99.99));
        }
        for barrier_type in [BarrierType::DownAndOut, BarrierType::DownAndIn] {
            assert!(barrier_hit_at_initial_spot(barrier_type, 100.0));
            assert!(!barrier_hit_at_initial_spot(barrier_type, 100.01));
        }
    }

    /// At-hit knock-out rebates are compounded forward from the hit time τ
    /// to maturity at the configured rate, so the engine's DF(T) nets to
    /// DF(τ): value = R·e^{r(T−τ)}·N.
    #[test]
    fn test_rebate_at_hit_grows_forward_from_hit_time() {
        let rate = 0.05;
        let rebate = 5.0;
        let grid = TimeGrid::uniform(1.0, 10).expect("grid should build");
        let mut payoff = BarrierOptionPayoff::new(
            100.0,
            120.0,
            BarrierType::UpAndOut,
            OptionType::Call,
            Some(rebate),
            1.0,
            10,
            0.2,
            &grid,
            BarrierMonitoring::Continuous { start_step: 0 },
        )
        .with_rebate_at_hit(rate);

        // Discrete breach at step 4 ⇒ τ = 0.4 on the uniform 0.1 grid.
        payoff
            .on_event(&mut create_path_state(0, 0.0, 100.0, 0.5))
            .expect("valid payoff event");
        for step in 1..=10usize {
            let spot = if step >= 4 { 125.0 } else { 100.0 };
            payoff
                .on_event(&mut create_path_state(step, step as f64 * 0.1, spot, 0.5))
                .expect("valid payoff event");
        }

        let expected = rebate * (rate * (1.0 - 0.4)).exp();
        let value = payoff.value(Currency::USD).expect("valid payoff").amount();
        assert!(
            (value - expected).abs() < 1e-10,
            "at-hit rebate should compound forward: got {value}, expected {expected}"
        );

        // Without at-hit timing the same path pays the flat rebate.
        let mut payoff_expiry = BarrierOptionPayoff::new(
            100.0,
            120.0,
            BarrierType::UpAndOut,
            OptionType::Call,
            Some(rebate),
            1.0,
            10,
            0.2,
            &grid,
            BarrierMonitoring::Continuous { start_step: 0 },
        );
        payoff_expiry
            .on_event(&mut create_path_state(0, 0.0, 100.0, 0.5))
            .expect("valid payoff event");
        for step in 1..=10usize {
            let spot = if step >= 4 { 125.0 } else { 100.0 };
            payoff_expiry
                .on_event(&mut create_path_state(step, step as f64 * 0.1, spot, 0.5))
                .expect("valid payoff event");
        }
        assert!(
            (payoff_expiry
                .value(Currency::USD)
                .expect("valid payoff")
                .amount()
                - rebate)
                .abs()
                < 1e-10
        );
    }

    /// A knock-in that never touches the barrier pays its rebate at expiry
    /// even when at-hit timing is configured — there is no hit time.
    #[test]
    fn test_rebate_at_hit_leaves_knock_in_expiry_rebate_unchanged() {
        let rebate = 5.0;
        let grid = TimeGrid::uniform(1.0, 10).expect("grid should build");
        let mut payoff = BarrierOptionPayoff::new(
            100.0,
            120.0,
            BarrierType::UpAndIn,
            OptionType::Call,
            Some(rebate),
            1.0,
            10,
            0.2,
            &grid,
            BarrierMonitoring::Continuous { start_step: 0 },
        )
        .with_rebate_at_hit(0.05);

        payoff
            .on_event(&mut create_path_state(0, 0.0, 100.0, 0.5))
            .expect("valid payoff event");
        for step in 1..=10usize {
            payoff
                .on_event(&mut create_path_state(step, step as f64 * 0.1, 100.0, 0.5))
                .expect("valid payoff event");
        }
        assert!(
            (payoff.value(Currency::USD).expect("valid payoff").amount() - rebate).abs() < 1e-10
        );
    }

    #[test]
    fn test_barrier_put_payoff() {
        let mut barrier_put = BarrierOptionPayoff::new(
            100.0,
            120.0,
            BarrierType::UpAndOut,
            OptionType::Put,
            None,
            1.0,
            10,
            0.2,
            &TimeGrid::uniform(1.0, 10).expect("grid should build"),
            BarrierMonitoring::Continuous { start_step: 0 },
        );

        // Path that never hits barrier (active)
        for step in 0..=10 {
            let spot = 90.0; // Below barrier, below strike (ITM put)
            let mut state = create_path_state(step, step as f64 * 0.1, spot, 0.5);
            barrier_put
                .on_event(&mut state)
                .expect("valid payoff event");
        }

        // Should get put payoff (100 - 90 = 10)
        let value = barrier_put.value(Currency::USD).expect("valid payoff");
        assert_eq!(value.amount(), 10.0);
    }

    #[test]
    fn test_barrier_rebate() {
        let rebate = 5.0;
        let mut barrier_call = BarrierOptionPayoff::new(
            100.0,
            120.0,
            BarrierType::UpAndOut,
            OptionType::Call,
            Some(rebate),
            1.0,
            10,
            0.2,
            &TimeGrid::uniform(1.0, 10).expect("grid should build"),
            BarrierMonitoring::Continuous { start_step: 0 },
        );

        // Hit barrier
        let mut s1 = create_path_state(0, 0.0, 105.0, 0.5);
        barrier_call.on_event(&mut s1).expect("valid payoff event");
        let mut s2 = create_path_state(1, 0.1, 125.0, 0.5); // Hit
        barrier_call.on_event(&mut s2).expect("valid payoff event");
        let mut s3 = create_path_state(10, 1.0, 130.0, 0.5); // Terminal
        barrier_call.on_event(&mut s3).expect("valid payoff event");

        let value = barrier_call.value(Currency::USD).expect("valid payoff");
        assert_eq!(value.amount(), 5.0);
    }

    #[test]
    fn test_barrier_uses_path_variance_when_present() {
        // Two barrier payoffs with identical parameters but different fallback
        // sigmas. When the path state carries a stochastic variance, both
        // must agree because the payoff consumes sqrt(variance) instead of
        // self.sigma.
        let time_grid = TimeGrid::uniform(1.0, 4).expect("grid should build");

        let build = |flat_sigma: f64| {
            BarrierOptionPayoff::new(
                100.0,
                120.0,
                BarrierType::UpAndOut,
                OptionType::Call,
                None,
                1.0,
                4,
                flat_sigma,
                &time_grid,
                BarrierMonitoring::Continuous { start_step: 0 },
            )
        };

        let mut p_low = build(0.01);
        let mut p_high = build(1.0);

        let stoch_var = 0.04_f64;
        let feed = |payoff: &mut BarrierOptionPayoff| {
            for (step, spot) in [
                (0usize, 95.0),
                (1, 105.0),
                (2, 115.0),
                (3, 110.0),
                (4, 118.0),
            ] {
                let mut state = PathState::new(step, step as f64 * 0.25);
                state.set(state_keys::SPOT, spot);
                state.set(state_keys::VARIANCE, stoch_var);
                state.set_uniform_random(0.5);
                payoff.on_event(&mut state).expect("valid payoff event");
            }
        };

        feed(&mut p_low);
        feed(&mut p_high);

        let v_low = p_low.value(Currency::USD).expect("valid payoff").amount();
        let v_high = p_high.value(Currency::USD).expect("valid payoff").amount();
        assert_eq!(
            v_low, v_high,
            "payoff must use sqrt(variance) from PathState when present, \
             ignoring the fallback self.sigma",
        );
    }

    #[test]
    fn production_barrier_zero_path_variance_has_no_bridge_crossing() {
        let time_grid = TimeGrid::uniform(1.0, 1).expect("grid");
        let mut payoff = BarrierOptionPayoff::new(
            100.0,
            120.0,
            BarrierType::UpAndOut,
            OptionType::Call,
            None,
            1.0,
            1,
            2.0,
            &time_grid,
            BarrierMonitoring::Continuous { start_step: 0 },
        );
        for (step, spot) in [(0, 110.0), (1, 115.0)] {
            let mut state = PathState::new(step, step as f64);
            state.set(state_keys::SPOT, spot);
            state.set(state_keys::VARIANCE, 0.0);
            state.set_uniform_random(0.5);
            payoff.on_event(&mut state).expect("zero-variance event");
        }
        assert_eq!(payoff.value(Currency::USD).expect("payoff").amount(), 15.0);
    }

    #[test]
    fn test_barrier_falls_back_to_self_sigma_without_variance() {
        // When variance is absent from PathState, the payoff must use
        // self.sigma for the bridge/Gobet-Miri adjustment. Two payoffs with
        // different flat sigmas should behave differently.
        let time_grid = TimeGrid::uniform(1.0, 4).expect("grid should build");

        let build = |flat_sigma: f64| {
            BarrierOptionPayoff::new(
                100.0,
                120.0,
                BarrierType::UpAndOut,
                OptionType::Call,
                None,
                1.0,
                4,
                flat_sigma,
                &time_grid,
                BarrierMonitoring::Continuous { start_step: 0 },
            )
        };

        let mut p_low = build(0.01);
        let mut p_high = build(1.0);

        let feed = |payoff: &mut BarrierOptionPayoff| {
            for (step, spot) in [
                (0usize, 95.0),
                (1, 105.0),
                (2, 115.0),
                (3, 110.0),
                (4, 118.0),
            ] {
                let mut state = PathState::new(step, step as f64 * 0.25);
                state.set(state_keys::SPOT, spot);
                state.set_uniform_random(0.5);
                payoff.on_event(&mut state).expect("valid payoff event");
            }
        };

        feed(&mut p_low);
        feed(&mut p_high);

        let v_low = p_low.value(Currency::USD).expect("valid payoff").amount();
        let v_high = p_high.value(Currency::USD).expect("valid payoff").amount();
        // At least one of the two must differ — for a near-barrier path the
        // Gobet-Miri adjustment is very sensitive to sigma, so the payoff
        // should not be identical when the flat sigma is the only input.
        assert!(
            (v_low - v_high).abs() > 0.0 || v_low + v_high == 0.0,
            "without variance in PathState the payoff must respond to self.sigma; \
             got v_low={v_low}, v_high={v_high}"
        );
    }

    #[test]
    fn test_bridge_uses_unshifted_barrier_no_double_correction() {
        // W-43: when the Brownian bridge is active (it always is inside
        // `check_barrier_hit`), the barrier passed to it must be the TRUE,
        // unshifted barrier. Layering the BGK `exp(±βσ√Δt)` shift on top of
        // the exact bridge crossing law over-corrects.
        //
        // Construct a down-and-out path whose closing spot is strictly
        // *below* the true barrier — that is an unconditional discrete
        // knockout. A BGK-shifted barrier would move the
        // barrier DOWN (away from spot) so the close lands *above* the
        // shifted barrier, turning a definite hit into a mere bridge
        // probability that a high uniform draw rejects.
        let time_grid = TimeGrid::uniform(1.0, 4).expect("grid should build");
        let mut payoff = BarrierOptionPayoff::new(
            110.0, // strike (put ITM at terminal spot 95)
            100.0, // true barrier
            BarrierType::DownAndOut,
            OptionType::Put,
            None,
            1.0,
            4,
            0.2, // sigma — with dt=0.25 the BGK shift is ~5.7%
            &time_grid,
            BarrierMonitoring::Continuous { start_step: 0 },
        );

        // step 0: spot 105, above barrier (no initial breach).
        let mut s0 = PathState::new(0, 0.0);
        s0.set(state_keys::SPOT, 105.0);
        s0.set_uniform_random(0.999);
        payoff.on_event(&mut s0).expect("valid payoff event");

        // step 1: spot 95 — strictly below the true barrier of 100, so the
        // option must knock out unconditionally. A high uniform draw of
        // 0.999 ensures the bridge probability path cannot register the hit
        // if the (wrongly) shifted barrier were used.
        let mut s1 = PathState::new(1, 0.25);
        s1.set(state_keys::SPOT, 95.0);
        s1.set_uniform_random(0.999);
        payoff.on_event(&mut s1).expect("valid payoff event");

        // Remaining steps stay below barrier; terminal spot 95.
        for (step, spot) in [(2usize, 96.0), (3, 97.0), (4, 95.0)] {
            let mut s = PathState::new(step, step as f64 * 0.25);
            s.set(state_keys::SPOT, spot);
            s.set_uniform_random(0.999);
            payoff.on_event(&mut s).expect("valid payoff event");
        }

        // The option is knocked out: value must be 0 (no rebate).
        let value = payoff.value(Currency::USD).expect("valid payoff").amount();
        assert_eq!(
            value, 0.0,
            "a down-and-out path closing below the true barrier must knock \
             out; a non-zero value means the BGK shift was applied on top of \
             the bridge and moved the barrier away from spot (W-43)",
        );
    }
    #[test]
    fn discrete_monitoring_ignores_between_observation_crossings() {
        let time_grid = TimeGrid::from_times(vec![0.0, 0.5, 1.0]).expect("grid should build");
        let mut payoff = BarrierOptionPayoff::new(
            80.0,
            100.0,
            BarrierType::UpAndOut,
            OptionType::Call,
            None,
            1.0,
            2,
            0.2,
            &time_grid,
            BarrierMonitoring::Discrete {
                observation_steps: vec![2],
            },
        );

        let mut initial = create_path_state(0, 0.0, 90.0, 0.5);
        payoff
            .on_event(&mut initial)
            .expect("initial state should be valid");
        let mut between_observations = create_path_state(1, 0.5, 110.0, 0.5);
        payoff
            .on_event(&mut between_observations)
            .expect("unobserved crossing should be ignored");
        let mut observation = create_path_state(2, 1.0, 90.0, 0.5);
        payoff
            .on_event(&mut observation)
            .expect("contractual observation should be valid");

        assert_eq!(
            payoff.value(Currency::USD).expect("valid payoff").amount(),
            10.0
        );
        assert!(!payoff.needs_uniform_random());
    }

    #[test]
    fn test_barrier_uses_step_dt_for_irregular_grid() {
        let time_grid = TimeGrid::from_times(vec![0.0, 0.2, 1.0]).expect("grid should build");
        let mut barrier_call = BarrierOptionPayoff::new(
            80.0,
            100.0,
            BarrierType::UpAndOut,
            OptionType::Call,
            None,
            1.0,
            2,
            0.2,
            &time_grid,
            BarrierMonitoring::Continuous { start_step: 0 },
        );

        let mut s0 = create_path_state(0, 0.0, 90.0, 0.65);
        barrier_call.on_event(&mut s0).expect("valid payoff event");
        let mut s1 = create_path_state(1, 0.2, 95.0, 0.65);
        barrier_call.on_event(&mut s1).expect("valid payoff event");
        let mut s2 = create_path_state(2, 1.0, 90.0, 0.65);
        barrier_call.on_event(&mut s2).expect("valid payoff event");

        assert_eq!(
            barrier_call
                .value(Currency::USD)
                .expect("valid payoff")
                .amount(),
            0.0
        );
    }
}

#[cfg(test)]
mod bridge_tests {
    use super::*;

    #[test]
    fn test_bridge_hit_probability_definite_hit() {
        let p = bridge_hit_probability(90.0, 110.0, 100.0, 0.2, 0.1);
        assert_eq!(p, 1.0);
    }

    #[test]
    fn test_bridge_hit_probability_same_side() {
        // Both above barrier
        let p = bridge_hit_probability(110.0, 120.0, 100.0, 0.2, 0.1);
        assert!(p > 0.0 && p < 1.0);
    }

    #[test]
    fn test_bridge_hit_probability_far_apart() {
        // Far from barrier - low probability
        let p = bridge_hit_probability(150.0, 160.0, 100.0, 0.2, 0.1);
        assert!(p < 0.01);
    }

    #[test]
    fn test_barrier_check_discrete_hit_up() {
        assert!(check_barrier_hit(
            95.0,
            105.0,
            100.0,
            BarrierDirection::Up,
            0.2,
            0.1,
            0.5
        ));
        assert!(check_barrier_hit(
            105.0,
            110.0,
            100.0,
            BarrierDirection::Up,
            0.2,
            0.1,
            0.5
        ));
    }

    #[test]
    fn test_barrier_check_no_hit_up() {
        // Well below barrier: the bridge hit probability here is ≈ 1.3e-8,
        // so any ordinary uniform draw must report no hit. (`fill_u01`
        // guarantees u ∈ (0, 1), so exactly-zero draws cannot occur.)
        assert!(!check_barrier_hit(
            80.0,
            85.0,
            100.0,
            BarrierDirection::Up,
            0.2,
            0.1,
            0.5
        ));
    }

    #[test]
    fn test_barrier_check_discrete_hit_down() {
        assert!(check_barrier_hit(
            105.0,
            95.0,
            100.0,
            BarrierDirection::Down,
            0.2,
            0.1,
            0.5
        ));
        assert!(check_barrier_hit(
            95.0,
            90.0,
            100.0,
            BarrierDirection::Down,
            0.2,
            0.1,
            0.5
        ));
    }

    #[test]
    fn test_barrier_symmetry() {
        // Up and down should be symmetric
        let p_up = bridge_hit_probability(110.0, 120.0, 100.0, 0.2, 0.1);
        let p_down = bridge_hit_probability(90.0, 80.0, 100.0, 0.2, 0.1);

        assert!((p_up - p_down).abs() < 0.1);
    }
}
