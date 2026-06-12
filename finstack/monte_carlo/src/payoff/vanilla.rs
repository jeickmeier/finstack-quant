//! Vanilla Monte Carlo payoffs.
//!
//! These payoffs are evaluated against a single terminal spot observed at
//! `maturity_step`. The generic engine reports discounted present values, but
//! each payoff in this module returns an undiscounted
//! [`finstack_core::money::Money`] amount and relies on the caller or engine to
//! apply discounting externally.
//!
//! # Conventions
//!
//! - `strike`, `forward_price`, and observed spots use the same price units.
//! - `notional` and `payout` scale the terminal payoff linearly in those same units.
//! - `maturity_step` refers to the path step index, not calendar days. For a
//!   vanilla payoff to trigger on the terminal simulation point, it should
//!   usually equal `TimeGrid::num_steps()`.

use crate::traits::PathState;
use crate::traits::Payoff;
use finstack_core::currency::Currency;
use finstack_core::money::Money;

/// European call option payoff.
///
/// Pays `max(S_T - K, 0) * notional` at `maturity_step`.
///
/// `S_T` is the spot stored in [`PathState`] at the configured maturity step.
#[derive(Debug, Clone)]
pub struct EuropeanCall {
    /// Strike price
    pub strike: f64,
    /// Notional amount
    pub notional: f64,
    /// Maturity step index
    pub maturity_step: usize,
    /// Terminal spot (accumulated during simulation)
    terminal_spot: f64,
}

impl EuropeanCall {
    /// Create a new European call payoff.
    ///
    /// # Arguments
    ///
    /// * `strike` - Strike price in the same units as the simulated spot.
    /// * `notional` - Linear payoff scaling.
    /// * `maturity_step` - Path step index at which the payoff observes `S_T`.
    ///
    /// # Returns
    ///
    /// A payoff that records the spot when `state.step == maturity_step`.
    pub fn new(strike: f64, notional: f64, maturity_step: usize) -> Self {
        Self {
            strike,
            notional,
            maturity_step,
            terminal_spot: 0.0,
        }
    }
}

impl Payoff for EuropeanCall {
    /// Process a path event at maturity.
    ///
    /// Captures the terminal spot price at maturity.
    ///
    /// # Panics
    ///
    /// Panics if the process did not populate a finite `SPOT` state variable
    /// (process/payoff wiring bug) — see [`super::require_finite_state`].
    fn on_event(&mut self, state: &mut PathState) {
        if state.step == self.maturity_step {
            self.terminal_spot = super::require_finite_state(state.spot(), "SPOT", state.step);
        }
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }

    fn value(&self, currency: Currency) -> Money {
        let intrinsic = (self.terminal_spot - self.strike).max(0.0);
        Money::new(intrinsic * self.notional, currency)
    }

    fn reset(&mut self) {
        self.terminal_spot = 0.0;
    }
}

/// European put option payoff.
///
/// Pays `max(K - S_T, 0) * notional` at `maturity_step`.
#[derive(Debug, Clone)]
pub struct EuropeanPut {
    /// Strike price
    pub strike: f64,
    /// Notional amount
    pub notional: f64,
    /// Maturity step index
    pub maturity_step: usize,
    /// Terminal spot
    terminal_spot: f64,
}

impl EuropeanPut {
    /// Create a new European put payoff.
    ///
    /// # Arguments
    ///
    /// * `strike` - Strike price in the same units as the simulated spot.
    /// * `notional` - Linear payoff scaling.
    /// * `maturity_step` - Path step index at which the payoff observes `S_T`.
    pub fn new(strike: f64, notional: f64, maturity_step: usize) -> Self {
        Self {
            strike,
            notional,
            maturity_step,
            terminal_spot: 0.0,
        }
    }
}

impl Payoff for EuropeanPut {
    /// Process a path event at maturity.
    ///
    /// Captures the terminal spot price at maturity.
    ///
    /// # Panics
    ///
    /// Panics if the process did not populate a finite `SPOT` state variable
    /// (process/payoff wiring bug) — see [`super::require_finite_state`]. A
    /// silent 0.0 default would pay the full strike here.
    fn on_event(&mut self, state: &mut PathState) {
        if state.step == self.maturity_step {
            self.terminal_spot = super::require_finite_state(state.spot(), "SPOT", state.step);
        }
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }

