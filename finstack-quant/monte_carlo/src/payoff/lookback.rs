//! Lookback option payoffs.
//!
//! Lookback options depend on the maximum or minimum spot price
//! observed over the life of the option.
//!
//! # Unified Implementation
//!
//! This module provides a unified [`Lookback`] struct that handles both call and put
//! lookback options via the [`LookbackDirection`] enum. This eliminates code duplication
//! while maintaining the same functionality.
//!
//! The unified [`Lookback`] struct replaces legacy call/put-specific types.

use crate::traits::PathState;
use crate::traits::Payoff;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

/// Direction of a fixed-strike lookback option.
///
/// Determines whether the option tracks the maximum (call) or minimum (put) spot price.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LookbackDirection {
    /// Call option: payoff = max(S_max - K, 0)
    Call,
    /// Put option: payoff = max(K - S_min, 0)
    Put,
}

/// Unified fixed-strike lookback option.
///
/// Supports both call and put lookback options through the [`LookbackDirection`] parameter.
///
/// # Payoffs
///
/// - **Call**: max(S_max - K, 0) × N, where S_max is the maximum spot observed
/// - **Put**: max(K - S_min, 0) × N, where S_min is the minimum spot observed
///
/// # Seasoning
///
/// For seasoned options where some monitoring has already occurred, use
/// [`with_initial_extremum`](Lookback::with_initial_extremum) to seed the
/// historical extremum. This ensures each MC path starts from the known
/// historical max/min rather than the default (±infinity).
///
/// # Examples
///
/// ```text
/// use finstack_quant_monte_carlo::payoff::lookback::{Lookback, LookbackDirection};
///
/// // Create a lookback call
/// let call = Lookback::new(LookbackDirection::Call, 100.0, 1.0, 10);
///
/// // Create a lookback put
/// let put = Lookback::new(LookbackDirection::Put, 100.0, 1.0, 10);
///
/// // Seasoned: observed max so far is 120
/// let seasoned_call = Lookback::with_initial_extremum(LookbackDirection::Call, 100.0, 1.0, 10, 120.0);
/// ```
#[derive(Debug, Clone)]
pub struct Lookback {
    /// Direction (call or put)
    pub direction: LookbackDirection,
    /// Strike price
    pub strike: f64,
    /// Notional amount
    pub notional: f64,
    /// Time step index for maturity
    pub maturity_step: usize,

    /// Extreme spot price observed (max for Call, min for Put)
    extreme_spot: f64,
    /// Initial extremum for reset (preserves seasoning across MC paths)
    initial_extreme: f64,
}

impl Lookback {
    /// Create a new fixed-strike lookback option.
    ///
    /// The `extreme_spot` is initialized to:
    /// - `NEG_INFINITY` for calls (to track maximum)
    /// - `INFINITY` for puts (to track minimum)
    pub fn new(
        direction: LookbackDirection,
        strike: f64,
        notional: f64,
        maturity_step: usize,
    ) -> Self {
        let extreme_spot = match direction {
            LookbackDirection::Call => f64::NEG_INFINITY,
            LookbackDirection::Put => f64::INFINITY,
        };

        Self {
            direction,
            strike,
            notional,
            maturity_step,
            extreme_spot,
            initial_extreme: extreme_spot,
        }
    }

    /// Create a new fixed-strike lookback option with a known historical extremum.
    ///
    /// Use this for seasoned options where some monitoring has already occurred.
    /// The `initial_extremum` seeds the tracking and is preserved across MC path resets.
    ///
    /// - For **Call**: pass the observed maximum so far (e.g., `max(observed_max, spot)`)
    /// - For **Put**: pass the observed minimum so far (e.g., `min(observed_min, spot)`)
    pub fn with_initial_extremum(
        direction: LookbackDirection,
        strike: f64,
        notional: f64,
        maturity_step: usize,
        initial_extremum: f64,
    ) -> Self {
        Self {
            direction,
            strike,
            notional,
            maturity_step,
            extreme_spot: initial_extremum,
            initial_extreme: initial_extremum,
        }
    }
}

