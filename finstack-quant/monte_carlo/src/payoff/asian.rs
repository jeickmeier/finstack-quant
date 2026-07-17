//! Asian option payoffs.
//!
//! Asian options depend on the average price over a period rather than
//! just the terminal price.
//!
//! - **Arithmetic Asian**: Average = (1/n) Σ S_i
//! - **Geometric Asian**: Average = (Π S_i)^(1/n)

use crate::traits::PathState;
use crate::traits::Payoff;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use std::collections::HashSet;

/// Default Asian fixing schedule for convenience pricers.
///
/// The engine emits an initial event at step `0` before any simulated move and
/// then post-step events `1..=num_steps`. Binding-level convenience methods use
/// the post-step schedule so the initial spot is not included as a fixing.
///
/// # Arguments
///
/// * `num_steps` - Number of simulated time-grid intervals; the returned
///   fixing indices are `1..=num_steps` and exclude the time-zero spot.
#[must_use]
pub fn default_fixing_steps(num_steps: usize) -> Vec<usize> {
    (1..=num_steps).collect()
}

/// Asian averaging method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AveragingMethod {
    /// Arithmetic average: (1/n) Σ S_i
    Arithmetic,
    /// Geometric average: (Π S_i)^(1/n)
    Geometric,
}

/// Asian call option.
///
/// Payoff: max(Avg - K, 0) × N
///
/// where Avg is computed using the specified averaging method.
///
/// Uses Kahan summation for arithmetic averaging to maintain numerical
/// stability when there are many fixing dates (e.g., daily monitoring).
#[derive(Debug, Clone)]
pub struct AsianCall {
    /// Strike price
    pub strike: f64,
    /// Notional
    pub notional: f64,
    /// Averaging method
    pub averaging: AveragingMethod,
    /// Fixing steps (indices where we sample the spot)
    pub fixing_steps: Vec<usize>,
    /// O(1) lookup set derived from fixing_steps
    fixing_set: HashSet<usize>,

    // State
    sum_spots: f64,     // For arithmetic
    kahan_comp: f64,    // Kahan summation compensation for arithmetic
    product_spots: f64, // For geometric (stored as log-product)
    num_fixings_seen: usize,

    // History
    initial_sum_spots: f64,
    initial_kahan_comp: f64,
    initial_product_spots: f64,
    initial_count: usize,
}

impl AsianCall {
    /// Create a new Asian call option.
    ///
    /// # Arguments
    ///
    /// * `strike` - Strike price
    /// * `notional` - Notional amount
    /// * `averaging` - Averaging method (arithmetic or geometric)
    /// * `fixing_steps` - Time step indices for averaging
    pub fn new(
        strike: f64,
        notional: f64,
        averaging: AveragingMethod,
        fixing_steps: Vec<usize>,
    ) -> Self {
        let fixing_set: HashSet<usize> = fixing_steps.iter().copied().collect();
        Self {
            strike,
            notional,
            averaging,
            fixing_steps,
            fixing_set,
            sum_spots: 0.0,
            kahan_comp: 0.0,
            product_spots: 0.0,
            num_fixings_seen: 0,
            initial_sum_spots: 0.0,
            initial_kahan_comp: 0.0,
            initial_product_spots: 0.0,
            initial_count: 0,
        }
    }

    /// Resume an Asian call after historical fixings have already occurred.
    ///
    /// `initial_sum` is the sum of historical fixing levels and
    /// `initial_product_log` is the sum of their natural logarithms. The latter
    /// is used only for geometric averaging. `initial_count` is the number of
    /// historical observations included in those aggregates; future simulated
    /// fixing steps are appended to this state.
    ///
    /// The constructor does not validate that the aggregates, count, and
    /// `fixing_steps` describe the same schedule, so callers restoring a
    /// partially observed trade must preserve that invariant.
    pub fn with_history(
        strike: f64,
        notional: f64,
        averaging: AveragingMethod,
        fixing_steps: Vec<usize>,
        initial_sum: f64,
        initial_product_log: f64,
        initial_count: usize,
    ) -> Self {
        let fixing_set: HashSet<usize> = fixing_steps.iter().copied().collect();
        Self {
            strike,
            notional,
            averaging,
            fixing_steps,
            fixing_set,
            sum_spots: initial_sum,
            kahan_comp: 0.0,
            product_spots: initial_product_log,
            num_fixings_seen: initial_count,
            initial_sum_spots: initial_sum,
            initial_kahan_comp: 0.0,
            initial_product_spots: initial_product_log,
            initial_count,
        }
    }

