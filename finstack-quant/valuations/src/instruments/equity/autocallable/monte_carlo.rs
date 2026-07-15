//! Autocallable structured product payoffs for Monte Carlo pricing.
//!
//! Autocallable products have early redemption features where the option
//! is automatically called (redeemed) if certain barrier conditions are met
//! at observation dates.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error as CoreError;
use finstack_quant_monte_carlo::traits::PathState;
use finstack_quant_monte_carlo::traits::Payoff;

/// Backward-compatible Monte Carlo path for the canonical instrument enum.
pub use super::types::FinalPayoffType;

/// Autocallable structured product payoff.
///
/// At each observation date, if spot >= autocall_barrier, the product
/// is redeemed early with coupon + principal.
///
/// If not autocalled, final payoff depends on FinalPayoffType and barriers.
#[derive(Debug, Clone)]
pub struct AutocallablePayoff {
    /// Observation dates (time in years from valuation)
    pub observation_dates: Vec<f64>,
    /// Autocall barrier levels at each observation date
    pub autocall_barriers: Vec<f64>,
    /// Coupon payments if autocalled at each date
    pub coupons: Vec<f64>,
    /// Memory ("Phoenix") coupon feature.
    ///
    /// When `true`, coupons from earlier observation dates whose barrier was
    /// not met are accrued and paid in full on the autocall date. When
    /// `false`, only the coupon at the autocall date is paid.
    pub memory_coupons: bool,
    /// Final barrier level (for knock-in/knock-out)
    pub final_barrier: f64,
    /// Final payoff structure
    pub final_payoff_type: FinalPayoffType,
    /// Participation rate for final payoff
    pub participation_rate: f64,
    /// Cap level for returns (e.g., 1.2 for 20% cap)
    pub cap_level: f64,
    /// Notional amount
    pub notional: f64,
    /// Currency
    pub currency: Currency,
    /// Initial spot price
    pub initial_spot: f64,
    /// Discount factor ratios (DF(t_obs) / DF(t_mat)) for correcting early cashflow PV
    pub df_ratios: Vec<f64>,
    /// Seed for `min_spot_observed` from past (already observed) fixings of a
    /// seasoned trade. `f64::INFINITY` for a new trade.
    seed_min_spot: f64,
    /// Seed for `max_spot_observed` from past fixings. `f64::NEG_INFINITY`
    /// for a new trade.
    seed_max_spot: f64,
    /// Accrued memory ("Phoenix") coupons missed at past observation dates of
    /// a seasoned trade, paid on a future autocall when `memory_coupons` is
    /// enabled. Zero for a new trade.
    prior_memory_coupons: f64,

    // State variables (tracked during path simulation)
    /// Index of observation date when autocalled (None if not autocalled)
    autocalled_at: Option<usize>,
    /// Index of next observation date to check (ensures each date is only checked once)
    next_obs_idx: usize,
    /// Minimum spot observed (for knock-in barriers)
    min_spot_observed: f64,
    /// Maximum spot observed (for knock-out barriers)
    max_spot_observed: f64,
    /// Final spot price
    final_spot: f64,
}

impl AutocallablePayoff {
    /// Get the final spot value (for testing/debugging).
    #[cfg(test)]
    pub fn final_spot(&self) -> f64 {
        self.final_spot
    }

