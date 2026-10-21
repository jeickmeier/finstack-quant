//! Lookback option payoffs.
//!
//! Lookback options depend on the maximum or minimum spot price
//! observed over the life of the option.
//!
//! Event-date extrema implement discrete monitoring. Calling
//! `with_continuous_monitoring` samples the conditional log-GBM Brownian-bridge
//! extremum inside each interval, using the engine's per-step uniform draw.
//!
//! # Unified Implementation
//!
//! This module provides a unified [`Lookback`] struct that handles both call and put
//! lookback options via the crate [`OptionType`] enum.

use super::barrier::BarrierMonitoring;
use crate::monte_carlo::traits::PathState;
use crate::monte_carlo::traits::Payoff;
use crate::types::OptionType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

/// Conditional log-GBM extremum state shared by fixed and floating payoffs.
#[derive(Debug, Clone)]
struct ContinuousExtremum {
    volatility: f64,
    previous: Option<(f64, f64)>,
}

impl ContinuousExtremum {
    fn new(volatility: f64) -> finstack_quant_core::Result<Self> {
        if !volatility.is_finite() || volatility < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "Continuous lookback volatility must be finite and nonnegative".into(),
            ));
        }
        Ok(Self {
            volatility,
            previous: None,
        })
    }

    fn sample(
        &mut self,
        state: &PathState,
        spot: f64,
        maximum: bool,
    ) -> finstack_quant_core::Result<f64> {
        if spot <= 0.0 || !state.time.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "Continuous lognormal lookback requires positive spot and finite time".into(),
            ));
        }
        let current = spot.ln();
        let previous = self.previous.replace((state.time, current));
        let Some((time, log_spot)) = previous else {
            return Ok(spot);
        };
        let dt = state.time - time;
        if dt < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "Continuous lookback event times must be nondecreasing".into(),
            ));
        }
        let variance = self.volatility * self.volatility * dt;
        let root = if variance == 0.0 {
            (current - log_spot).abs()
        } else {
            let u = state.uniform_random();
            if !u.is_finite() || !(0.0..=1.0).contains(&u) {
                return Err(finstack_quant_core::Error::Validation(
                    "Lookback bridge requires a uniform draw in [0,1]".into(),
                ));
            }
            ((current - log_spot).powi(2) - 2.0 * variance * u.max(f64::MIN_POSITIVE).ln()).sqrt()
        };
        Ok((0.5 * (current + log_spot + if maximum { root } else { -root })).exp())
    }
}

/// Unified fixed-strike lookback option.
///
/// Supports both call and put lookback options through the [`OptionType`] parameter.
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
/// use finstack_quant_models::monte_carlo::payoff::lookback::Lookback;
/// use finstack_quant_models::types::OptionType;
///
/// // Create a lookback call
/// let call = Lookback::new(OptionType::Call, 100.0, 1.0, 10);
///
/// // Create a lookback put
/// let put = Lookback::new(OptionType::Put, 100.0, 1.0, 10);
///
/// // Seasoned: observed max so far is 120
/// let seasoned_call = Lookback::with_initial_extremum(OptionType::Call, 100.0, 1.0, 10, 120.0);
/// ```
#[derive(Debug, Clone)]
pub struct Lookback {
    /// Direction (call or put)
    pub option_type: OptionType,
    /// Strike price
    pub strike: f64,
    /// Notional amount
    pub notional: f64,
    /// Time step index for maturity
    pub maturity_step: usize,