impl Payoff for Lookback {
    fn on_event(&mut self, state: &mut PathState) {
        if state.step <= self.maturity_step {
            if let Some(spot) = state.spot() {
                self.extreme_spot = match self.direction {
                    LookbackDirection::Call => self.extreme_spot.max(spot),
                    LookbackDirection::Put => self.extreme_spot.min(spot),
                };
            }
        }
    }

    fn value(&self, currency: Currency) -> Money {
        let intrinsic = match self.direction {
            LookbackDirection::Call => (self.extreme_spot - self.strike).max(0.0),
            LookbackDirection::Put => (self.strike - self.extreme_spot).max(0.0),
        };
        Money::new(intrinsic * self.notional, currency)
    }

    fn reset(&mut self) {
        self.extreme_spot = self.initial_extreme;
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

/// Floating strike lookback call.
///
/// Payoff: (S_T - S_min) × N
///
/// The strike "floats" to the minimum observed price.
///
/// For seasoned options, use [`with_initial_min`](FloatingStrikeLookbackCall::with_initial_min)
/// to seed the historical minimum.
#[derive(Debug, Clone)]
pub struct FloatingStrikeLookbackCall {
    /// Notional amount
    pub notional: f64,
    /// Time step index for maturity
    pub maturity_step: usize,

    terminal_spot: f64,
    min_spot: f64,
    /// Initial minimum for reset (preserves seasoning across MC paths)
    initial_min: f64,
}

impl FloatingStrikeLookbackCall {
    /// Create a new floating strike lookback call.
    pub fn new(notional: f64, maturity_step: usize) -> Self {
        Self {
            notional,
            maturity_step,
            terminal_spot: 0.0,
            min_spot: f64::INFINITY,
            initial_min: f64::INFINITY,
        }
    }

    /// Create a floating strike lookback call with a known historical minimum.
    ///
    /// Use this for seasoned options where the observed minimum is below the current spot.
    /// The `initial_min` is preserved across MC path resets.
    pub fn with_initial_min(notional: f64, maturity_step: usize, initial_min: f64) -> Self {
        Self {
            notional,
            maturity_step,
            terminal_spot: 0.0,
            min_spot: initial_min,
            initial_min,
        }
    }
}

impl Payoff for FloatingStrikeLookbackCall {
    fn on_event(&mut self, state: &mut PathState) {
        if state.step <= self.maturity_step {
            if let Some(spot) = state.spot() {
                self.min_spot = self.min_spot.min(spot);
                if state.step == self.maturity_step {
                    self.terminal_spot = spot;
                }
            }
        }
    }

    fn value(&self, currency: Currency) -> Money {
        // Floor at zero for defensive coding: while mathematically S_T >= S_min,
        // floating-point edge cases (e.g., pathological reset states) could produce
        // negative values without this guard.
        let payoff = (self.terminal_spot - self.min_spot).max(0.0);
        Money::new(payoff * self.notional, currency)
    }

    fn reset(&mut self) {
        self.terminal_spot = 0.0;
        self.min_spot = self.initial_min;
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

/// Floating strike lookback put.
///
/// Payoff: (S_max - S_T) × N
///
/// The strike "floats" to the maximum observed price.
///
/// For seasoned options, use [`with_initial_max`](FloatingStrikeLookbackPut::with_initial_max)
/// to seed the historical maximum.
#[derive(Debug, Clone)]
pub struct FloatingStrikeLookbackPut {
    /// Notional amount
    pub notional: f64,
    /// Time step index for maturity
    pub maturity_step: usize,

    terminal_spot: f64,
    max_spot: f64,
    /// Initial maximum for reset (preserves seasoning across MC paths)
    initial_max: f64,
}

impl FloatingStrikeLookbackPut {
    /// Create a new floating strike lookback put.
    pub fn new(notional: f64, maturity_step: usize) -> Self {
        Self {
            notional,
            maturity_step,
            terminal_spot: 0.0,
            max_spot: f64::NEG_INFINITY,
            initial_max: f64::NEG_INFINITY,
        }
    }

    /// Create a floating strike lookback put with a known historical maximum.
    ///
    /// Use this for seasoned options where the observed maximum is above the current spot.
    /// The `initial_max` is preserved across MC path resets.
    pub fn with_initial_max(notional: f64, maturity_step: usize, initial_max: f64) -> Self {
        Self {
            notional,
            maturity_step,
            terminal_spot: 0.0,
            max_spot: initial_max,
            initial_max,
        }
    }
}

impl Payoff for FloatingStrikeLookbackPut {
    fn on_event(&mut self, state: &mut PathState) {
        if state.step <= self.maturity_step {
            if let Some(spot) = state.spot() {
                self.max_spot = self.max_spot.max(spot);
                if state.step == self.maturity_step {
                    self.terminal_spot = spot;
                }
            }
        }
    }

    fn value(&self, currency: Currency) -> Money {
        // Floor at zero for defensive coding: while mathematically S_max >= S_T,
        // floating-point edge cases could produce negative values without this guard.
        let payoff = (self.max_spot - self.terminal_spot).max(0.0);
        Money::new(payoff * self.notional, currency)
    }

    fn reset(&mut self) {
        self.terminal_spot = 0.0;
        self.max_spot = self.initial_max;
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::state_keys;

    fn create_state(step: usize, spot: f64) -> PathState {
        let mut state = PathState::new(step, step as f64 * 0.1);
        state.set(state_keys::SPOT, spot);
        state
    }

    #[test]
    fn test_lookback_call_unified() {
        let mut lookback = Lookback::new(LookbackDirection::Call, 100.0, 1.0, 10);

        // Simulate path: max = 120
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 120.0));
        lookback.on_event(&mut create_state(10, 110.0));

        let value = lookback.value(Currency::USD);
        // max(120 - 100, 0) = 20
        assert_eq!(value.amount(), 20.0);
        assert_eq!(lookback.extreme_spot, 120.0);
    }

    #[test]
    fn test_lookback_put_unified() {
        let mut lookback = Lookback::new(LookbackDirection::Put, 100.0, 1.0, 10);

        // Simulate path: min = 80
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 80.0));
        lookback.on_event(&mut create_state(10, 90.0));

        let value = lookback.value(Currency::USD);
        // max(100 - 80, 0) = 20
        assert_eq!(value.amount(), 20.0);
        assert_eq!(lookback.extreme_spot, 80.0);
    }

    #[test]
    fn test_lookback_call_out_of_money() {
        let mut lookback = Lookback::new(LookbackDirection::Call, 150.0, 1.0, 10);

        // Path never exceeds strike
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 120.0));
        lookback.on_event(&mut create_state(10, 110.0));