    /// Create a new autocallable payoff.
    ///
    /// # Arguments
    ///
    /// * `observation_dates` - Dates when autocall barriers are checked (must be sorted)
    /// * `autocall_barriers` - Barrier levels at each observation date
    /// * `coupons` - Coupon payments if autocalled at each date
    /// * `memory_coupons` - When `true`, earlier missed coupons accrue and are
    ///   paid on the autocall date (Phoenix feature)
    /// * `final_barrier` - Barrier for final payoff (knock-in/knock-out)
    /// * `final_payoff_type` - Type of final payoff
    /// * `participation_rate` - Participation rate for final payoff
    /// * `cap_level` - Maximum return cap
    /// * `notional` - Notional amount
    /// * `currency` - Currency
    /// * `initial_spot` - Initial spot price S_0
    /// * `df_ratios` - Discount factor ratios DF(T_obs)/DF(T_mat) for each observation date
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        observation_dates: Vec<f64>,
        autocall_barriers: Vec<f64>,
        coupons: Vec<f64>,
        memory_coupons: bool,
        final_barrier: f64,
        final_payoff_type: FinalPayoffType,
        participation_rate: f64,
        cap_level: f64,
        notional: f64,
        currency: Currency,
        initial_spot: f64,
        df_ratios: Vec<f64>,
    ) -> finstack_quant_core::Result<Self> {
        if observation_dates.len() != autocall_barriers.len() {
            return Err(CoreError::Validation(format!(
                "AutocallablePayoff: observation_dates ({}) and autocall_barriers ({}) must have the same length",
                observation_dates.len(),
                autocall_barriers.len()
            )));
        }
        if observation_dates.len() != coupons.len() {
            return Err(CoreError::Validation(format!(
                "AutocallablePayoff: observation_dates ({}) and coupons ({}) must have the same length",
                observation_dates.len(),
                coupons.len()
            )));
        }
        if observation_dates.len() != df_ratios.len() {
            return Err(CoreError::Validation(format!(
                "AutocallablePayoff: observation_dates ({}) and df_ratios ({}) must have the same length",
                observation_dates.len(),
                df_ratios.len()
            )));
        }
        for i in 1..observation_dates.len() {
            if observation_dates[i - 1] >= observation_dates[i] {
                return Err(CoreError::Validation(format!(
                    "AutocallablePayoff: observation_dates must be strictly increasing (index {} = {} >= index {} = {})",
                    i - 1,
                    observation_dates[i - 1],
                    i,
                    observation_dates[i]
                )));
            }
        }

        Ok(Self {
            observation_dates,
            autocall_barriers,
            coupons,
            memory_coupons,
            final_barrier,
            final_payoff_type,
            participation_rate,
            cap_level,
            notional,
            currency,
            initial_spot,
            df_ratios,
            seed_min_spot: f64::INFINITY,
            seed_max_spot: f64::NEG_INFINITY,
            prior_memory_coupons: 0.0,
            autocalled_at: None,
            next_obs_idx: 0,
            min_spot_observed: f64::INFINITY,
            max_spot_observed: f64::NEG_INFINITY,
            final_spot: 0.0, // Will be set when at maturity
        })
    }

    /// Seed the payoff with the deterministic state of a seasoned trade.
    ///
    /// * `prior_min_spot` / `prior_max_spot` — min/max of the observed past
    ///   fixings (discrete knock-in monitoring carries across `as_of`).
    /// * `prior_memory_coupons` — sum of coupons missed at past observation
    ///   dates, released on a future autocall when memory is enabled.
    #[must_use]
    pub fn with_seasoned_state(
        mut self,
        prior_min_spot: f64,
        prior_max_spot: f64,
        prior_memory_coupons: f64,
    ) -> Self {
        self.seed_min_spot = prior_min_spot;
        self.seed_max_spot = prior_max_spot;
        self.prior_memory_coupons = prior_memory_coupons;
        self.min_spot_observed = prior_min_spot;
        self.max_spot_observed = prior_max_spot;
        self
    }

    /// Final (non-autocalled) payoff as a ratio of notional, given the spot at
    /// the final observation and the discretely-monitored minimum spot.
    ///
    /// Shared between the path payoff (`value`) and the deterministic
    /// all-observations-past branch of the pricer.
    pub fn final_payoff_ratio(&self, final_spot: f64, min_spot_observed: f64) -> f64 {
        match self.final_payoff_type {
            FinalPayoffType::CapitalProtection { floor } => {
                // Use final_spot directly (defaults to 0.0 if never set, which will hit the floor)
                let return_ratio = (final_spot / self.initial_spot).min(self.cap_level);
                let participation_term = self.participation_rate * return_ratio;
                floor.max(participation_term)
            }
            FinalPayoffType::Participation { rate } => {
                let capped_ratio = (final_spot / self.initial_spot).min(self.cap_level);
                1.0 + rate * ((capped_ratio - 1.0).max(0.0))
            }
            FinalPayoffType::KnockInPut { strike } => {
                let barrier_level = self.initial_spot * self.final_barrier;
                if min_spot_observed <= barrier_level {
                    // Knocked in: the note holder is short a down-and-in put, so
                    // they receive principal reduced by the put loss, floored at
                    // zero — NOT the bare put intrinsic. A knocked-in path ending
                    // at-the-money returns full principal (put worth ~0).
                    let strike_ratio = strike / self.initial_spot;
                    let spot_ratio = final_spot / self.initial_spot;
                    let put_loss = (strike_ratio - spot_ratio).max(0.0);
                    (1.0 - put_loss).max(0.0)
                } else {
                    1.0
                }
            }
        }
    }
}

