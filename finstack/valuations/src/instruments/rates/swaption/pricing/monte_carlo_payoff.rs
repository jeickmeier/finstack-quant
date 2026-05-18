//! Swaption payoffs for Monte Carlo pricing.
//!
//! Implements Bermudan swaption pricing using Longstaff-Schwartz Monte Carlo.
//! A swaption is an option to enter into an interest rate swap at future dates.

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_monte_carlo::traits::PathState;
use finstack_monte_carlo::traits::Payoff;

/// Swaption type (payer or receiver).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwaptionType {
    /// Payer swaption: right to pay fixed rate
    Payer,
    /// Receiver swaption: right to receive fixed rate
    Receiver,
}

/// Swap schedule for Monte Carlo pricing.
///
/// Stores payment dates and accrual fractions for computing swap rates
/// and annuities from Hull-White short rate simulations.
#[derive(Debug, Clone)]
pub struct SwapSchedule {
    /// Payment dates (time in years from valuation date)
    pub payment_dates: Vec<f64>,
    /// Accrual fractions (daycount) for each period
    pub accrual_fractions: Vec<f64>,
    /// Start date of swap (time in years)
    pub start_date: f64,
    /// End date of swap (time in years)
    pub end_date: f64,
}

impl SwapSchedule {
    /// Create a new swap schedule.
    ///
    /// # Arguments
    ///
    /// * `start_date` - Swap start date (time in years)
    /// * `end_date` - Swap end date (time in years)
    /// * `payment_dates` - Payment dates (must be sorted, within [start_date, end_date])
    /// * `accrual_fractions` - Accrual fractions for each period
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if `payment_dates` and
    /// `accrual_fractions` differ in length, `start_date >= end_date`, the
    /// payment dates are not strictly sorted ascending, or any payment date
    /// falls outside `[start_date, end_date]`.
    pub fn new(
        start_date: f64,
        end_date: f64,
        payment_dates: Vec<f64>,
        accrual_fractions: Vec<f64>,
    ) -> finstack_core::Result<Self> {
        use std::cmp::Ordering;

        if payment_dates.len() != accrual_fractions.len() {
            return Err(finstack_core::Error::Validation(format!(
                "SwapSchedule: payment_dates ({}) and accrual_fractions ({}) must have the same length",
                payment_dates.len(),
                accrual_fractions.len()
            )));
        }
        if start_date.partial_cmp(&end_date) != Some(Ordering::Less) {
            return Err(finstack_core::Error::Validation(format!(
                "SwapSchedule: start_date ({start_date}) must be strictly before end_date ({end_date})"
            )));
        }
        // Verify payment dates are strictly sorted ascending and within range.
        for (i, &date) in payment_dates.iter().enumerate() {
            if i > 0 && payment_dates[i - 1].partial_cmp(&date) != Some(Ordering::Less) {
                return Err(finstack_core::Error::Validation(format!(
                    "SwapSchedule: payment_dates must be strictly sorted ascending; \
                     found {} >= {} at index {}",
                    payment_dates[i - 1],
                    date,
                    i
                )));
            }
            if date.is_nan() || date < start_date || date > end_date {
                return Err(finstack_core::Error::Validation(format!(
                    "SwapSchedule: payment date {date} at index {i} is outside \
                     [start_date {start_date}, end_date {end_date}]"
                )));
            }
        }

        Ok(Self {
            payment_dates,
            accrual_fractions,
            start_date,
            end_date,
        })
    }

    /// Compute annuity (PV01) at time t from discount factors.
    ///
    /// A(t) = Σ τ_i * DF(t, T_i) where τ_i are accrual fractions.
    #[cfg(test)]
    fn annuity(&self, discount_factors: &[f64]) -> f64 {
        assert_eq!(
            discount_factors.len(),
            self.payment_dates.len(),
            "Discount factors must match payment dates"
        );

        self.accrual_fractions
            .iter()
            .zip(discount_factors.iter())
            .map(|(tau, df)| tau * df)
            .sum()
    }
}

