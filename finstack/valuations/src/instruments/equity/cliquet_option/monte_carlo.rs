//! Cliquet option payoffs for Monte Carlo pricing.
//!
//! Cliquet options have periodic resets where the strike is reset to the current
//! spot at reset dates, effectively creating a series of forward-starting options.

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_core::Error as CoreError;
use finstack_monte_carlo::traits::PathState;
use finstack_monte_carlo::traits::Payoff;

/// Cliquet call option payoff.
///
/// A cliquet option accumulates returns over multiple periods with periodic resets.
/// At each reset date, the strike is reset to the current spot, and returns
/// are accumulated subject to local and global caps/floors.
///
/// # Payoff Structure
///
/// Period Return R_i = S_i / S_{i-1} - 1
/// Capped/Floored R_i^* = min(max(R_i, local_floor), local_cap)
///
/// Total Payoff = Notional × min(max(Σ R_i^*, global_floor), global_cap)
///
/// where:
/// - S_i is spot at reset date i
/// - local_cap/local_floor are limits per period
/// - global_cap/global_floor are limits on total return
#[derive(Debug, Clone)]
pub struct CliquetCallPayoff {
    /// Reset dates (time in years, must be sorted)
    pub reset_dates: Vec<f64>,
    /// Local cap per period (e.g., 0.05 for 5% max per period)
    pub local_cap: f64,
    /// Local floor per period (e.g., 0.0 for 0% min per period)
    pub local_floor: f64,
    /// Global cap on total return (e.g., 0.20 for 20% max total)
    pub global_cap: f64,
    /// Global floor on total return (e.g., 0.0 for 0% min total)
    pub global_floor: f64,
    /// Notional amount
    pub notional: f64,
    /// Currency
    pub currency: Currency,
    /// Initial spot price S_0
    pub initial_spot: f64,

    // State variables (tracked during path simulation)
    /// Spot prices at reset dates
    reset_spots: Vec<f64>,
    /// Index of next reset date to check
    next_reset_idx: usize,
    /// Payoff type (Additive or Multiplicative)
    pub payoff_type: CliquetPayoffType,
}

/// Cliquet payoff aggregation type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CliquetPayoffType {
    /// Additive: Sum of period returns
    #[default]
    Additive,
    /// Multiplicative: Product of (1 + period returns) - 1
    Multiplicative,
}

impl CliquetCallPayoff {
    /// Create a new cliquet call payoff.
    ///
    /// # Arguments
    ///
    /// * `reset_dates` - Period-end observation times in years (must be strictly
    ///   increasing and strictly positive). Period 1 runs from the contract
    ///   start to `reset_dates[0]` and is anchored to `initial_spot`; a t=0
    ///   strike-set reset is NOT a period observation and must be excluded.
    /// * `local_cap` - Maximum return per period (e.g., 0.05)
    /// * `local_floor` - Minimum return per period (e.g., 0.0)
    /// * `global_cap` - Maximum total return (e.g., 0.20)
    /// * `global_floor` - Minimum total return (e.g., 0.0)
    /// * `notional` - Notional amount
    /// * `currency` - Currency
    /// * `initial_spot` - Initial spot price S_0
    /// * `payoff_type` - Additive or Multiplicative aggregation
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reset_dates: Vec<f64>,
        local_cap: f64,
        local_floor: f64,
        global_cap: f64,
        global_floor: f64,
        notional: f64,
        currency: Currency,
        initial_spot: f64,
        payoff_type: CliquetPayoffType,
    ) -> finstack_core::Result<Self> {
        for i in 1..reset_dates.len() {
            if reset_dates[i - 1] >= reset_dates[i] {
                return Err(CoreError::Validation(format!(
                    "CliquetCallPayoff: reset_dates must be strictly increasing (index {} = {} >= index {} = {})",
                    i - 1,
                    reset_dates[i - 1],
                    i,
                    reset_dates[i]
                )));
            }
        }
        if local_cap < local_floor {
            return Err(CoreError::Validation(format!(
                "CliquetCallPayoff: local_cap ({local_cap}) must be >= local_floor ({local_floor})"
            )));
        }
        if global_cap < global_floor {
            return Err(CoreError::Validation(format!(
                "CliquetCallPayoff: global_cap ({global_cap}) must be >= global_floor ({global_floor})"
            )));
        }

        Ok(Self {
            reset_dates,
            local_cap,
            local_floor,
            global_cap,
            global_floor,
            notional,
            currency,
            initial_spot,
            reset_spots: Vec::new(),
            next_reset_idx: 0,
            payoff_type,
        })
    }

    /// Compute cliquet return from reset spots.
    ///
    /// Returns accumulated return subject to local and global caps/floors.
    fn compute_return(&self) -> f64 {
        if self.reset_spots.is_empty() {
            return 0.0;
        }

        match self.payoff_type {
            CliquetPayoffType::Additive => {
                let mut total_return = 0.0;
                let mut prev_spot = self.initial_spot;

                for &spot in &self.reset_spots {
                    // Period return: S_i / S_{i-1} - 1
                    let raw_return = spot / prev_spot - 1.0;
                    // Apply local floor and cap
                    let period_return = raw_return.max(self.local_floor).min(self.local_cap);
                    total_return += period_return;
                    prev_spot = spot;
                }

                // Apply global floor and cap
                total_return.max(self.global_floor).min(self.global_cap)
            }
            CliquetPayoffType::Multiplicative => {
                let mut total_growth = 1.0;
                let mut prev_spot = self.initial_spot;

                for &spot in &self.reset_spots {
                    let raw_return = spot / prev_spot - 1.0;
                    let period_return = raw_return.max(self.local_floor).min(self.local_cap);
                    total_growth *= 1.0 + period_return;
                    prev_spot = spot;
                }

                let total_return = total_growth - 1.0;
                total_return.max(self.global_floor).min(self.global_cap)
            }
        }
    }
}