impl Payoff for AutocallablePayoff {
    fn on_event(&mut self, state: &mut PathState) {
        let Some(spot) = state.spot().filter(|spot| spot.is_finite() && *spot > 0.0) else {
            return;
        };

        // Check autocall — and monitor the knock-in barrier — at the discrete
        // observation dates only.
        //
        // The knock-in barrier (`min_spot_observed` / `max_spot_observed`) is
        // contractually monitored *discretely* at the observation dates (see
        // the `Barrier Monitoring Convention` section of `types.rs`). Updating
        // it on every MC time step would amount to continuous monitoring on a
        // dense grid, which over-counts barrier breaches and mis-prices the
        // knock-in. Min/max are therefore recorded inside the observation-date
        // loop, one sample per observation date, not once per time step.
        if self.autocalled_at.is_none() {
            const EPS: f64 = 1e-6;
            // Consume every observation date now due. A single MC time step can
            // jump past multiple observation dates (coarse grid, or final step);
            // each must be evaluated in order so a barrier breach at any of them
            // is not silently skipped.
            while self.next_obs_idx < self.observation_dates.len() {
                let idx = self.next_obs_idx;
                let obs_date = self.observation_dates[idx];
                // Forward-looking check: we're at or past this observation date
                // (avoids missing dates when MC time steps don't align exactly).
                if state.time < obs_date - EPS {
                    break;
                }
                self.next_obs_idx = idx + 1;
                // Discrete knock-in monitoring: record the spot at this
                // observation date for the final-barrier check.
                self.min_spot_observed = self.min_spot_observed.min(spot);
                self.max_spot_observed = self.max_spot_observed.max(spot);
                let barrier_level = self.initial_spot * self.autocall_barriers[idx];
                if spot >= barrier_level {
                    // Autocall at the first date whose barrier is breached.
                    self.autocalled_at = Some(idx);
                    break;
                }
            }
        }

        // Store final spot at maturity
        // Assume maturity is the last observation date (or can be set separately)
        if let Some(&last_date) = self.observation_dates.last() {
            // Update final spot if we're at or past the last observation date
            // Check if we're at the observation date (within epsilon for floating point)
            let is_at_maturity = (state.time - last_date).abs() < 1e-10 || state.time >= last_date;
            if is_at_maturity {
                // Always update final_spot if we're at maturity.
                self.final_spot = spot;
            }
        } else {
            // If no observation dates, use current spot as final
            self.final_spot = spot;
        }
    }