    /// Compute the average based on accumulated samples.
    fn compute_average(&self) -> f64 {
        if self.num_fixings_seen == 0 {
            return 0.0;
        }

        match self.averaging {
            AveragingMethod::Arithmetic => self.sum_spots / self.num_fixings_seen as f64,
            AveragingMethod::Geometric => {
                // exp(log-sum / n) = (product)^(1/n)
                (self.product_spots / self.num_fixings_seen as f64).exp()
            }
        }
    }

    /// Add a value using Kahan compensated summation.
    ///
    /// Kahan summation reduces floating-point error from O(n*ε) to O(ε)
    /// where ε is machine epsilon. This is critical for options with
    /// many fixing dates (e.g., 252 daily fixings).
    #[inline]
    fn kahan_add(&mut self, value: f64) {
        let y = value - self.kahan_comp;
        let t = self.sum_spots + y;
        self.kahan_comp = (t - self.sum_spots) - y;
        self.sum_spots = t;
    }
}

impl Payoff for AsianCall {
    /// Accumulate the spot fixing when the current step is a fixing step.
    ///
    /// # Panics
    ///
    /// Panics if `SPOT` is missing or non-finite at a fixing step — see
    /// `require_finite_state`.
    fn on_event(&mut self, state: &mut PathState) {
        if self.fixing_set.contains(&state.step) {
            let spot = super::require_finite_state(state.spot(), "SPOT", state.step);
            match self.averaging {
                AveragingMethod::Arithmetic => {
                    // Use Kahan summation for numerical stability
                    self.kahan_add(spot);
                }
                AveragingMethod::Geometric => {
                    // Store as log-sum for numerical stability
                    self.product_spots += spot.ln();
                }
            }
            self.num_fixings_seen += 1;
        }
    }

    fn value(&self, currency: Currency) -> Money {
        let average = self.compute_average();
        let intrinsic = (average - self.strike).max(0.0);
        Money::new(intrinsic * self.notional, currency)
    }

    /// The last contracted fixing step: the engine validates that the time
    /// grid reaches it, so configured fixings can never silently fall off
    /// the grid and shrink the average.
    fn max_event_step(&self) -> Option<usize> {
        self.fixing_steps.iter().max().copied()
    }

    fn reset(&mut self) {
        self.sum_spots = self.initial_sum_spots;
        self.kahan_comp = self.initial_kahan_comp;
        self.product_spots = self.initial_product_spots;
        self.num_fixings_seen = self.initial_count;
    }
}

/// Asian put option.
///
/// Payoff: max(K - Avg, 0) × N
///
/// Uses Kahan summation for arithmetic averaging to maintain numerical
/// stability when there are many fixing dates (e.g., daily monitoring).
#[derive(Debug, Clone)]
pub struct AsianPut {
    /// Strike price
    pub strike: f64,
    /// Notional amount
    pub notional: f64,
    /// Averaging method (arithmetic or geometric)
    pub averaging: AveragingMethod,
    /// Time step indices for averaging observations
    pub fixing_steps: Vec<usize>,
    /// O(1) lookup set derived from fixing_steps
    fixing_set: HashSet<usize>,

    sum_spots: f64,
    kahan_comp: f64,
    product_spots: f64,
    num_fixings_seen: usize,

    // History
    initial_sum_spots: f64,
    initial_kahan_comp: f64,
    initial_product_spots: f64,
    initial_count: usize,
}

