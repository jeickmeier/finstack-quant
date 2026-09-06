//! Asian option payoffs.
//!
//! Asian options depend on the average price over a period rather than
//! just the terminal price.
//!
//! - **Arithmetic Asian**: Average = (1/n) Σ S_i
//! - **Geometric Asian**: Average = (Π S_i)^(1/n)

use crate::monte_carlo::traits::PathState;
use crate::monte_carlo::traits::Payoff;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use std::collections::HashSet;

fn validate_fixing_schedule(fixing_steps: &[usize], initial_count: usize) -> Result<()> {
    if fixing_steps.is_empty() && initial_count == 0 {
        return Err(Error::Validation(
            "Asian payoff requires at least one fixing step or historical fixing".to_string(),
        ));
    }
    Ok(())
}

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

    sum_spots: f64,     // For arithmetic
    kahan_comp: f64,    // Kahan summation compensation for arithmetic
    product_spots: f64, // For geometric (stored as log-product)
    num_fixings_seen: usize,

    initial_sum_spots: f64,
    initial_kahan_comp: f64,
    initial_product_spots: f64,
    initial_count: usize,
}

impl AsianCall {
    /// Create an Asian call with at least one scheduled fixing and no history.
    ///
    /// Fixing indices may be unsorted or repeated. Each scheduled path event
    /// contributes once; the original vector is retained as contract metadata.
    ///
    /// # Arguments
    ///
    /// * `strike` - Exercise level in the same price units as simulated spot.
    /// * `notional` - Scalar multiplier applied to the positive difference
    ///   between the fixing average and `strike`; the payoff currency is
    ///   supplied separately to [`Payoff::value`].
    /// * `averaging` - Arithmetic mean of spot levels or geometric mean formed
    ///   from their natural logarithms.
    /// * `fixing_steps` - Owned, nonempty path-step indices at which spot enters
    ///   the average. Step `0` includes the initial spot; later indices refer
    ///   to post-step events and must fit within the pricing engine's grid.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `fixing_steps` is empty.
    pub fn new(
        strike: f64,
        notional: f64,
        averaging: AveragingMethod,
        fixing_steps: Vec<usize>,
    ) -> Result<Self> {
        Self::with_history(strike, notional, averaging, fixing_steps, 0.0, 0.0, 0)
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
    ///
    /// # Arguments
    ///
    /// * `strike` - Exercise level in the same price units as all historical
    ///   and future spot fixings.
    /// * `notional` - Scalar multiplier applied to the positive difference
    ///   between the fixing average and `strike`; [`Payoff::value`] supplies
    ///   the payoff currency separately.
    /// * `averaging` - Arithmetic mean using `initial_sum`, or geometric mean
    ///   using `initial_product_log`, combined with future simulated fixings.
    /// * `fixing_steps` - Owned indices of future path events to observe. Step
    ///   `0` includes the initial spot; indices may be unsorted or repeated,
    ///   but each scheduled event contributes once. An empty vector is valid
    ///   only when `initial_count` is positive, for a fully observed contract.
    /// * `initial_sum` - Sum of historical spot levels in spot-price units,
    ///   used for arithmetic averaging and restored before every new path.
    /// * `initial_product_log` - Sum of the natural logarithms of positive
    ///   historical spot levels, used for geometric averaging and restored
    ///   before every new path.
    /// * `initial_count` - Number of historical fixings represented by the
    ///   supplied aggregates; zero is permitted when future fixings exist.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `fixing_steps` is empty and
    /// `initial_count` is zero, leaving no observations for the average.
    pub fn with_history(
        strike: f64,
        notional: f64,
        averaging: AveragingMethod,
        fixing_steps: Vec<usize>,
        initial_sum: f64,
        initial_product_log: f64,
        initial_count: usize,
    ) -> Result<Self> {
        validate_fixing_schedule(&fixing_steps, initial_count)?;
        let fixing_set: HashSet<usize> = fixing_steps.iter().copied().collect();
        Ok(Self {
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
        })
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
    /// # Errors
    /// Returns an error if `SPOT` is missing or non-finite at a fixing step.
    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        if self.fixing_set.contains(&state.step) {
            let spot = super::require_finite_state(state.spot(), "SPOT", state.step)?;
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
        Ok(())
    }

    fn value(&self, currency: Currency) -> finstack_quant_core::Result<Money> {
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
    ///
    /// # Arguments
    ///
    /// * `strike` - Exercise level in the same price units as simulated spot.
    /// * `notional` - Scalar multiplier applied to the positive difference
    ///   between `strike` and the fixing average; the payoff currency is
    ///   supplied separately to [`Payoff::value`].
    /// * `averaging` - Arithmetic mean of spot levels or geometric mean formed
    ///   from their natural logarithms.
    /// * `fixing_steps` - Owned, nonempty path-step indices at which spot enters
    ///   the average. Step `0` includes the initial spot; later indices refer
    ///   to post-step events and must fit within the pricing engine's grid.
    ///   Indices may be unsorted or repeated; each scheduled event contributes once.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `fixing_steps` is empty.
    pub fn new(
        strike: f64,
        notional: f64,
        averaging: AveragingMethod,
        fixing_steps: Vec<usize>,
    ) -> Result<Self> {
        Self::with_history(strike, notional, averaging, fixing_steps, 0.0, 0.0, 0)
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
    ///
    /// # Arguments
    ///
    /// * `strike` - Exercise level in the same price units as all historical
    ///   and future spot fixings.
    /// * `notional` - Scalar multiplier applied to the positive difference
    ///   between `strike` and the fixing average; [`Payoff::value`] supplies
    ///   the payoff currency separately.
    /// * `averaging` - Arithmetic mean using `initial_sum`, or geometric mean
    ///   using `initial_product_log`, combined with future simulated fixings.
    /// * `fixing_steps` - Owned indices of future path events to observe. Step
    ///   `0` includes the initial spot; indices may be unsorted or repeated,
    ///   but each scheduled event contributes once. An empty vector is valid
    ///   only when `initial_count` is positive, for a fully observed contract.
    /// * `initial_sum` - Sum of historical spot levels in spot-price units,
    ///   used for arithmetic averaging and restored before every new path.
    /// * `initial_product_log` - Sum of the natural logarithms of positive
    ///   historical spot levels, used for geometric averaging and restored
    ///   before every new path.
    /// * `initial_count` - Number of historical fixings represented by the
    ///   supplied aggregates; zero is permitted when future fixings exist.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `fixing_steps` is empty and
    /// `initial_count` is zero, leaving no observations for the average.
    pub fn with_history(
        strike: f64,
        notional: f64,
        averaging: AveragingMethod,
        fixing_steps: Vec<usize>,
        initial_sum: f64,
        initial_product_log: f64,
        initial_count: usize,
    ) -> Result<Self> {
        validate_fixing_schedule(&fixing_steps, initial_count)?;
        let fixing_set: HashSet<usize> = fixing_steps.iter().copied().collect();
        Ok(Self {
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
        })
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
    /// # Errors
    /// Returns an error if `SPOT` is missing or non-finite at a fixing step.
    fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
        if self.fixing_set.contains(&state.step) {
            let spot = super::require_finite_state(state.spot(), "SPOT", state.step)?;
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
        Ok(())
    }

    fn value(&self, currency: Currency) -> finstack_quant_core::Result<Money> {
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
    crate::closed_form::geometric_asian_call(
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
    use crate::monte_carlo::traits::state_keys;

    fn create_state(step: usize, spot: f64) -> PathState {
        let mut state = PathState::new(step, step as f64 * 0.1);
        state.set(state_keys::SPOT, spot);
        state
    }

    #[test]
    fn test_asian_call_rejects_empty_fixings_without_history() {
        for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
            let error = AsianCall::new(100.0, 1.0, averaging, Vec::new())
                .expect_err("an average needs at least one fixing");
            assert!(matches!(error, Error::Validation(_)));

            let error = AsianCall::with_history(100.0, 1.0, averaging, Vec::new(), 0.0, 0.0, 0)
                .expect_err("empty history does not supply a fixing");
            assert!(matches!(error, Error::Validation(_)));
        }
    }

    #[test]
    fn test_asian_put_rejects_empty_fixings_without_history() {
        for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
            let error = AsianPut::new(100.0, 1.0, averaging, Vec::new())
                .expect_err("an average needs at least one fixing");
            assert!(matches!(error, Error::Validation(_)));

            let error = AsianPut::with_history(100.0, 1.0, averaging, Vec::new(), 0.0, 0.0, 0)
                .expect_err("empty history does not supply a fixing");
            assert!(matches!(error, Error::Validation(_)));
        }
    }

    #[test]
    fn test_asian_unsorted_duplicate_fixings_preserve_history_on_reset() {
        let fixing_steps = vec![2, 0, 2];
        let mut call = AsianCall::with_history(
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps.clone(),
            90.0,
            90.0_f64.ln(),
            1,
        )
        .expect("historical and future fixings");
        let mut put = AsianPut::with_history(
            120.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps.clone(),
            90.0,
            90.0_f64.ln(),
            1,
        )
        .expect("historical and future fixings");

        assert_eq!(call.fixing_steps, fixing_steps);
        assert_eq!(put.fixing_steps, fixing_steps);
        for _ in 0..2 {
            call.reset();
            put.reset();
            for (step, spot) in [(0, 100.0), (1, 1_000.0), (2, 140.0)] {
                let mut state = create_state(step, spot);
                call.on_event(&mut state).expect("valid call fixing");
                put.on_event(&mut state).expect("valid put fixing");
            }
            assert_eq!(call.num_fixings_seen, 3);
            assert_eq!(put.num_fixings_seen, 3);
            assert_eq!(
                call.value(Currency::USD).expect("valid payoff").amount(),
                10.0
            );
            assert_eq!(
                put.value(Currency::USD).expect("valid payoff").amount(),
                10.0
            );
        }
    }

    #[test]
    fn test_arithmetic_asian_call() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps)
            .expect("nonempty fixing schedule");

        // Simulate fixings: 90, 100, 110 -> average = 100
        let mut s0 = create_state(0, 90.0);
        let mut s1 = create_state(5, 100.0);
        let mut s2 = create_state(10, 110.0);
        asian.on_event(&mut s0).expect("valid payoff event");
        asian.on_event(&mut s1).expect("valid payoff event");
        asian.on_event(&mut s2).expect("valid payoff event");

        let value = asian.value(Currency::USD).expect("valid payoff");
        // Average = 100, strike = 100, payoff = 0
        assert_eq!(value.amount(), 0.0);
    }

    #[test]
    fn test_arithmetic_asian_call_itm() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps)
            .expect("nonempty fixing schedule");

        // Average = (100 + 110 + 120) / 3 = 110
        let mut s1 = create_state(0, 100.0);
        asian.on_event(&mut s1).expect("valid payoff event");
        let mut s2 = create_state(5, 110.0);
        asian.on_event(&mut s2).expect("valid payoff event");
        let mut s3 = create_state(10, 120.0);
        asian.on_event(&mut s3).expect("valid payoff event");

        let value = asian.value(Currency::USD).expect("valid payoff");
        // max(110 - 100, 0) = 10
        assert_eq!(value.amount(), 10.0);
    }

    #[test]
    fn test_geometric_asian_call() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianCall::new(100.0, 1.0, AveragingMethod::Geometric, fixing_steps)
            .expect("nonempty fixing schedule");

        // Geometric average of (80, 100, 125) = (80*100*125)^(1/3) = 100
        let mut s4 = create_state(0, 80.0);
        asian.on_event(&mut s4).expect("valid payoff event");
        let mut s5 = create_state(5, 100.0);
        asian.on_event(&mut s5).expect("valid payoff event");
        let mut s6 = create_state(10, 125.0);
        asian.on_event(&mut s6).expect("valid payoff event");

        let value = asian.value(Currency::USD).expect("valid payoff");
        let expected_avg = (80.0 * 100.0 * 125.0_f64).powf(1.0 / 3.0);
        let expected_payoff = (expected_avg - 100.0).max(0.0);
        assert!((value.amount() - expected_payoff).abs() < 0.01);
    }