    /// Steps at which the extremum is observed (every step by default).
    monitoring: BarrierMonitoring,
    bridge: Option<ContinuousExtremum>,
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
    pub fn new(option_type: OptionType, strike: f64, notional: f64, maturity_step: usize) -> Self {
        let extreme_spot = match option_type {
            OptionType::Call => f64::NEG_INFINITY,
            OptionType::Put => f64::INFINITY,
        };

        Self {
            option_type,
            strike,
            notional,
            maturity_step,
            monitoring: BarrierMonitoring::Continuous { start_step: 0 },
            bridge: None,
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
    ///
    /// # Arguments
    ///
    /// * `option_type` - Payoff direction selecting call/upside or put/downside behavior.
    /// * `strike` - Option strike in the surface's quote units (absolute or relative)
    /// * `notional` - Trade notional amount in the instrument currency's major units
    /// * `maturity_step` - Zero-based simulation step at which the payoff matures.
    /// * `initial_extremum` - Observed pre-simulation extremum in underlying price units.
    pub fn with_initial_extremum(
        option_type: OptionType,
        strike: f64,
        notional: f64,
        maturity_step: usize,
        initial_extremum: f64,
    ) -> Self {
        Self {
            option_type,
            strike,
            notional,
            maturity_step,
            monitoring: BarrierMonitoring::Continuous { start_step: 0 },
            bridge: None,
            extreme_spot: initial_extremum,
            initial_extreme: initial_extremum,
        }
    }

    /// Restrict extremum tracking to the given monitoring steps.
    ///
    /// # Arguments
    ///
    /// * `monitoring` - `Continuous { start_step }` observes every event step
    ///   from `start_step`; `Discrete { observation_steps }` observes only the
    ///   listed contractual observation steps (sorted and deduplicated on setup).
    #[must_use]
    pub fn with_monitoring(mut self, monitoring: BarrierMonitoring) -> Self {
        self.monitoring = monitoring.normalized();
        self.bridge = None;
        self
    }

    /// Monitor the continuous GBM extremum between engine event dates.
    ///
    /// The bridge is exact for constant volatility and piecewise-constant
    /// log drift on the supplied grid. It samples one independent uniform
    /// per step and changes the payoff's random-stream consumption.
    ///
    /// # Arguments
    ///
    /// * `volatility` - Finite nonnegative annualized lognormal spot volatility
    ///   in decimal units, using the same year clock as the simulated process.
    ///
    /// # Errors
    ///
    /// Returns a validation error for negative or non-finite volatility.
    pub fn with_continuous_monitoring(
        mut self,
        volatility: f64,
    ) -> finstack_quant_core::Result<Self> {
        self.bridge = Some(ContinuousExtremum::new(volatility)?);
        self.monitoring = BarrierMonitoring::Continuous { start_step: 0 };
        Ok(self)
    }
}

impl Payoff for Lookback {
    fn supports_lrm_greeks(&self) -> bool {
        self.bridge.is_none() && self.maturity_step > 0 && !self.monitoring.observes(0)
    }

    fn needs_uniform_random(&self) -> bool {
        self.bridge
            .as_ref()
            .is_some_and(|bridge| bridge.volatility > 0.0)
    }

    /// Update the tracked extremum when the event falls on or before maturity.
    ///
    /// # Arguments
    ///
    /// * `state` - Path state at the current engine event date. Must contain a
    ///   finite `SPOT` when `state.step <= maturity_step`.
    ///
    /// # Errors
    ///
    /// Returns an error if `SPOT` is missing or non-finite at an in-window event.
    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        if state.step <= self.maturity_step && self.monitoring.observes(state.step) {
            let spot = super::require_finite_state(state.spot(), "SPOT", state.step)?;
            let extreme = match &mut self.bridge {
                Some(bridge) => bridge.sample(state, spot, self.option_type == OptionType::Call)?,
                None => spot,
            };
            self.extreme_spot = match self.option_type {
                OptionType::Call => self.extreme_spot.max(extreme),
                OptionType::Put => self.extreme_spot.min(extreme),
            };
        }
        Ok(())
    }

    fn value(&self, currency: Currency) -> finstack_quant_core::Result<Money> {
        let intrinsic = self.option_type.intrinsic(self.extreme_spot, self.strike);
        Money::new(intrinsic * self.notional, currency)
    }

    fn reset(&mut self) {
        self.extreme_spot = self.initial_extreme;
        if let Some(bridge) = &mut self.bridge {
            bridge.previous = None;
        }
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

/// Floating-strike lookback call or put.
///
/// - **Call**: (S_T - S_min) × N, the strike floats to the minimum observed price
/// - **Put**: (S_max - S_T) × N, the strike floats to the maximum observed price
///
/// For seasoned options, use
/// [`with_initial_extremum`](FloatingStrikeLookback::with_initial_extremum)
/// to seed the historical minimum (call) or maximum (put).
#[derive(Debug, Clone)]
pub struct FloatingStrikeLookback {
    /// Direction (call or put)
    pub option_type: OptionType,
    /// Notional amount
    pub notional: f64,
    /// Time step index for maturity
    pub maturity_step: usize,

    /// Steps at which the extremum is observed (every step by default).
    monitoring: BarrierMonitoring,
    bridge: Option<ContinuousExtremum>,
    terminal_spot: f64,
    /// Extreme spot price observed (min for Call, max for Put)
    extreme_spot: f64,
    /// Initial extremum for reset (preserves seasoning across MC paths)
    initial_extreme: f64,
}

impl FloatingStrikeLookback {
    /// Create a new floating-strike lookback option.
    ///
    /// The `extreme_spot` is initialized to:
    /// - `INFINITY` for calls (to track minimum)
    /// - `NEG_INFINITY` for puts (to track maximum)
    pub fn new(option_type: OptionType, notional: f64, maturity_step: usize) -> Self {
        let initial_extremum = match option_type {
            OptionType::Call => f64::INFINITY,
            OptionType::Put => f64::NEG_INFINITY,
        };
        Self::with_initial_extremum(option_type, notional, maturity_step, initial_extremum)
    }

    /// Create a floating-strike lookback option with a known historical extremum.
    ///
    /// Use this for seasoned options where some monitoring has already occurred.
    /// The `initial_extremum` is preserved across MC path resets.
    ///
    /// # Arguments
    ///
    /// * `option_type` - Call (strike floats to the minimum) or put (strike
    ///   floats to the maximum).
    /// * `notional` - Trade notional amount in the instrument currency's major units
    /// * `maturity_step` - Zero-based simulation step at which the payoff matures.
    /// * `initial_extremum` - Observed pre-simulation minimum (call) or
    ///   maximum (put) in underlying price units.
    pub fn with_initial_extremum(
        option_type: OptionType,
        notional: f64,
        maturity_step: usize,
        initial_extremum: f64,
    ) -> Self {
        Self {
            option_type,
            notional,
            maturity_step,
            monitoring: BarrierMonitoring::Continuous { start_step: 0 },
            bridge: None,
            terminal_spot: 0.0,
            extreme_spot: initial_extremum,
            initial_extreme: initial_extremum,
        }
    }

    /// Restrict extremum tracking to the given monitoring steps.
    ///
    /// # Arguments
    ///
    /// * `monitoring` - `Continuous { start_step }` observes every event step
    ///   from `start_step`; `Discrete { observation_steps }` observes only the
    ///   listed contractual observation steps (sorted and deduplicated on setup).
    #[must_use]
    pub fn with_monitoring(mut self, monitoring: BarrierMonitoring) -> Self {
        self.monitoring = monitoring.normalized();
        self.bridge = None;
        self
    }

    /// Monitor the continuous GBM extremum between engine event dates.
    ///
    /// The bridge is exact for constant volatility and piecewise-constant
    /// log drift on the supplied grid. It samples one independent uniform
    /// per step and changes the payoff's random-stream consumption.
    ///
    /// # Arguments
    ///
    /// * `volatility` - Finite nonnegative annualized lognormal spot volatility
    ///   in decimal units, using the same year clock as the simulated process.
    ///
    /// # Errors
    ///
    /// Returns a validation error for negative or non-finite volatility.
    pub fn with_continuous_monitoring(
        mut self,
        volatility: f64,
    ) -> finstack_quant_core::Result<Self> {
        self.bridge = Some(ContinuousExtremum::new(volatility)?);
        self.monitoring = BarrierMonitoring::Continuous { start_step: 0 };
        Ok(self)
    }
}

impl Payoff for FloatingStrikeLookback {
    fn supports_lrm_greeks(&self) -> bool {
        self.bridge.is_none() && self.maturity_step > 0 && !self.monitoring.observes(0)
    }

    fn needs_uniform_random(&self) -> bool {
        self.bridge
            .as_ref()
            .is_some_and(|bridge| bridge.volatility > 0.0)
    }

    /// Update the tracked extremum and capture terminal spot at maturity.
    ///
    /// # Arguments
    ///
    /// * `state` - Path state at the current engine event date. Must contain a
    ///   finite `SPOT` when `state.step <= maturity_step`.
    ///
    /// # Errors
    ///
    /// Returns an error if `SPOT` is missing or non-finite at an in-window event.
    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        if state.step <= self.maturity_step {
            let spot = super::require_finite_state(state.spot(), "SPOT", state.step)?;
            if self.monitoring.observes(state.step) {
                let extreme = match &mut self.bridge {
                    Some(bridge) => {
                        bridge.sample(state, spot, self.option_type == OptionType::Put)?
                    }
                    None => spot,
                };
                self.extreme_spot = match self.option_type {
                    OptionType::Call => self.extreme_spot.min(extreme),
                    OptionType::Put => self.extreme_spot.max(extreme),
                };
            }
            if state.step == self.maturity_step {
                self.terminal_spot = spot;
            }
        }
        Ok(())
    }

    fn value(&self, currency: Currency) -> finstack_quant_core::Result<Money> {
        // Floor at zero for defensive coding: while mathematically
        // S_min <= S_T <= S_max, floating-point edge cases (e.g., pathological
        // reset states) could produce negative values without this guard.
        let payoff = self
            .option_type
            .intrinsic(self.terminal_spot, self.extreme_spot);
        Money::new(payoff * self.notional, currency)
    }

    fn reset(&mut self) {
        self.terminal_spot = 0.0;
        self.extreme_spot = self.initial_extreme;
        if let Some(bridge) = &mut self.bridge {
            bridge.previous = None;
        }
    }

    fn max_event_step(&self) -> Option<usize> {
        Some(self.maturity_step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::traits::state_keys;

    fn create_state(step: usize, spot: f64) -> PathState {
        let mut state = PathState::new(step, step as f64 * 0.1);
        state.set(state_keys::SPOT, spot);
        state
    }

    #[test]
    fn test_lookback_errors_without_spot() {
        let mut lookback = Lookback::new(OptionType::Call, 100.0, 1.0, 10);
        let mut state = PathState::new(0, 0.0);
        let error = lookback
            .on_event(&mut state)
            .expect_err("missing spot must fail");
        assert!(error.to_string().contains("payoff input 'SPOT' missing"));
    }

    #[test]
    fn test_lookback_call_unified() {
        let mut lookback = Lookback::new(OptionType::Call, 100.0, 1.0, 10);

        // Simulate path: max = 120
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 120.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 110.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // max(120 - 100, 0) = 20
        assert_eq!(value.amount(), 20.0);
        assert_eq!(lookback.extreme_spot, 120.0);
    }

    #[test]
    fn test_lookback_put_unified() {
        let mut lookback = Lookback::new(OptionType::Put, 100.0, 1.0, 10);

        // Simulate path: min = 80
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 80.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 90.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // max(100 - 80, 0) = 20
        assert_eq!(value.amount(), 20.0);
        assert_eq!(lookback.extreme_spot, 80.0);
    }

    #[test]
    fn test_lookback_call_out_of_money() {
        let mut lookback = Lookback::new(OptionType::Call, 150.0, 1.0, 10);

        // Path never exceeds strike
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 120.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 110.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // max(120 - 150, 0) = 0
        assert_eq!(value.amount(), 0.0);
    }

    #[test]
    fn test_lookback_put_out_of_money() {
        let mut lookback = Lookback::new(OptionType::Put, 50.0, 1.0, 10);

        // Path never goes below strike
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 80.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 90.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // max(50 - 80, 0) = 0
        assert_eq!(value.amount(), 0.0);
    }

    #[test]
    fn test_lookback_call_reset() {
        let mut lookback = Lookback::new(OptionType::Call, 100.0, 1.0, 10);

        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 120.0))
            .expect("valid payoff event");
        assert_eq!(lookback.extreme_spot, 120.0);

        lookback.reset();
        assert_eq!(lookback.extreme_spot, f64::NEG_INFINITY);
    }

    #[test]
    fn test_lookback_put_reset() {
        let mut lookback = Lookback::new(OptionType::Put, 100.0, 1.0, 10);

        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 80.0))
            .expect("valid payoff event");
        assert_eq!(lookback.extreme_spot, 80.0);

        lookback.reset();
        assert_eq!(lookback.extreme_spot, f64::INFINITY);
    }