/// Bermudan swaption payoff.
///
/// A Bermudan swaption allows exercise at multiple dates before maturity.
/// At each exercise date, the holder can choose to enter into a swap with
/// fixed rate equal to the strike.
///
/// # Payoff
///
/// At exercise date t, if exercised:
/// - Payer: Pay fixed rate K, receive floating → value = (S(t) - K)⁺ · A(t) · N
/// - Receiver: Receive fixed rate K, pay floating → value = (K - S(t))⁺ · A(t) · N
///
/// where `S(t)` is the forward swap rate, `A(t)` is the **swap annuity** at the
/// exercise date and `N` the notional. The exercise-date value is then
/// discounted to the valuation date by the pathwise numéraire `B(t)`:
///
/// ```text
/// PV = (swap-rate diff)⁺ · A(t) · N / B(t)
/// ```
///
/// `(swap-rate diff)⁺` alone is a *rate*, not a present value: it must be
/// multiplied by the annuity (units of years) and divided by the numéraire to
/// become a discounted cashflow. The pricer records `A(t)` and `B(t)` via
/// [`BermudanSwaptionPayoff::record_exercise_state`] before the exercise event.
#[derive(Debug, Clone)]
pub struct BermudanSwaptionPayoff {
    /// Exercise dates (time in years from valuation date)
    pub exercise_dates: Vec<f64>,
    /// Swap schedule
    pub swap_schedule: SwapSchedule,
    /// Strike rate (fixed rate of the swap)
    pub strike: f64,
    /// Swaption type (payer or receiver)
    pub option_type: SwaptionType,
    /// Notional amount
    pub notional: f64,
    // State variables (tracked during path simulation)
    /// Current forward swap rate minus strike, payer convention `S(t) − K`
    /// (computed at exercise dates by the pricer). This is a *rate*, not a PV.
    current_swap_value: f64,
    /// Swap annuity `A(t)` at the current exercise date.
    ///
    /// Set by the pricer alongside `current_swap_value`. The exercise payoff
    /// is `(rate diff)⁺ · A(t) · N`; without the annuity the payoff would be a
    /// dimensionless rate rather than a cashflow.
    current_annuity: f64,
    /// Pathwise money-market numéraire `B(t)` at the current exercise date,
    /// used to discount the exercise value to the valuation date (`B(0) = 1`).
    current_numeraire: f64,
    /// Index of last exercise date checked
    next_exercise_idx: usize,
    /// Whether option was exercised
    exercised: bool,
    /// Exercise date (if exercised)
    exercise_date: Option<f64>,
}

impl BermudanSwaptionPayoff {
    /// Create a new Bermudan swaption payoff.
    ///
    /// # Arguments
    ///
    /// * `exercise_dates` - Dates when exercise is allowed (must be strictly
    ///   sorted ascending)
    /// * `swap_schedule` - Underlying swap schedule
    /// * `strike` - Fixed rate of the swap (e.g., 0.0325 for 3.25%)
    /// * `option_type` - Payer or receiver
    /// * `notional` - Notional amount
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if `exercise_dates` are not
    /// strictly sorted ascending. (Library code must not panic — a `debug`
    /// `assert!` would also be a `panic!` in debug builds.)
    pub fn new(
        exercise_dates: Vec<f64>,
        swap_schedule: SwapSchedule,
        strike: f64,
        option_type: SwaptionType,
        notional: f64,
    ) -> finstack_core::Result<Self> {
        // Verify exercise dates are strictly sorted ascending. `partial_cmp`
        // returning anything other than `Less` (including `None` for NaN)
        // is rejected.
        use std::cmp::Ordering;
        for i in 1..exercise_dates.len() {
            if exercise_dates[i - 1].partial_cmp(&exercise_dates[i]) != Some(Ordering::Less) {
                return Err(finstack_core::Error::Validation(format!(
                    "BermudanSwaptionPayoff: exercise_dates must be strictly sorted \
                     ascending; found {} >= {} at index {}",
                    exercise_dates[i - 1],
                    exercise_dates[i],
                    i
                )));
            }
        }

        Ok(Self {
            exercise_dates,
            swap_schedule,
            strike,
            option_type,
            notional,
            current_swap_value: 0.0,
            current_annuity: 0.0,
            current_numeraire: 1.0,
            next_exercise_idx: 0,
            exercised: false,
            exercise_date: None,
        })
    }

    /// Record the exercise-date state the payoff needs to produce a properly
    /// annuitied, discounted value.
    ///
    /// The pricer must call this *before* the exercise event so [`value`] can
    /// turn the raw rate difference into a present value:
    ///
    /// ```text
    /// PV = (rate diff)⁺ · annuity · notional / numeraire
    /// ```
    ///
    /// # Arguments
    ///
    /// * `swap_rate_minus_strike` - `S(t) − K` (payer convention), a rate.
    /// * `annuity` - swap annuity `A(t)` at the exercise date (`Σ τ_i P(t,T_i)`).
    /// * `numeraire` - pathwise money-market numéraire `B(t)` (`B(0) = 1`).
    ///
    /// [`value`]: Self::value
    //
    // Integration point for the generic `finstack_monte_carlo` `Payoff`-driven
    // engine. The production Bermudan-swaption path uses the dedicated
    // Longstaff-Schwartz induction in `monte_carlo_lsmc.rs`, which annuitises
    // and discounts directly, so this setter is currently exercised only by
    // unit tests — kept (rather than deleted) so the `Payoff` impl remains
    // self-consistent and correct for any caller routing through that engine.
    #[allow(dead_code)]
    pub fn record_exercise_state(
        &mut self,
        swap_rate_minus_strike: f64,
        annuity: f64,
        numeraire: f64,
    ) {
        self.current_swap_value = swap_rate_minus_strike;
        self.current_annuity = annuity;
        self.current_numeraire = numeraire;
    }

