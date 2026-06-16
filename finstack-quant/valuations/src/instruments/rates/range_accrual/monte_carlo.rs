//! Range accrual payoff for Monte Carlo pricing.
//!
//! Range accrual products pay coupons based on the number of days the underlying
//! stays within a specified range.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_monte_carlo::traits::PathState;
use finstack_quant_monte_carlo::traits::Payoff;

/// Range accrual payoff.
///
/// Accumulates coupon payments based on how many observation dates the
/// underlying spot price stays within a specified range [lower_bound, upper_bound].
///
/// # Payoff Structure
///
/// Payoff = coupon_rate * (days_in_range / total_days) * notional
///
/// where days_in_range is the count of observation dates where lower_bound <= S_t <= upper_bound.
///
/// # Historical Fixings
///
/// For mid-life valuations, use `new_with_history` to include past observations
/// that were known to be in or out of range.
#[derive(Debug, Clone)]
pub struct RangeAccrualPayoff {
    /// Observation dates (time in years, must be sorted, future only)
    pub observation_dates: Vec<f64>,
    /// Lower bound of the range (effective/absolute)
    pub lower_bound: f64,
    /// Upper bound of the range (effective/absolute)
    pub upper_bound: f64,
    /// Coupon rate (e.g., 0.08 for 8% annual)
    pub coupon_rate: f64,
    /// Notional amount
    pub notional: f64,
    /// Currency
    pub currency: Currency,

    // Historical fixing info (for mid-life valuations)
    /// Number of past observations that were in range
    past_in_range: usize,
    /// Total number of past observations
    total_past_observations: usize,

    // State variables (tracked during path simulation)
    /// Number of future observation dates where spot was in range
    days_in_range: usize,
    /// Total number of future observation dates checked
    total_observations: usize,
    /// Index of next observation date to check
    next_obs_idx: usize,
}

impl RangeAccrualPayoff {
    /// Create a new range accrual payoff.
    ///
    /// # Arguments
    ///
    /// * `observation_dates` - Future dates when range check occurs (must be sorted)
    /// * `lower_bound` - Lower bound of the range (effective/absolute)
    /// * `upper_bound` - Upper bound of the range (must be > lower_bound)
    /// * `coupon_rate` - Annual coupon rate
    /// * `notional` - Notional amount
    /// * `currency` - Currency
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if `lower_bound >= upper_bound`,
    /// `coupon_rate` is negative, or `observation_dates` are not strictly sorted.
    pub fn new(
        observation_dates: Vec<f64>,
        lower_bound: f64,
        upper_bound: f64,
        coupon_rate: f64,
        notional: f64,
        currency: Currency,
    ) -> finstack_quant_core::Result<Self> {
        Self::new_with_history(
            observation_dates,
            lower_bound,
            upper_bound,
            coupon_rate,
            notional,
            currency,
            0,
            0,
        )
    }

    /// Create a new range accrual payoff with historical fixing data.
    ///
    /// Use this for mid-life valuations where some observations have already occurred.
    ///
    /// # Arguments
    ///
    /// * `observation_dates` - Future dates when range check occurs (must be sorted)
    /// * `lower_bound` - Lower bound of the range (effective/absolute)
    /// * `upper_bound` - Upper bound of the range (must be > lower_bound)
    /// * `coupon_rate` - Annual coupon rate
    /// * `notional` - Notional amount
    /// * `currency` - Currency
    /// * `past_in_range` - Number of past observations that were in range
    /// * `total_past_observations` - Total number of past observations
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if `lower_bound >= upper_bound`,
    /// `coupon_rate` is negative, `past_in_range > total_past_observations`, or
    /// `observation_dates` are not strictly sorted in ascending order.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_history(
        observation_dates: Vec<f64>,
        lower_bound: f64,
        upper_bound: f64,
        coupon_rate: f64,
        notional: f64,
        currency: Currency,
        past_in_range: usize,
        total_past_observations: usize,
    ) -> finstack_quant_core::Result<Self> {
        use std::cmp::Ordering;

        if lower_bound.partial_cmp(&upper_bound) != Some(Ordering::Less) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RangeAccrualPayoff: lower_bound ({lower_bound}) must be strictly less than upper_bound ({upper_bound})"
            )));
        }
        if coupon_rate.partial_cmp(&0.0) == Some(Ordering::Less) || coupon_rate.is_nan() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RangeAccrualPayoff: coupon_rate ({coupon_rate}) must be non-negative"
            )));
        }
        if past_in_range > total_past_observations {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RangeAccrualPayoff: past_in_range ({past_in_range}) cannot exceed total_past_observations ({total_past_observations})"
            )));
        }

        // Verify observation dates are strictly sorted ascending.
        for i in 1..observation_dates.len() {
            if observation_dates[i - 1].partial_cmp(&observation_dates[i]) != Some(Ordering::Less) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "RangeAccrualPayoff: observation_dates must be strictly sorted ascending; \
                     found {} >= {} at index {}",
                    observation_dates[i - 1],
                    observation_dates[i],
                    i
                )));
            }
        }

        Ok(Self {
            observation_dates,
            lower_bound,
            upper_bound,
            coupon_rate,
            notional,
            currency,
            past_in_range,
            total_past_observations,
            days_in_range: 0,
            total_observations: 0,
            next_obs_idx: 0,
        })
    }

    /// Check if spot is within range.
    fn is_in_range(&self, spot: f64) -> bool {
        spot >= self.lower_bound && spot <= self.upper_bound
    }

    /// Tolerance for matching observation dates (in years).
    const TIME_TOLERANCE: f64 = 1e-6;
}