    fn value(&self, currency: Currency) -> Money {
        // If autocalled early
        if let Some(idx) = self.autocalled_at {
            // Coupon paid on autocall.
            //
            // - Without memory: only the coupon at the autocall date `idx`.
            // - With memory ("Phoenix"): every coupon from observation dates
            //   0..=idx accrues and is paid in full on the autocall date.
            //   Earlier observation barriers were necessarily *not* met (the
            //   product would have autocalled there otherwise), so those
            //   coupons were "remembered" and are now released.
            let coupon: f64 = if self.memory_coupons {
                // Seasoned trades also release coupons missed at past
                // observation dates (deterministically known at pricing time).
                self.prior_memory_coupons + self.coupons[..=idx].iter().sum::<f64>()
            } else {
                self.coupons[idx]
            };
            // Return coupon + principal.
            // Adjust for discounting: The engine applies DF(T_mat), but the
            // autocall cashflow (principal *and* all accrued memory coupons)
            // settles on the autocall date T_obs.
            // value * DF(T_mat) = Payoff * DF(T_obs)
            // value = Payoff * (DF(T_obs) / DF(T_mat))
            let payoff = (coupon + 1.0) * self.notional;
            let adjusted_payoff = payoff * self.df_ratios[idx];
            return Money::new(adjusted_payoff, currency);
        }

        // Final payoff (not autocalled)
        let final_payoff = self.final_payoff_ratio(self.final_spot, self.min_spot_observed);

        Money::new(final_payoff * self.notional, currency)
    }

    fn reset(&mut self) {
        self.autocalled_at = None;
        self.next_obs_idx = 0;
        // Restore the seeded seasoned state (INFINITY/NEG_INFINITY/0 for a
        // new trade), not the bare defaults, so every path starts from the
        // same deterministic past.
        self.min_spot_observed = self.seed_min_spot;
        self.max_spot_observed = self.seed_max_spot;
        self.final_spot = 0.0; // Reset to default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_monte_carlo::traits::state_keys;

    #[test]
    fn test_autocallable_creation() {
        let observation_dates = vec![0.25, 0.5, 0.75, 1.0];
        let barriers = vec![1.05, 1.05, 1.05, 1.05];
        let coupons = vec![0.08, 0.08, 0.08, 0.10];

        let payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            false, // memory_coupons
            0.75,  // Final barrier
            FinalPayoffType::CapitalProtection { floor: 0.9 },
            1.0, // Participation rate
            1.2, // Cap level
            100_000.0,
            Currency::USD,
            100.0,                    // Initial spot
            vec![1.0, 1.0, 1.0, 1.0], // df_ratios
        )
        .expect("test fixture is well-formed");

        assert_eq!(payoff.observation_dates.len(), 4);
        assert_eq!(payoff.initial_spot, 100.0);
        assert!(payoff.autocalled_at.is_none());
    }