    fn value(&self, currency: Currency) -> Money {
        let intrinsic = (self.strike - self.terminal_spot).max(0.0);
        Money::new(intrinsic * self.notional, currency)
    }

    fn reset(&mut self) {
        self.terminal_spot = 0.0;
    }
}

/// Digital (binary) option payoff.
///
/// Pays a fixed `payout` amount if the terminal spot satisfies the strike test
/// at `maturity_step`.
///
/// # Variants
///
/// - Call: pays if `S_T > K`
/// - Put: pays if `S_T < K`
#[derive(Debug, Clone)]
pub struct Digital {
    /// Strike price
    pub strike: f64,
    /// Payout amount (if condition met)
    pub payout: f64,
    /// Maturity step index
    pub maturity_step: usize,
    /// Is this a call (true) or put (false)?
    pub is_call: bool,
    /// Terminal spot
    terminal_spot: f64,
}

impl Digital {
    /// Create a digital call that pays `payout` when `S_T > strike`.
    pub fn call(strike: f64, payout: f64, maturity_step: usize) -> Self {
        Self {
            strike,
            payout,
            maturity_step,
            is_call: true,
            terminal_spot: 0.0,
        }
    }

    /// Create a digital put that pays `payout` when `S_T < strike`.
    pub fn put(strike: f64, payout: f64, maturity_step: usize) -> Self {
        Self {
            strike,
            payout,
            maturity_step,
            is_call: false,
            terminal_spot: 0.0,
        }
    }
}

impl Payoff for Digital {
    /// Process a path event at maturity.
    ///
    /// Captures the terminal spot price at maturity.
    ///
    /// # Panics
    ///
    /// Panics if the process did not populate a finite `SPOT` state variable
    /// (process/payoff wiring bug) — see [`super::require_finite_state`]. A
    /// silent 0.0 default would make digital puts always pay.
    fn on_event(&mut self, state: &mut PathState) {
        if state.step == self.maturity_step {
            self.terminal_spot = super::require_finite_state(state.spot(), "SPOT", state.step);
        }
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }

    fn value(&self, currency: Currency) -> Money {
        let condition_met = if self.is_call {
            self.terminal_spot > self.strike
        } else {
            self.terminal_spot < self.strike
        };

        let payoff = if condition_met { self.payout } else { 0.0 };
        Money::new(payoff, currency)
    }

    fn reset(&mut self) {
        self.terminal_spot = 0.0;
    }
}

/// Forward contract payoff.
///
/// Pays `(S_T - F) * notional` for a long position and `(F - S_T) * notional`
/// for a short position.
///
/// Unlike an option payoff, this amount can be negative.
#[derive(Debug, Clone)]
pub struct Forward {
    /// Forward price
    pub forward_price: f64,
    /// Notional amount
    pub notional: f64,
    /// Maturity step index
    pub maturity_step: usize,
    /// Is this a long (true) or short (false) position?
    pub is_long: bool,
    /// Terminal spot
    terminal_spot: f64,
}

impl Forward {
    /// Create a long forward position.
    pub fn long(forward_price: f64, notional: f64, maturity_step: usize) -> Self {
        Self {
            forward_price,
            notional,
            maturity_step,
            is_long: true,
            terminal_spot: 0.0,
        }
    }

    /// Create a short forward position.
    pub fn short(forward_price: f64, notional: f64, maturity_step: usize) -> Self {
        Self {
            forward_price,
            notional,
            maturity_step,
            is_long: false,
            terminal_spot: 0.0,
        }
    }
}

impl Payoff for Forward {
    /// Process a path event at maturity.
    ///
    /// Captures the terminal spot price at maturity.
    ///
    /// # Panics
    ///
    /// Panics if the process did not populate a finite `SPOT` state variable
    /// (process/payoff wiring bug) — see [`super::require_finite_state`]. A
    /// silent 0.0 default would pay ±F × N.
    fn on_event(&mut self, state: &mut PathState) {
        if state.step == self.maturity_step {
            self.terminal_spot = super::require_finite_state(state.spot(), "SPOT", state.step);
        }
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }

    fn value(&self, currency: Currency) -> Money {
        let diff = self.terminal_spot - self.forward_price;
        let payoff = if self.is_long { diff } else { -diff };
        Money::new(payoff * self.notional, currency)
    }

    fn reset(&mut self) {
        self.terminal_spot = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::state_keys;

    fn create_terminal_state(step: usize, spot: f64) -> PathState {
        let mut state = PathState::new(step, step as f64 * 0.1);
        state.set(state_keys::SPOT, spot);
        state
    }

    #[test]
    fn test_european_call() {
        let mut call = EuropeanCall::new(100.0, 1.0, 10);

        // Simulate path with terminal spot = 110
        let mut state = create_terminal_state(10, 110.0);
        call.on_event(&mut state);

        let value = call.value(Currency::USD);
        assert_eq!(value.amount(), 10.0); // max(110 - 100, 0)
        assert_eq!(value.currency(), Currency::USD);
    }

    #[test]
    fn test_european_call_otm() {
        let mut call = EuropeanCall::new(100.0, 1.0, 10);

        // Out of the money
        let mut state = create_terminal_state(10, 90.0);
        call.on_event(&mut state);

        let value = call.value(Currency::USD);
        assert_eq!(value.amount(), 0.0); // max(90 - 100, 0) = 0
    }

    #[test]
    fn test_european_put() {
        let mut put = EuropeanPut::new(100.0, 1.0, 10);

        // In the money
        let mut state = create_terminal_state(10, 90.0);
        put.on_event(&mut state);

        let value = put.value(Currency::USD);
        assert_eq!(value.amount(), 10.0); // max(100 - 90, 0)
    }

    #[test]
    fn test_digital_call() {
        let mut digital = Digital::call(100.0, 50.0, 10);

        // Above strike
        let mut state = create_terminal_state(10, 110.0);
        digital.on_event(&mut state);

        let value = digital.value(Currency::USD);
        assert_eq!(value.amount(), 50.0);

        // Reset and test below strike
        digital.reset();
        let mut state2 = create_terminal_state(10, 90.0);
        digital.on_event(&mut state2);

        let value2 = digital.value(Currency::USD);
        assert_eq!(value2.amount(), 0.0);
    }

    #[test]
    fn test_digital_put() {
        let mut digital = Digital::put(100.0, 50.0, 10);

        // Below strike
        let mut state = create_terminal_state(10, 90.0);
        digital.on_event(&mut state);

        let value = digital.value(Currency::USD);
        assert_eq!(value.amount(), 50.0);
    }

    #[test]
    fn test_forward_long() {
        let mut forward = Forward::long(100.0, 1.0, 10);

        // Spot above forward price
        let mut state = create_terminal_state(10, 110.0);
        forward.on_event(&mut state);

        let value = forward.value(Currency::USD);
        assert_eq!(value.amount(), 10.0); // 110 - 100
    }

    #[test]
    fn test_forward_short() {
        let mut forward = Forward::short(100.0, 1.0, 10);

        // Spot above forward price (loss for short)
        let mut state = create_terminal_state(10, 110.0);
        forward.on_event(&mut state);

        let value = forward.value(Currency::USD);
        assert_eq!(value.amount(), -10.0); // -(110 - 100)
    }

    #[test]
    fn test_payoff_reset() {
        let mut call = EuropeanCall::new(100.0, 1.0, 10);

        let mut state = create_terminal_state(10, 110.0);
        call.on_event(&mut state);
        assert_eq!(call.terminal_spot, 110.0);

        call.reset();
        assert_eq!(call.terminal_spot, 0.0);
    }

    #[test]
    fn test_notional_scaling() {
        let mut call = EuropeanCall::new(100.0, 10.0, 10);

        let mut state = create_terminal_state(10, 110.0);
        call.on_event(&mut state);

        let value = call.value(Currency::USD);
        assert_eq!(value.amount(), 100.0); // (110 - 100) * 10
    }
}