impl AsianPut {
    /// Create an Asian put with no historical fixings.
    ///
    /// The payoff is `max(strike - average, 0) * notional`, where `average`
    /// is computed over the supplied path-step indices using `averaging`.
    /// Fixing indices are deduplicated for lookup while their original vector
    /// is retained as contract metadata.
    pub fn new(
        strike: f64,
        notional: f64,
        averaging: AveragingMethod,
        fixing_steps: Vec<usize>,
    ) -> Self {
        let fixing_set: HashSet<usize> = fixing_steps.iter().copied().collect();
        Self {
            strike,
            notional,
            averaging,
            fixing_steps,
            fixing_set,
            sum_spots: 0.0,
            kahan_comp: 0.0,
            product_spots: 0.0,
            num_fixings_seen: 0,
            initial_sum_spots: 0.0,
            initial_kahan_comp: 0.0,
            initial_product_spots: 0.0,
            initial_count: 0,
        }
    }

    /// Resume an Asian put after historical fixings have already occurred.
    ///
    /// `initial_sum` is the sum of historical fixing levels and
    /// `initial_product_log` is the sum of their natural logarithms. The latter
    /// is used only for geometric averaging. `initial_count` is the number of
    /// historical observations included in those aggregates; future simulated
    /// fixing steps are appended to this state.
    ///
    /// The constructor does not validate that the aggregates, count, and
    /// `fixing_steps` describe the same schedule, so callers restoring a
    /// partially observed trade must preserve that invariant.
    pub fn with_history(
        strike: f64,
        notional: f64,
        averaging: AveragingMethod,
        fixing_steps: Vec<usize>,
        initial_sum: f64,
        initial_product_log: f64,
        initial_count: usize,
    ) -> Self {
        let fixing_set: HashSet<usize> = fixing_steps.iter().copied().collect();
        Self {
            strike,
            notional,
            averaging,
            fixing_steps,
            fixing_set,
            sum_spots: initial_sum,
            kahan_comp: 0.0,
            product_spots: initial_product_log,
            num_fixings_seen: initial_count,
            initial_sum_spots: initial_sum,
            initial_kahan_comp: 0.0,
            initial_product_spots: initial_product_log,
            initial_count,
        }
    }

    fn compute_average(&self) -> f64 {
        if self.num_fixings_seen == 0 {
            return 0.0;
        }

        match self.averaging {
            AveragingMethod::Arithmetic => self.sum_spots / self.num_fixings_seen as f64,
            AveragingMethod::Geometric => (self.product_spots / self.num_fixings_seen as f64).exp(),
        }
    }

    /// Add a value using Kahan compensated summation.
    #[inline]
    fn kahan_add(&mut self, value: f64) {
        let y = value - self.kahan_comp;
        let t = self.sum_spots + y;
        self.kahan_comp = (t - self.sum_spots) - y;
        self.sum_spots = t;
    }
}

impl Payoff for AsianPut {
    /// Accumulate the spot fixing when the current step is a fixing step.
    ///
    /// # Panics
    ///
    /// Panics if `SPOT` is missing or non-finite at a fixing step — see
    /// `require_finite_state`.
    fn on_event(&mut self, state: &mut PathState) {
        if self.fixing_set.contains(&state.step) {
            let spot = super::require_finite_state(state.spot(), "SPOT", state.step);
            match self.averaging {
                AveragingMethod::Arithmetic => {
                    // Use Kahan summation for numerical stability
                    self.kahan_add(spot);
                }
                AveragingMethod::Geometric => {
                    self.product_spots += spot.ln();
                }
            }
            self.num_fixings_seen += 1;
        }
    }

    fn value(&self, currency: Currency) -> Money {
        let average = self.compute_average();
        let intrinsic = (self.strike - average).max(0.0);
        Money::new(intrinsic * self.notional, currency)
    }

    /// The last contracted fixing step: the engine validates that the time
    /// grid reaches it, so configured fixings can never silently fall off
    /// the grid and shrink the average.
    fn max_event_step(&self) -> Option<usize> {
        self.fixing_steps.iter().max().copied()
    }

    fn reset(&mut self) {
        self.sum_spots = self.initial_sum_spots;
        self.kahan_comp = self.initial_kahan_comp;
        self.product_spots = self.initial_product_spots;
        self.num_fixings_seen = self.initial_count;
    }
}