impl Payoff for RangeAccrualPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        // Use a while loop to handle multiple observations per time step
        // This can happen when the simulation grid doesn't align exactly with observation dates
        while self.next_obs_idx < self.observation_dates.len() {
            let target_date = self.observation_dates[self.next_obs_idx];

            // Check if we've reached or passed this observation date
            if state.time >= target_date - Self::TIME_TOLERANCE {
                if let Some(spot) = state.spot() {
                    if self.is_in_range(spot) {
                        self.days_in_range += 1;
                    }
                    self.total_observations += 1;
                    self.next_obs_idx += 1;
                } else {
                    // No spot available, skip this observation
                    break;
                }
            } else {
                // Haven't reached next observation date yet
                break;
            }
        }
    }

    fn value(&self, currency: Currency) -> Money {
        // Include both historical and simulated observations
        let total_in_range = self.past_in_range + self.days_in_range;
        let total_obs = self.total_past_observations + self.total_observations;

        if total_obs == 0 {
            return Money::new(0.0, currency);
        }

        // Compute accrual fraction: total_in_range / total_observations
        let accrual_fraction = total_in_range as f64 / total_obs as f64;

        // Payoff = coupon_rate * accrual_fraction * notional
        let payoff = self.coupon_rate * accrual_fraction * self.notional;

        Money::new(payoff, currency)
    }

    fn reset(&mut self) {
        // Reset only the simulation state, not the historical data
        self.days_in_range = 0;
        self.total_observations = 0;
        self.next_obs_idx = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_monte_carlo::traits::state_keys;

    #[test]
    fn test_range_accrual_creation() {
        let observation_dates = vec![0.25, 0.5, 0.75, 1.0];
        let accrual = RangeAccrualPayoff::new(
            observation_dates,
            95.0,  // Lower bound
            105.0, // Upper bound
            0.08,  // 8% coupon
            100_000.0,
            Currency::USD,
        )
        .expect("valid range accrual inputs");

        assert_eq!(accrual.observation_dates.len(), 4);
        assert_eq!(accrual.lower_bound, 95.0);
        assert_eq!(accrual.upper_bound, 105.0);
    }

    #[test]
    fn test_range_accrual_in_range() {
        let observation_dates = vec![0.25, 0.5];
        let mut accrual = RangeAccrualPayoff::new(
            observation_dates,
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
        )
        .expect("valid range accrual inputs");

        // Both observations in range
        let mut state1 = PathState::new(10, 0.25);
        state1.set(state_keys::SPOT, 100.0);
        accrual.on_event(&mut state1);

        let mut state2 = PathState::new(20, 0.5);
        state2.set(state_keys::SPOT, 98.0);
        accrual.on_event(&mut state2);

        let value = accrual.value(Currency::USD);
        // 2 days in range / 2 total = 1.0 fraction
        // Payoff = 0.08 * 1.0 * 100_000 = 8_000
        assert!((value.amount() - 8_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_range_accrual_partial() {
        let observation_dates = vec![0.25, 0.5, 0.75];
        let mut accrual = RangeAccrualPayoff::new(
            observation_dates,
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
        )
        .expect("valid range accrual inputs");

        // Only 2 out of 3 in range
        let mut state1 = PathState::new(10, 0.25);
        state1.set(state_keys::SPOT, 100.0); // In range
        accrual.on_event(&mut state1);

        let mut state2 = PathState::new(20, 0.5);
        state2.set(state_keys::SPOT, 110.0); // Out of range
        accrual.on_event(&mut state2);

        let mut state3 = PathState::new(30, 0.75);
        state3.set(state_keys::SPOT, 98.0); // In range
        accrual.on_event(&mut state3);

        let value = accrual.value(Currency::USD);
        // 2 days in range / 3 total = 2/3 fraction
        // Payoff = 0.08 * (2/3) * 100_000 = 5_333.33...
        assert!((value.amount() - 5_333.333333).abs() < 1.0);
    }

    #[test]
    fn test_range_accrual_boundary() {
        let observation_dates = vec![0.25];
        let mut accrual = RangeAccrualPayoff::new(
            observation_dates,
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
        )
        .expect("valid range accrual inputs");

        // Exactly at lower boundary (should be in range)
        let mut state = PathState::new(10, 0.25);
        state.set(state_keys::SPOT, 95.0);
        accrual.on_event(&mut state);

        assert_eq!(accrual.days_in_range, 1);

        // Exactly at upper boundary (should be in range)
        accrual.reset();
        state.set(state_keys::SPOT, 105.0);
        accrual.on_event(&mut state);

        assert_eq!(accrual.days_in_range, 1);
    }

    #[test]
    fn test_range_accrual_reset() {
        let observation_dates = vec![0.25];
        let mut accrual = RangeAccrualPayoff::new(
            observation_dates,
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
        )
        .expect("valid range accrual inputs");

        let mut state = PathState::new(10, 0.25);
        state.set(state_keys::SPOT, 100.0);
        accrual.on_event(&mut state);

        assert_eq!(accrual.days_in_range, 1);
        assert_eq!(accrual.total_observations, 1);

        accrual.reset();

        assert_eq!(accrual.days_in_range, 0);
        assert_eq!(accrual.total_observations, 0);
        assert_eq!(accrual.next_obs_idx, 0);
    }

    #[test]
    fn test_range_accrual_with_history() {
        // Simulate mid-life valuation: 2 past observations (1 in range), 2 future
        let future_observation_dates = vec![0.25, 0.5]; // Future observations
        let mut accrual = RangeAccrualPayoff::new_with_history(
            future_observation_dates,
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
            1, // 1 past observation in range
            2, // 2 total past observations
        )
        .expect("valid range accrual inputs");

        // Simulate 1 future observation in range
        let mut state1 = PathState::new(10, 0.25);
        state1.set(state_keys::SPOT, 100.0); // In range
        accrual.on_event(&mut state1);

        // Simulate 1 future observation out of range
        let mut state2 = PathState::new(20, 0.5);
        state2.set(state_keys::SPOT, 110.0); // Out of range
        accrual.on_event(&mut state2);

        let value = accrual.value(Currency::USD);
        // Total: 2 in range (1 past + 1 future) / 4 total (2 past + 2 future) = 0.5 fraction
        // Payoff = 0.08 * 0.5 * 100_000 = 4_000
        assert!((value.amount() - 4_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_range_accrual_history_preserved_on_reset() {
        let observation_dates = vec![0.25];
        let mut accrual = RangeAccrualPayoff::new_with_history(
            observation_dates,
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
            3, // 3 past in range
            5, // 5 total past
        )
        .expect("valid range accrual inputs");

        let mut state = PathState::new(10, 0.25);
        state.set(state_keys::SPOT, 100.0);
        accrual.on_event(&mut state);

        accrual.reset();

        // Historical data should be preserved
        assert_eq!(accrual.past_in_range, 3);
        assert_eq!(accrual.total_past_observations, 5);
        // Simulation state should be reset
        assert_eq!(accrual.days_in_range, 0);
        assert_eq!(accrual.total_observations, 0);
    }

    #[test]
    fn test_range_accrual_multiple_observations_per_step() {
        // Test that multiple observations are handled when time step overshoots
        let observation_dates = vec![0.1, 0.2, 0.3]; // Close together
        let mut accrual = RangeAccrualPayoff::new(
            observation_dates,
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
        )
        .expect("valid range accrual inputs");

        // Single time step that passes all observations
        let mut state = PathState::new(10, 0.5); // Time = 0.5, past all observations
        state.set(state_keys::SPOT, 100.0); // In range
        accrual.on_event(&mut state);

        // All 3 observations should be counted
        assert_eq!(accrual.total_observations, 3);
        assert_eq!(accrual.days_in_range, 3);
    }

    #[test]
    fn test_range_accrual_rejects_degenerate_inputs() {
        // Inverted bounds: lower >= upper.
        assert!(RangeAccrualPayoff::new(
            vec![0.25, 0.5],
            105.0,
            95.0,
            0.08,
            100_000.0,
            Currency::USD,
        )
        .is_err());

        // Negative coupon rate.
        assert!(RangeAccrualPayoff::new(
            vec![0.25, 0.5],
            95.0,
            105.0,
            -0.01,
            100_000.0,
            Currency::USD,
        )
        .is_err());

        // Unsorted observation dates.
        assert!(RangeAccrualPayoff::new(
            vec![0.5, 0.25],
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
        )
        .is_err());

        // past_in_range exceeds total_past_observations.
        assert!(RangeAccrualPayoff::new_with_history(
            vec![0.25],
            95.0,
            105.0,
            0.08,
            100_000.0,
            Currency::USD,
            3,
            2,
        )
        .is_err());
    }
}