impl Payoff for CliquetCallPayoff {
    fn on_event(&mut self, state: &mut PathState) {
        const EPS: f64 = 1e-6;
        let Some(spot) = state.spot() else {
            return;
        };
        // Capture every reset date now due. A single MC time step can span
        // multiple reset dates (coarse grid); each due reset must record a spot
        // so period returns are computed against the correct number of resets.
        while self.next_reset_idx < self.reset_dates.len()
            && state.time >= self.reset_dates[self.next_reset_idx] - EPS
        {
            self.reset_spots.push(spot);
            self.next_reset_idx += 1;
        }
    }

    fn value(&self, currency: Currency) -> Money {
        // Compute total cliquet return
        let total_return = self.compute_return();

        // Payoff = total_return * notional
        Money::new(total_return * self.notional, currency)
    }

    fn reset(&mut self) {
        self.reset_spots.clear();
        self.next_reset_idx = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cliquet_creation() {
        let reset_dates = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let cliquet = CliquetCallPayoff::new(
            reset_dates,
            0.10, // 10% local cap
            0.0,  // 0% local floor
            0.30, // 30% global cap
            0.0,  // 0% global floor
            100_000.0,
            Currency::USD,
            100.0,
            CliquetPayoffType::Additive,
        )
        .expect("test fixture is well-formed");

        assert_eq!(cliquet.reset_dates.len(), 5);
        assert_eq!(cliquet.local_cap, 0.10);
        assert_eq!(cliquet.local_floor, 0.0);
        assert_eq!(cliquet.global_cap, 0.30);
        assert_eq!(cliquet.global_floor, 0.0);
    }

    #[test]
    fn test_cliquet_compute_return() {
        let reset_dates = vec![0.0, 0.25, 0.5];
        let mut cliquet = CliquetCallPayoff::new(
            reset_dates,
            0.10, // 10% local cap
            0.0,  // 0% local floor
            0.30, // 30% global cap
            0.0,  // 0% global floor
            1.0,
            Currency::USD,
            100.0,
            CliquetPayoffType::Additive,
        )
        .expect("test fixture is well-formed");

        // Simulate resets: 100 -> 110 -> 115
        cliquet.reset_spots = vec![110.0, 115.0];

        let return_val = cliquet.compute_return();
        // Period 1: min(max(110/100 - 1, 0), 0.10) = min(0.10, 0.10) = 0.10
        // Period 2: min(max(115/110 - 1, 0), 0.10) = min(0.0454545..., 0.10) = 0.0454545...
        // Total: 0.10 + 0.0454545... = 0.1454545...
        assert!((return_val - 0.14545454545454542).abs() < 1e-10);
    }

    #[test]
    fn test_cliquet_local_cap_floor() {
        let reset_dates = vec![0.0, 0.25, 0.5];
        let mut cliquet = CliquetCallPayoff::new(
            reset_dates,
            0.10,  // 10% local cap
            -0.05, // -5% local floor
            0.30,  // 30% global cap
            -0.20, // -20% global floor
            1.0,
            Currency::USD,
            100.0,
            CliquetPayoffType::Additive,
        )
        .expect("test fixture is well-formed");

        // Simulate: 100 -> 150 (hit cap) -> 100 (drop 33%, hit floor)
        cliquet.reset_spots = vec![150.0, 100.0];

        let return_val = cliquet.compute_return();
        // Period 1: 150/100 - 1 = 0.50 -> capped at 0.10
        // Period 2: 100/150 - 1 = -0.333... -> floored at -0.05
        // Total: 0.10 - 0.05 = 0.05
        assert!((return_val - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_cliquet_global_cap() {
        let reset_dates = vec![0.0, 0.25, 0.5, 0.75];
        let mut cliquet = CliquetCallPayoff::new(
            reset_dates,
            0.10, // 10% local cap
            0.0,  // 0% local floor
            0.30, // 30% global cap
            0.0,  // 0% global floor
            1.0,
            Currency::USD,
            100.0,
            CliquetPayoffType::Additive,
        )
        .expect("test fixture is well-formed");

        // Simulate 4 periods each hitting local cap: 4 * 10% = 40%, but capped at 30%
        cliquet.reset_spots = vec![110.0, 121.0, 133.1, 146.41];

        let return_val = cliquet.compute_return();
        // Total uncapped: 0.10 + 0.10 + 0.10 + 0.10 = 0.40
        // But global cap: min(0.40, 0.30) = 0.30
        assert!((return_val - 0.30).abs() < 1e-10);
    }

    #[test]
    fn coarse_step_spanning_multiple_reset_dates_captures_all() {
        use finstack_monte_carlo::traits::state_keys;

        // A coarse MC grid has steps that each span several reset dates. Every
        // due reset must record a spot so reset_spots has one entry per reset.
        let reset_dates = vec![0.25, 0.5, 0.75, 1.0];
        let mut cliquet = CliquetCallPayoff::new(
            reset_dates.clone(),
            0.10,
            0.0,
            0.30,
            0.0,
            100_000.0,
            Currency::USD,
            100.0,
            CliquetPayoffType::Additive,
        )
        .expect("test fixture is well-formed");

        // First coarse step at t = 0.5 spans reset dates 0 (0.25) and 1 (0.5).
        let mut state = PathState::new(1, 0.5);
        state.set(state_keys::SPOT, 105.0);
        cliquet.on_event(&mut state);
        assert_eq!(
            cliquet.next_reset_idx, 2,
            "a step spanning two reset dates must consume both"
        );

        // Second coarse step at t = 1.0 spans reset dates 2 (0.75) and 3 (1.0).
        let mut state = PathState::new(2, 1.0);
        state.set(state_keys::SPOT, 110.0);
        cliquet.on_event(&mut state);

        assert_eq!(
            cliquet.next_reset_idx, 4,
            "all reset dates must be consumed after the coarse grid completes"
        );
        assert_eq!(
            cliquet.reset_spots.len(),
            reset_dates.len(),
            "reset_spots must hold exactly one spot per reset date"
        );
        assert_eq!(cliquet.reset_spots, vec![105.0, 105.0, 110.0, 110.0]);
    }

    #[test]
    fn test_cliquet_reset() {
        let reset_dates = vec![0.0, 0.25];
        let mut cliquet = CliquetCallPayoff::new(
            reset_dates,
            0.10,
            0.0,
            0.30,
            0.0,
            1.0,
            Currency::USD,
            100.0,
            CliquetPayoffType::Additive,
        )
        .expect("test fixture is well-formed");

        cliquet.reset_spots = vec![110.0];
        cliquet.next_reset_idx = 1;

        cliquet.reset();

        assert!(cliquet.reset_spots.is_empty());
        assert_eq!(cliquet.next_reset_idx, 0);
        assert_eq!(cliquet.compute_return(), 0.0);
    }
}