    #[test]
    fn test_autocallable_early_exercise() {
        let observation_dates = vec![0.25, 0.5];
        let barriers = vec![1.05, 1.05];
        let coupons = vec![0.08, 0.10];

        let mut payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            false, // memory_coupons
            0.75,
            FinalPayoffType::CapitalProtection { floor: 0.9 },
            1.0,
            1.2,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0, 1.0],
        )
        .expect("test fixture is well-formed");

        // Simulate first observation date with spot above barrier
        let mut state = PathState::new(10, 0.25);
        state.set(state_keys::SPOT, 106.0); // Above 105 barrier

        payoff.on_event(&mut state);

        assert_eq!(payoff.autocalled_at, Some(0));

        let value = payoff.value(Currency::USD);
        // Should be coupon (0.08) + principal (1.0) = 1.08 * notional
        assert!((value.amount() - 108_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_autocallable_capital_protection() {
        let observation_dates = vec![1.0];
        let barriers = vec![1.20]; // Very high barrier, unlikely to hit
        let coupons = vec![0.0];

        let mut payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            false, // memory_coupons
            0.75,
            FinalPayoffType::CapitalProtection { floor: 0.9 },
            1.0,
            1.2,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0],
        )
        .expect("test fixture is well-formed");

        // Not autocalled, final spot is below initial
        let mut state = PathState::new(100, 1.0);
        state.set(state_keys::SPOT, 80.0); // Below initial

        // Verify spot is set correctly
        assert_eq!(state.spot(), Some(80.0), "Spot should be set to 80.0");

        payoff.on_event(&mut state);

        let value = payoff.value(Currency::USD);

        // Capital protection: max(0.9, 1.0 * 0.8) = 0.9
        // Expected: 90_000.0 (0.9 * 100_000.0)
        assert!(
            (value.amount() - 90_000.0).abs() < 1e-6,
            "Expected 90_000.0 but got {}. final_spot={}",
            value.amount(),
            payoff.final_spot()
        );
    }

    #[test]
    fn missing_spot_event_leaves_payoff_state_unchanged() {
        let mut payoff = AutocallablePayoff::new(
            vec![1.0],
            vec![2.0],
            vec![0.0],
            false, // memory_coupons
            0.75,
            FinalPayoffType::Participation { rate: 1.0 },
            1.0,
            1.5,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0],
        )
        .expect("test fixture is well-formed");

        let mut state = PathState::new(100, 1.0);
        payoff.on_event(&mut state);

        assert_eq!(payoff.autocalled_at, None);
        assert_eq!(payoff.next_obs_idx, 0);
        assert_eq!(payoff.final_spot(), 0.0);
        assert_eq!(payoff.min_spot_observed, f64::INFINITY);
        assert_eq!(payoff.max_spot_observed, f64::NEG_INFINITY);
    }

    #[test]
    fn test_autocallable_reset() {
        let observation_dates = vec![0.25];
        let barriers = vec![1.05];
        let coupons = vec![0.08];

        let mut payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            false, // memory_coupons
            0.75,
            FinalPayoffType::CapitalProtection { floor: 0.9 },
            1.0,
            1.2,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0],
        )
        .expect("test fixture is well-formed");

        let mut state = PathState::new(10, 0.25);
        state.set(state_keys::SPOT, 106.0);
        payoff.on_event(&mut state);
        assert!(payoff.autocalled_at.is_some());

        payoff.reset();
        assert!(payoff.autocalled_at.is_none());
        assert_eq!(payoff.min_spot_observed, f64::INFINITY);
    }

    #[test]
    fn test_final_knock_in_barrier_scales_from_initial_spot() {
        let notional = 100_000.0;
        let mut payoff = AutocallablePayoff::new(
            vec![1.0],
            vec![2.0],
            vec![0.0],
            false, // memory_coupons
            0.6,
            FinalPayoffType::KnockInPut { strike: 100.0 },
            1.0,
            1.2,
            notional,
            Currency::USD,
            100.0,
            vec![1.0],
        )
        .expect("test fixture is well-formed");

        let mut state = PathState::new(100, 1.0);
        state.set(state_keys::SPOT, 55.0);
        payoff.on_event(&mut state);

        let value = payoff.value(Currency::USD);
        // Knocked in at spot=55, strike=100, S0=100: put loss = max(1.0 - 0.55, 0)
        // = 0.45, and the note pays principal - put_loss = 1.0 - 0.45 = 0.55.
        let expected = 0.55 * notional;
        assert!(
            (value.amount() - expected).abs() < 1e-6,
            "A 60% final barrier should knock in when spot hits 55 on a 100 initial spot, \
             paying principal minus the put loss (0.55 x notional); got {}",
            value.amount()
        );
    }

    #[test]
    fn coarse_step_spanning_multiple_observation_dates_evaluates_all() {
        // A single MC time step jumps past three observation dates. The barrier
        // is breached only at the third date; the autocallable must still
        // evaluate dates 0 and 1 (advancing next_obs_idx past them) and then
        // autocall at index 2 rather than silently skipping the due dates.
        let observation_dates = vec![0.25, 0.5, 0.75, 1.0];
        let barriers = vec![2.0, 2.0, 1.05, 2.0];
        let coupons = vec![0.05, 0.06, 0.07, 0.08];

        let mut payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            false, // memory_coupons
            0.75,
            FinalPayoffType::CapitalProtection { floor: 0.9 },
            1.0,
            1.2,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0, 1.0, 1.0, 1.0],
        )
        .expect("test fixture is well-formed");

        // One coarse step lands at t = 0.75, past observation dates 0, 1 and 2.
        let mut state = PathState::new(1, 0.75);
        state.set(state_keys::SPOT, 106.0); // Above 105 barrier (index 2 only)
        payoff.on_event(&mut state);

        assert_eq!(
            payoff.autocalled_at,
            Some(2),
            "autocall must fire at the first breached due date even when a step spans several"
        );
        assert_eq!(
            payoff.next_obs_idx, 3,
            "skipped due dates 0 and 1 must be consumed before the breached date"
        );
    }

    #[test]
    fn final_step_consumes_all_remaining_observation_dates() {
        // The final MC step lands at maturity, past every observation date.
        // No barrier is breached, so all dates must be consumed without autocall.
        let observation_dates = vec![0.25, 0.5, 0.75, 1.0];
        let barriers = vec![2.0, 2.0, 2.0, 2.0];
        let coupons = vec![0.05, 0.06, 0.07, 0.08];

        let mut payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            false, // memory_coupons
            0.75,
            FinalPayoffType::CapitalProtection { floor: 0.9 },
            1.0,
            1.2,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0, 1.0, 1.0, 1.0],
        )
        .expect("test fixture is well-formed");

        let mut state = PathState::new(1, 1.0);
        state.set(state_keys::SPOT, 110.0); // Below every 200 barrier
        payoff.on_event(&mut state);

        assert_eq!(payoff.autocalled_at, None);
        assert_eq!(
            payoff.next_obs_idx, 4,
            "every observation date due at the final step must be consumed"
        );
    }

    #[test]
    fn test_participation_payoff_respects_cap_level() {
        let mut payoff = AutocallablePayoff::new(
            vec![1.0],
            vec![2.0],
            vec![0.0],
            false, // memory_coupons
            0.6,
            FinalPayoffType::Participation { rate: 1.0 },
            1.0,
            1.2,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0],
        )
        .expect("test fixture is well-formed");

        let mut state = PathState::new(100, 1.0);
        state.set(state_keys::SPOT, 150.0);
        payoff.on_event(&mut state);

        let value = payoff.value(Currency::USD);
        let expected = 1.2 * 100_000.0;
        assert!(
            (value.amount() - expected).abs() < 1e-6,
            "Participation payoff should cap at 120% of notional: expected {expected}, got {}",
            value.amount()
        );
    }

    /// The knock-in barrier is monitored *discretely* at the observation dates
    /// only (see `types.rs` barrier-monitoring convention). A spot excursion
    /// below the barrier on a non-observation time step must NOT knock the note
    /// in. Before the fix, `on_event` updated `min_spot_observed` on every MC
    /// time step, so an intermediate-step dip wrongly triggered the knock-in.
    #[test]
    fn knock_in_barrier_monitored_only_at_discrete_observation_dates() {
        // Single observation date at t = 1.0; final knock-in barrier 60%.
        let observation_dates = vec![1.0];
        let barriers = vec![2.0]; // unreachable autocall barrier
        let coupons = vec![0.0];

        let mut payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            false,
            0.6, // 60% knock-in barrier => barrier level = 60.0
            FinalPayoffType::KnockInPut { strike: 100.0 },
            1.0,
            1.5,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0],
        )
        .expect("test fixture is well-formed");

        // Intermediate (non-observation) time step at t = 0.5 with spot = 40,
        // which is *below* the 60.0 knock-in barrier. Because t = 0.5 is not an
        // observation date, this excursion must be ignored for knock-in.
        let mut mid = PathState::new(1, 0.5);
        mid.set(state_keys::SPOT, 40.0);
        payoff.on_event(&mut mid);

        // The dip must NOT have been recorded — discrete monitoring only.
        assert_eq!(
            payoff.min_spot_observed,
            f64::INFINITY,
            "an intermediate-step dip must not be recorded for discrete knock-in monitoring"
        );

        // Observation date at t = 1.0 with spot = 95 (well above the barrier).
        let mut obs = PathState::new(1, 1.0);
        obs.set(state_keys::SPOT, 95.0);
        payoff.on_event(&mut obs);

        let value = payoff.value(Currency::USD);
        // Knock-in monitored only at t=1.0 where spot=95 > 60 barrier => NOT
        // knocked in => full principal returned.
        assert!(
            (value.amount() - 100_000.0).abs() < 1e-6,
            "with discrete monitoring the note is not knocked in (obs-date spot 95 \
             > 60 barrier); expected full principal 100_000, got {}",
            value.amount()
        );
    }

    /// Memory ("Phoenix") feature: when the product autocalls at a later
    /// observation date, every coupon from earlier observation dates whose
    /// barrier was not met must be accrued and paid in full on the autocall
    /// date. Before the fix, only the coupon at the autocall date was paid and
    /// the earlier missed coupons were silently lost, underpricing the note.
    #[test]
    fn memory_coupons_accrue_unpaid_earlier_coupons_on_autocall() {
        // Three observation dates. Barriers 0 and 1 are unreachable (200%),
        // barrier 2 is 105%. The path breaches only at observation 2.
        let observation_dates = vec![0.25, 0.5, 0.75];
        let barriers = vec![2.0, 2.0, 1.05];
        let coupons = vec![0.03, 0.04, 0.05];

        let mut payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            true, // memory_coupons ENABLED
            0.6,
            FinalPayoffType::Participation { rate: 1.0 },
            1.0,
            1.5,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0, 1.0, 1.0], // df_ratios all 1.0 to isolate the coupon logic
        )
        .expect("test fixture is well-formed");

        // One coarse step to t = 0.75 breaches only observation index 2.
        let mut state = PathState::new(1, 0.75);
        state.set(state_keys::SPOT, 110.0); // above the 105% barrier at index 2
        payoff.on_event(&mut state);
        assert_eq!(payoff.autocalled_at, Some(2));

        let value = payoff.value(Currency::USD);
        // Memory: accrued coupons = 0.03 + 0.04 + 0.05 = 0.12, plus principal.
        // Payoff = (0.12 + 1.0) * 100_000 = 112_000.
        let expected = (0.03 + 0.04 + 0.05 + 1.0) * 100_000.0;
        assert!(
            (value.amount() - expected).abs() < 1e-6,
            "memory autocallable must pay the cumulative accrued coupons \
             (0.12 + principal); expected {expected}, got {}",
            value.amount()
        );
    }

    /// Regression guard: with `memory_coupons = false` (the default), an
    /// autocall must still pay ONLY the coupon at the autocall date — earlier
    /// missed coupons are not accrued. This pins the non-memory pricing so the
    /// memory feature cannot leak into ordinary autocallables.
    #[test]
    fn non_memory_autocall_pays_only_the_autocall_date_coupon() {
        let observation_dates = vec![0.25, 0.5, 0.75];
        let barriers = vec![2.0, 2.0, 1.05];
        let coupons = vec![0.03, 0.04, 0.05];

        let mut payoff = AutocallablePayoff::new(
            observation_dates,
            barriers,
            coupons,
            false, // memory_coupons DISABLED
            0.6,
            FinalPayoffType::Participation { rate: 1.0 },
            1.0,
            1.5,
            100_000.0,
            Currency::USD,
            100.0,
            vec![1.0, 1.0, 1.0],
        )
        .expect("test fixture is well-formed");

        let mut state = PathState::new(1, 0.75);
        state.set(state_keys::SPOT, 110.0);
        payoff.on_event(&mut state);
        assert_eq!(payoff.autocalled_at, Some(2));

        let value = payoff.value(Currency::USD);
        // Non-memory: only coupon[2] = 0.05 is paid, plus principal.
        let expected = (0.05 + 1.0) * 100_000.0;
        assert!(
            (value.amount() - expected).abs() < 1e-6,
            "non-memory autocallable must pay only the autocall-date coupon \
             (0.05 + principal); expected {expected}, got {}",
            value.amount()
        );
    }
}