    /// Check if we should exercise at current time.
    ///
    /// For payer: exercise if S(t) > K (swap value > 0)
    /// For receiver: exercise if K > S(t) (swap value > 0 when considering receiver)
    fn should_exercise(&self) -> bool {
        match self.option_type {
            SwaptionType::Payer => self.current_swap_value > 0.0,
            SwaptionType::Receiver => self.current_swap_value < 0.0, // Receiver wants negative (pay float, receive fixed)
        }
    }
}

impl Payoff for BermudanSwaptionPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        // Check if we're at an exercise date
        if !self.exercised && self.next_exercise_idx < self.exercise_dates.len() {
            let target_date = self.exercise_dates[self.next_exercise_idx];

            // Check if current time matches exercise date (within tolerance)
            if (state.time - target_date).abs() < 1e-6 {
                // Swap value should be computed by pricer before calling on_event
                // If swap value indicates exercise, mark as exercised
                if self.should_exercise() {
                    self.exercised = true;
                    self.exercise_date = Some(target_date);
                }
                self.next_exercise_idx += 1;
            }
        }
    }

    fn value(&self, currency: Currency) -> Money {
        if self.exercised {
            // Exercise value is the discounted, annuitied swap value:
            //   PV = (rate diff)⁺ · A(t) · N / B(t)
            // The bare rate difference `current_swap_value` is NOT a present
            // value — it must be scaled by the annuity `A(t)` (years) and
            // discounted by the pathwise numéraire `B(t)`. Returning the raw
            // rate would inject a spurious dimensionless "cashflow".
            let rate_diff = match self.option_type {
                SwaptionType::Payer => self.current_swap_value.max(0.0),
                SwaptionType::Receiver => (-self.current_swap_value).max(0.0),
            };
            // `B(t) > 0` always (it is exp of an integral); guard defensively
            // so an unset/degenerate numéraire cannot produce a non-finite PV.
            let numeraire = if self.current_numeraire > 0.0 {
                self.current_numeraire
            } else {
                1.0
            };
            let pv = rate_diff * self.current_annuity * self.notional / numeraire;
            Money::new(pv, currency)
        } else {
            // Not exercised - value is zero (continuation value handled by LSMC)
            Money::new(0.0, currency)
        }
    }

    fn reset(&mut self) {
        self.current_swap_value = 0.0;
        self.current_annuity = 0.0;
        self.current_numeraire = 1.0;
        self.next_exercise_idx = 0;
        self.exercised = false;
        self.exercise_date = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_schedule_creation() {
        let payment_dates = vec![1.0, 1.25, 1.5, 1.75, 2.0];
        let accruals = vec![0.25, 0.25, 0.25, 0.25, 0.25];
        let schedule = SwapSchedule::new(1.0, 2.0, payment_dates, accruals)
            .expect("valid swap schedule inputs");

        assert_eq!(schedule.start_date, 1.0);
        assert_eq!(schedule.end_date, 2.0);
        assert_eq!(schedule.payment_dates.len(), 5);
    }

    #[test]
    fn test_swap_schedule_annuity() {
        let payment_dates = vec![1.0, 1.25, 1.5];
        let accruals = vec![0.25, 0.25, 0.25];
        let schedule = SwapSchedule::new(1.0, 1.5, payment_dates, accruals)
            .expect("valid swap schedule inputs");

        let discount_factors = vec![0.95, 0.94, 0.93];
        let annuity = schedule.annuity(&discount_factors);

        // Annuity = 0.25 * 0.95 + 0.25 * 0.94 + 0.25 * 0.93 = 0.705
        assert!((annuity - 0.705).abs() < 1e-10);
    }

    #[test]
    fn test_bermudan_swaption_payoff_creation() {
        let exercise_dates = vec![1.0, 1.5, 2.0];
        let payment_dates = vec![1.0, 1.25, 1.5, 1.75, 2.0];
        let accruals = vec![0.25, 0.25, 0.25, 0.25, 0.25];
        let schedule = SwapSchedule::new(1.0, 2.0, payment_dates, accruals)
            .expect("valid swap schedule inputs");

        let payoff = BermudanSwaptionPayoff::new(
            exercise_dates,
            schedule,
            0.0325,
            SwaptionType::Payer,
            10_000_000.0,
        )
        .expect("valid bermudan payoff inputs");

        assert_eq!(payoff.strike, 0.0325);
        assert_eq!(payoff.exercise_dates.len(), 3);
        assert!(!payoff.exercised);
    }

    #[test]
    fn test_bermudan_swaption_payoff_rejects_unsorted_exercise_dates() {
        let payment_dates = vec![1.0, 1.25];
        let accruals = vec![0.25, 0.25];
        let schedule = SwapSchedule::new(1.0, 1.25, payment_dates, accruals)
            .expect("valid swap schedule inputs");
        // Unsorted exercise dates must produce an Err, not a panic.
        let result = BermudanSwaptionPayoff::new(
            vec![1.5, 1.0],
            schedule,
            0.0325,
            SwaptionType::Payer,
            1.0,
        );
        assert!(
            result.is_err(),
            "unsorted exercise dates must return Err, not panic"
        );
    }

    /// Regression test (item 4): the Bermudan swaption exercise value must be
    /// the properly annuitied, discounted swap value — `(rate diff)⁺ · A · N /
    /// B` — not the bare rate difference. Previously `value()` returned
    /// `current_swap_value.max(0.0) · notional`, a dimensionless rate scaled by
    /// notional, omitting `× annuity` and discounting.
    #[test]
    fn exercised_payoff_is_annuitied_and_discounted() {
        let payment_dates = vec![1.0, 1.5, 2.0];
        let accruals = vec![0.5, 0.5, 0.5];
        let schedule = SwapSchedule::new(1.0, 2.0, payment_dates, accruals)
            .expect("valid swap schedule inputs");
        let mut payoff = BermudanSwaptionPayoff::new(
            vec![1.0],
            schedule,
            0.03,
            SwaptionType::Payer,
            10_000_000.0,
        )
        .expect("valid bermudan payoff inputs");

        // Pricer records: S − K = 1% rate diff, annuity 2.7, numéraire B = 1.05.
        let rate_diff = 0.01_f64;
        let annuity = 2.7_f64;
        let numeraire = 1.05_f64;
        payoff.record_exercise_state(rate_diff, annuity, numeraire);
        payoff.exercised = true;
        payoff.exercise_date = Some(1.0);

        let pv = payoff.value(Currency::USD).amount();

        // Correct payoff: (rate diff)⁺ · A · N / B.
        let expected = rate_diff * annuity * 10_000_000.0 / numeraire;
        assert!(
            (pv - expected).abs() < 1e-6,
            "exercise value must be annuitied and discounted: got {pv}, expected {expected}"
        );
        // The bare-rate bug would have returned rate_diff · notional = 100_000;
        // the correct value is materially different.
        let buggy = rate_diff * 10_000_000.0;
        assert!(
            (pv - buggy).abs() > 1.0,
            "payoff must differ from the unannuitied/undiscounted rate·notional"
        );
    }

    #[test]
    fn test_bermudan_swaption_reset() {
        let exercise_dates = vec![1.0];
        let payment_dates = vec![1.0, 1.25];
        let accruals = vec![0.25, 0.25];
        let schedule = SwapSchedule::new(1.0, 1.25, payment_dates, accruals)
            .expect("valid swap schedule inputs");

        let mut payoff =
            BermudanSwaptionPayoff::new(exercise_dates, schedule, 0.0325, SwaptionType::Payer, 1.0)
                .expect("valid bermudan payoff inputs");

        // Simulate some state
        payoff.current_swap_value = 0.01;
        payoff.exercised = true;
        payoff.exercise_date = Some(1.0);

        // Reset
        payoff.reset();

        assert_eq!(payoff.current_swap_value, 0.0);
        assert!(!payoff.exercised);
        assert_eq!(payoff.exercise_date, None);
        assert_eq!(payoff.next_exercise_idx, 0);
    }

    #[test]
    fn test_swap_schedule_rejects_degenerate_inputs() {
        // Mismatched lengths between payment dates and accrual fractions.
        assert!(SwapSchedule::new(1.0, 2.0, vec![1.0, 1.5], vec![0.25]).is_err());

        // start_date not before end_date.
        assert!(SwapSchedule::new(2.0, 1.0, vec![], vec![]).is_err());

        // Unsorted payment dates.
        assert!(SwapSchedule::new(1.0, 2.0, vec![1.5, 1.0], vec![0.25, 0.25]).is_err());

        // Payment date outside [start_date, end_date].
        assert!(SwapSchedule::new(1.0, 2.0, vec![2.5], vec![0.25]).is_err());
    }
}