        let value = lookback.value(Currency::USD);
        // max(120 - 150, 0) = 0
        assert_eq!(value.amount(), 0.0);
    }

    #[test]
    fn test_lookback_put_out_of_money() {
        let mut lookback = Lookback::new(LookbackDirection::Put, 50.0, 1.0, 10);

        // Path never goes below strike
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 80.0));
        lookback.on_event(&mut create_state(10, 90.0));

        let value = lookback.value(Currency::USD);
        // max(50 - 80, 0) = 0
        assert_eq!(value.amount(), 0.0);
    }

    #[test]
    fn test_lookback_call_reset() {
        let mut lookback = Lookback::new(LookbackDirection::Call, 100.0, 1.0, 10);

        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 120.0));
        assert_eq!(lookback.extreme_spot, 120.0);

        lookback.reset();
        assert_eq!(lookback.extreme_spot, f64::NEG_INFINITY);
    }

    #[test]
    fn test_lookback_put_reset() {
        let mut lookback = Lookback::new(LookbackDirection::Put, 100.0, 1.0, 10);

        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 80.0));
        assert_eq!(lookback.extreme_spot, 80.0);

        lookback.reset();
        assert_eq!(lookback.extreme_spot, f64::INFINITY);
    }

    #[test]
    fn test_lookback_with_notional() {
        let mut call = Lookback::new(LookbackDirection::Call, 100.0, 2.5, 10);
        let mut put = Lookback::new(LookbackDirection::Put, 100.0, 2.5, 10);

        // Call path: max = 120
        call.on_event(&mut create_state(0, 100.0));
        call.on_event(&mut create_state(5, 120.0));

        // Put path: min = 80
        put.on_event(&mut create_state(0, 100.0));
        put.on_event(&mut create_state(5, 80.0));

        // Call: (120 - 100) * 2.5 = 50
        assert_eq!(call.value(Currency::USD).amount(), 50.0);

        // Put: (100 - 80) * 2.5 = 50
        assert_eq!(put.value(Currency::USD).amount(), 50.0);
    }

    #[test]
    fn test_floating_strike_lookback() {
        let mut lookback = FloatingStrikeLookbackCall::new(1.0, 10);

        // Path: starts 100, min 90, ends 110
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 90.0));
        lookback.on_event(&mut create_state(10, 110.0));

        let value = lookback.value(Currency::USD);
        // 110 - 90 = 20
        assert_eq!(value.amount(), 20.0);
    }

    #[test]
    fn test_floating_strike_lookback_put() {
        let mut lookback = FloatingStrikeLookbackPut::new(1.0, 10);

        // Path: starts 100, max 120, ends 105
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 120.0));
        lookback.on_event(&mut create_state(10, 105.0));

        let value = lookback.value(Currency::USD);
        // S_max - S_T = 120 - 105 = 15
        assert_eq!(value.amount(), 15.0);
    }

    #[test]
    fn test_floating_strike_lookback_put_with_notional() {
        let mut lookback = FloatingStrikeLookbackPut::new(2.5, 10);

        // Path: starts 100, max 130, ends 110
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 130.0));
        lookback.on_event(&mut create_state(10, 110.0));

        let value = lookback.value(Currency::USD);
        // (130 - 110) * 2.5 = 50
        assert_eq!(value.amount(), 50.0);
    }

    #[test]
    fn test_floating_strike_lookback_put_reset() {
        let mut lookback = FloatingStrikeLookbackPut::new(1.0, 10);

        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 120.0));
        assert_eq!(lookback.max_spot, 120.0);

        lookback.reset();
        assert_eq!(lookback.max_spot, f64::NEG_INFINITY);
        assert_eq!(lookback.terminal_spot, 0.0);
    }

    #[test]
    fn test_floating_strike_lookback_call_with_initial_min() {
        // Seasoned: historical min = 80, current spot starts at 100
        let mut lookback = FloatingStrikeLookbackCall::with_initial_min(1.0, 10, 80.0);

        // Path never goes below 90, but historical min was 80
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 90.0));
        lookback.on_event(&mut create_state(10, 110.0));

        let value = lookback.value(Currency::USD);
        // S_T - S_min = 110 - 80 = 30 (uses historical min)
        assert_eq!(value.amount(), 30.0);

        // Reset preserves historical min
        lookback.reset();
        assert_eq!(lookback.min_spot, 80.0);
    }

    #[test]
    fn test_floating_strike_lookback_put_with_initial_max() {
        // Seasoned: historical max = 150, current spot starts at 100
        let mut lookback = FloatingStrikeLookbackPut::with_initial_max(1.0, 10, 150.0);

        // Path max is 110, but historical max was 150
        lookback.on_event(&mut create_state(0, 100.0));
        lookback.on_event(&mut create_state(5, 110.0));
        lookback.on_event(&mut create_state(10, 95.0));

        let value = lookback.value(Currency::USD);
        // S_max - S_T = 150 - 95 = 55 (uses historical max)
        assert_eq!(value.amount(), 55.0);

        // Reset preserves historical max
        lookback.reset();
        assert_eq!(lookback.max_spot, 150.0);
    }

    #[test]
    fn test_fixed_strike_with_initial_extremum() {
        // Seasoned call: historical max = 130
        let mut call =
            Lookback::with_initial_extremum(LookbackDirection::Call, 100.0, 1.0, 10, 130.0);
        call.on_event(&mut create_state(0, 100.0));
        call.on_event(&mut create_state(10, 110.0));

        // max(130, 110) - 100 = 30
        let value = call.value(Currency::USD);
        assert_eq!(value.amount(), 30.0);

        // Reset preserves initial extremum
        call.reset();
        assert_eq!(call.extreme_spot, 130.0);

        // Seasoned put: historical min = 70
        let mut put = Lookback::with_initial_extremum(LookbackDirection::Put, 100.0, 1.0, 10, 70.0);
        put.on_event(&mut create_state(0, 100.0));
        put.on_event(&mut create_state(10, 90.0));

        // 100 - min(70, 90) = 30
        let value = put.value(Currency::USD);
        assert_eq!(value.amount(), 30.0);

        put.reset();
        assert_eq!(put.extreme_spot, 70.0);
    }
}