/// Closed-form price for geometric Asian call under GBM.
///
/// Delegates to the canonical implementation in `closed_form::asian::geometric_asian_call`
/// which uses the correct adjusted volatility formula:
///   σ_adj = σ × √((n+1)(2n+1)/(6n²))
///
/// # Arguments
///
/// * `spot` - Initial underlying spot in the same price units as `strike`.
/// * `strike` - Option exercise price in the same price units as `spot`.
/// * `time_to_maturity` - Time to maturity
/// * `rate` - Continuously compounded annual risk-free rate as a decimal.
/// * `dividend_yield` - Continuously compounded annual dividend or carry yield
///   as a decimal.
/// * `volatility` - Annualized lognormal volatility as a decimal.
/// * `num_fixings` - Number of averaging points
///
/// # Returns
///
/// Present value of geometric Asian call
pub fn geometric_asian_call_closed_form(
    spot: f64,
    strike: f64,
    time_to_maturity: f64,
    rate: f64,
    dividend_yield: f64,
    volatility: f64,
    num_fixings: usize,
) -> f64 {
    finstack_quant_core::math::volatility::geometric_asian_call(
        spot,
        strike,
        time_to_maturity,
        rate,
        dividend_yield,
        volatility,
        num_fixings,
    )
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
    fn test_arithmetic_asian_call() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        // Simulate fixings: 90, 100, 110 -> average = 100
        let mut s0 = create_state(0, 90.0);
        let mut s1 = create_state(5, 100.0);
        let mut s2 = create_state(10, 110.0);
        asian.on_event(&mut s0);
        asian.on_event(&mut s1);
        asian.on_event(&mut s2);

        let value = asian.value(Currency::USD);
        // Average = 100, strike = 100, payoff = 0
        assert_eq!(value.amount(), 0.0);
    }

    #[test]
    fn test_arithmetic_asian_call_itm() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        // Average = (100 + 110 + 120) / 3 = 110
        let mut s1 = create_state(0, 100.0);
        asian.on_event(&mut s1);
        let mut s2 = create_state(5, 110.0);
        asian.on_event(&mut s2);
        let mut s3 = create_state(10, 120.0);
        asian.on_event(&mut s3);

        let value = asian.value(Currency::USD);
        // max(110 - 100, 0) = 10
        assert_eq!(value.amount(), 10.0);
    }

    #[test]
    fn test_geometric_asian_call() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianCall::new(100.0, 1.0, AveragingMethod::Geometric, fixing_steps);

        // Geometric average of (80, 100, 125) = (80*100*125)^(1/3) = 100
        let mut s4 = create_state(0, 80.0);
        asian.on_event(&mut s4);
        let mut s5 = create_state(5, 100.0);
        asian.on_event(&mut s5);
        let mut s6 = create_state(10, 125.0);
        asian.on_event(&mut s6);

        let value = asian.value(Currency::USD);
        let expected_avg = (80.0 * 100.0 * 125.0_f64).powf(1.0 / 3.0);
        let expected_payoff = (expected_avg - 100.0).max(0.0);
        assert!((value.amount() - expected_payoff).abs() < 0.01);
    }

    #[test]
    fn test_asian_put() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianPut::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        // Average = (90 + 95 + 100) / 3 = 95
        let mut s7 = create_state(0, 90.0);
        asian.on_event(&mut s7);
        let mut s8 = create_state(5, 95.0);
        asian.on_event(&mut s8);
        let mut s9 = create_state(10, 100.0);
        asian.on_event(&mut s9);

        let value = asian.value(Currency::USD);
        // max(100 - 95, 0) = 5
        assert_eq!(value.amount(), 5.0);
    }

    #[test]
    fn test_asian_reset() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps);

        let mut s10 = create_state(0, 100.0);
        asian.on_event(&mut s10);
        let mut s11 = create_state(5, 110.0);
        asian.on_event(&mut s11);
        assert_eq!(asian.num_fixings_seen, 2);

        asian.reset();
        assert_eq!(asian.num_fixings_seen, 0);
        assert_eq!(asian.sum_spots, 0.0);
    }

    #[test]
    fn test_geometric_asian_closed_form() {
        // Test that closed form gives reasonable results
        let price = geometric_asian_call_closed_form(100.0, 100.0, 1.0, 0.05, 0.02, 0.2, 12);

        // Should be positive and less than ATM European
        assert!(price > 0.0);
        assert!(price < 10.0); // Reasonable range
    }
}