    #[test]
    fn test_asian_put() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianPut::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps)
            .expect("nonempty fixing schedule");

        // Average = (90 + 95 + 100) / 3 = 95
        let mut s7 = create_state(0, 90.0);
        asian.on_event(&mut s7).expect("valid payoff event");
        let mut s8 = create_state(5, 95.0);
        asian.on_event(&mut s8).expect("valid payoff event");
        let mut s9 = create_state(10, 100.0);
        asian.on_event(&mut s9).expect("valid payoff event");

        let value = asian.value(Currency::USD).expect("valid payoff");
        // max(100 - 95, 0) = 5
        assert_eq!(value.amount(), 5.0);
    }

    #[test]
    fn test_asian_reset() {
        let fixing_steps = vec![0, 5, 10];
        let mut asian = AsianCall::new(100.0, 1.0, AveragingMethod::Arithmetic, fixing_steps)
            .expect("nonempty fixing schedule");

        let mut s10 = create_state(0, 100.0);
        asian.on_event(&mut s10).expect("valid payoff event");
        let mut s11 = create_state(5, 110.0);
        asian.on_event(&mut s11).expect("valid payoff event");
        assert_eq!(asian.num_fixings_seen, 2);

        asian.reset();
        assert_eq!(asian.num_fixings_seen, 0);
        assert_eq!(asian.sum_spots, 0.0);
    }

    #[test]
    fn test_geometric_asian_closed_form() {
        let price = geometric_asian_call_closed_form(100.0, 100.0, 1.0, 0.05, 0.02, 0.2, 12);

        assert!(price > 0.0);
        assert!(price < 10.0);
    }
}