    #[test]
    fn test_lookback_with_notional() {
        let mut call = Lookback::new(OptionType::Call, 100.0, 2.5, 10);
        let mut put = Lookback::new(OptionType::Put, 100.0, 2.5, 10);

        // Call path: max = 120
        call.on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        call.on_event(&mut create_state(5, 120.0))
            .expect("valid payoff event");

        // Put path: min = 80
        put.on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        put.on_event(&mut create_state(5, 80.0))
            .expect("valid payoff event");

        // Call: (120 - 100) * 2.5 = 50
        assert_eq!(
            call.value(Currency::USD).expect("valid payoff").amount(),
            50.0
        );

        // Put: (100 - 80) * 2.5 = 50
        assert_eq!(
            put.value(Currency::USD).expect("valid payoff").amount(),
            50.0
        );
    }

    #[test]
    fn test_floating_strike_lookback() {
        let mut lookback = FloatingStrikeLookback::new(OptionType::Call, 1.0, 10);

        // Path: starts 100, min 90, ends 110
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 90.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 110.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // 110 - 90 = 20
        assert_eq!(value.amount(), 20.0);
    }

    #[test]
    fn test_floating_strike_lookback_put() {
        let mut lookback = FloatingStrikeLookback::new(OptionType::Put, 1.0, 10);

        // Path: starts 100, max 120, ends 105
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 120.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 105.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // S_max - S_T = 120 - 105 = 15
        assert_eq!(value.amount(), 15.0);
    }

    #[test]
    fn test_floating_strike_lookback_put_with_notional() {
        let mut lookback = FloatingStrikeLookback::new(OptionType::Put, 2.5, 10);

        // Path: starts 100, max 130, ends 110
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 130.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 110.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // (130 - 110) * 2.5 = 50
        assert_eq!(value.amount(), 50.0);
    }

    #[test]
    fn test_floating_strike_lookback_put_reset() {
        let mut lookback = FloatingStrikeLookback::new(OptionType::Put, 1.0, 10);

        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 120.0))
            .expect("valid payoff event");
        assert_eq!(lookback.extreme_spot, 120.0);

        lookback.reset();
        assert_eq!(lookback.extreme_spot, f64::NEG_INFINITY);
        assert_eq!(lookback.terminal_spot, 0.0);
    }

    #[test]
    fn test_floating_strike_lookback_call_with_initial_min() {
        // Seasoned: historical min = 80, current spot starts at 100
        let mut lookback =
            FloatingStrikeLookback::with_initial_extremum(OptionType::Call, 1.0, 10, 80.0);

        // Path never goes below 90, but historical min was 80
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 90.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 110.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // S_T - S_min = 110 - 80 = 30 (uses historical min)
        assert_eq!(value.amount(), 30.0);

        // Reset preserves historical min
        lookback.reset();
        assert_eq!(lookback.extreme_spot, 80.0);
    }

    #[test]
    fn test_floating_strike_lookback_put_with_initial_max() {
        // Seasoned: historical max = 150, current spot starts at 100
        let mut lookback =
            FloatingStrikeLookback::with_initial_extremum(OptionType::Put, 1.0, 10, 150.0);

        // Path max is 110, but historical max was 150
        lookback
            .on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(5, 110.0))
            .expect("valid payoff event");
        lookback
            .on_event(&mut create_state(10, 95.0))
            .expect("valid payoff event");

        let value = lookback.value(Currency::USD).expect("valid payoff");
        // S_max - S_T = 150 - 95 = 55 (uses historical max)
        assert_eq!(value.amount(), 55.0);

        // Reset preserves historical max
        lookback.reset();
        assert_eq!(lookback.extreme_spot, 150.0);
    }

    #[test]
    fn test_fixed_strike_with_initial_extremum() {
        // Seasoned call: historical max = 130
        let mut call = Lookback::with_initial_extremum(OptionType::Call, 100.0, 1.0, 10, 130.0);
        call.on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        call.on_event(&mut create_state(10, 110.0))
            .expect("valid payoff event");

        // max(130, 110) - 100 = 30
        let value = call.value(Currency::USD).expect("valid payoff");
        assert_eq!(value.amount(), 30.0);

        // Reset preserves initial extremum
        call.reset();
        assert_eq!(call.extreme_spot, 130.0);

        // Seasoned put: historical min = 70
        let mut put = Lookback::with_initial_extremum(OptionType::Put, 100.0, 1.0, 10, 70.0);
        put.on_event(&mut create_state(0, 100.0))
            .expect("valid payoff event");
        put.on_event(&mut create_state(10, 90.0))
            .expect("valid payoff event");

        // 100 - min(70, 90) = 30
        let value = put.value(Currency::USD).expect("valid payoff");
        assert_eq!(value.amount(), 30.0);

        put.reset();
        assert_eq!(put.extreme_spot, 70.0);
    }

    #[test]
    fn discrete_monitoring_ignores_unobserved_steps() {
        // Path 100 -> 150 (step 1, unobserved) -> 110 (step 2, observed).
        let path = [(0, 100.0), (1, 150.0), (2, 110.0)];
        let monitoring = BarrierMonitoring::Discrete {
            observation_steps: vec![2],
        };

        let mut fixed =
            Lookback::new(OptionType::Call, 100.0, 1.0, 2).with_monitoring(monitoring.clone());
        let mut floating_put = FloatingStrikeLookback::new(OptionType::Put, 1.0, 2)
            .with_monitoring(monitoring.clone());
        let mut floating_call =
            FloatingStrikeLookback::new(OptionType::Call, 1.0, 2).with_monitoring(monitoring);
        for (step, spot) in path {
            let mut state = create_state(step, spot);
            fixed.on_event(&mut state).expect("valid payoff event");
            floating_put
                .on_event(&mut state)
                .expect("valid payoff event");
            floating_call
                .on_event(&mut state)
                .expect("valid payoff event");
        }

        // Only step 2 is observed: max = min = 110 and S_T = 110.
        assert_eq!(
            fixed.value(Currency::USD).expect("valid payoff").amount(),
            10.0
        );
        assert_eq!(
            floating_put
                .value(Currency::USD)
                .expect("valid payoff")
                .amount(),
            0.0
        );
        assert_eq!(
            floating_call
                .value(Currency::USD)
                .expect("valid payoff")
                .amount(),
            0.0
        );
    }

    /// Fixed- and floating-strike payoffs on fixed paths, fresh and seasoned,
    /// with continuous and discrete monitoring, across two resets.
    #[test]
    fn lookback_payoffs_on_fixed_paths_are_bit_stable() {
        let path = [100.0, 112.5, 87.25, 131.0, 96.5, 104.75];
        let discrete = BarrierMonitoring::Discrete {
            observation_steps: vec![1, 2, 5],
        };
        type TrackedPayoff = Box<dyn FnMut(&mut PathState) -> f64>;
        let mut payoffs: Vec<TrackedPayoff> = Vec::new();
        macro_rules! track {
            ($payoff:expr) => {{
                let mut payoff = $payoff;
                payoffs.push(Box::new(move |state: &mut PathState| {
                    if state.step == 0 {
                        payoff.reset();
                    }
                    payoff.on_event(state).expect("finite spot");
                    payoff.value(Currency::USD).expect("payoff").amount()
                }));
            }};
        }
        track!(Lookback::new(OptionType::Call, 100.0, 2.0, 5));
        track!(Lookback::new(OptionType::Put, 100.0, 2.0, 5));
        track!(Lookback::with_initial_extremum(
            OptionType::Call,
            100.0,
            2.0,
            5,
            140.0
        ));
        track!(Lookback::with_initial_extremum(
            OptionType::Put,
            100.0,
            2.0,
            5,
            70.0
        ));
        track!(Lookback::new(OptionType::Call, 100.0, 2.0, 5).with_monitoring(discrete.clone()));
        track!(FloatingStrikeLookback::new(OptionType::Call, 2.0, 5));
        track!(FloatingStrikeLookback::new(OptionType::Put, 2.0, 5));
        track!(FloatingStrikeLookback::with_initial_extremum(
            OptionType::Call,
            2.0,
            5,
            80.0
        ));
        track!(FloatingStrikeLookback::with_initial_extremum(
            OptionType::Put,
            2.0,
            5,
            150.0
        ));
        track!(
            FloatingStrikeLookback::new(OptionType::Call, 2.0, 5).with_monitoring(discrete.clone())
        );
        track!(FloatingStrikeLookback::new(OptionType::Put, 2.0, 5).with_monitoring(discrete));

        let expected = [
            62.0, 25.5, 80.0, 60.0, 25.0, 35.0, 52.5, 49.5, 90.5, 35.0, 15.5,
        ];
        for _ in 0..2 {
            let mut finals = vec![0.0; payoffs.len()];
            for (step, &spot) in path.iter().enumerate() {
                let mut state = create_state(step, spot);
                for (value, payoff) in finals.iter_mut().zip(payoffs.iter_mut()) {
                    *value = payoff(&mut state);
                }
            }
            assert_eq!(finals, expected);
        }
    }
    #[test]
    fn continuous_bridge_resets_and_samples_both_sides() {
        let mut call = Lookback::new(OptionType::Call, 100.0, 1.0, 1)
            .with_continuous_monitoring(0.2)
            .unwrap();
        let mut put = Lookback::new(OptionType::Put, 100.0, 1.0, 1)
            .with_continuous_monitoring(0.2)
            .unwrap();
        for _ in 0..2 {
            let mut start = PathState::new(0, 0.0);
            start.set(state_keys::SPOT, 100.0);
            let mut end = PathState::new(1, 1.0);
            end.set(state_keys::SPOT, 100.0);
            end.set_uniform_random(0.5);
            call.on_event(&mut start).unwrap();
            call.on_event(&mut end).unwrap();
            put.on_event(&mut start).unwrap();
            put.on_event(&mut end).unwrap();
            let log_extreme = (-0.5_f64 * 0.04 * 0.5_f64.ln()).sqrt();
            assert!(
                (call.value(Currency::USD).unwrap().amount() - 100.0 * log_extreme.exp_m1()).abs()
                    < 1e-10
            );
            assert!(
                (put.value(Currency::USD).unwrap().amount() - 100.0 * -(-log_extreme).exp_m1())
                    .abs()
                    < 1e-10
            );
            call.reset();
            put.reset();
        }
    }

    #[test]
    fn continuous_lookback_grid_and_seed_convergence_meets_one_bp_budget() {
        use crate::monte_carlo::pricer::path_dependent::{
            PathDependentPricer, PathDependentPricerConfig,
        };
        use crate::monte_carlo::process::gbm::{GbmParams, GbmProcess};
        use finstack_quant_core::math::time_grid::TimeGrid;
        // For zero log drift, the reflection principle gives
        // max(log(S/S0)) ~ |N(0, sigma² T)|. Integrating its density yields
        // E[max(S)] = 2 S0 exp(sigma² T/2) Phi(sigma sqrt(T)).
        // This reference uses neither the lookback kernel nor bridge sampling.
        let sigma = 0.03_f64;
        let rate = 0.5 * sigma * sigma;
        let df = (-rate).exp();
        let expected_max = 200.0 * rate.exp() * finstack_quant_core::math::norm_cdf(sigma);
        let fixed_reference = df * (expected_max - 100.0);
        let floating_reference = df * expected_max - 100.0;
        let process = GbmProcess::new(GbmParams::new(rate, 0.0, sigma).unwrap());
        let budget = 100.0 * 1e-4;
        let mut fixed_values = Vec::new();
        let mut floating_values = Vec::new();
        for steps in [1, 8, 32] {
            for seed in [17, 29] {
                let pricer = PathDependentPricer::new(PathDependentPricerConfig {
                    num_paths: 1_000_000,
                    seed,
                    use_parallel: true,
                    antithetic: true,
                    ..Default::default()
                });
                let fixed = Lookback::new(OptionType::Call, 100.0, 1.0, steps)
                    .with_continuous_monitoring(sigma)
                    .unwrap();
                let floating = FloatingStrikeLookback::new(OptionType::Put, 1.0, steps)
                    .with_continuous_monitoring(sigma)
                    .unwrap();
                let fixed = pricer
                    .price_with_grid(
                        &process,
                        100.0,
                        TimeGrid::uniform(1.0, steps).unwrap(),
                        &fixed,
                        Currency::USD,
                        df,
                    )
                    .unwrap();
                let floating = pricer
                    .price_with_grid(
                        &process,
                        100.0,
                        TimeGrid::uniform(1.0, steps).unwrap(),
                        &floating,
                        Currency::USD,
                        df,
                    )
                    .unwrap();
                for (result, reference) in
                    [(&fixed, fixed_reference), (&floating, floating_reference)]
                {
                    let half_width = 2.5758293035489004 * result.stderr;
                    assert!(
                        half_width < budget,
                        "steps={steps}, seed={seed}, 99% half-width={half_width}"
                    );
                    assert!(
                        (result.mean.amount() - reference).abs() < budget,
                        "steps={steps}, seed={seed}, value={}, reference={reference}, stderr={}",
                        result.mean.amount(),
                        result.stderr
                    );
                }
                fixed_values.push(fixed.mean.amount());
                floating_values.push(floating.mean.amount());
            }
        }
        for values in [fixed_values, floating_values] {
            let min = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                max - min < budget,
                "grid/seed refinement spread {} exceeds {budget}",
                max - min
            );
        }
    }
}
