//! Geometric Brownian Motion with discrete dividends.
//!
//! Extends the standard GBM process to handle discrete dividend payments,
//! which are critical for pricing equity derivatives (especially American options
//! near ex-dividend dates).
//!
//! # Dividend Types
//!
//! - **Cash Dividends**: Fixed dollar amount per share
//! - **Proportional Dividends**: Percentage of spot price
//!
//! # Implementation
//!
//! At ex-dividend times, the spot price jumps down by the dividend amount:
//! - Cash: S → S - D
//! - Proportional: S → S * (1 - d)
//!
//! Between dividends, evolution follows standard GBM.

use super::super::traits::StochasticProcess;
use super::gbm::GbmParams;

/// Dividend payment specification.
#[derive(Debug, Clone, PartialEq)]
pub enum Dividend {
    /// Cash dividend: fixed dollar amount
    Cash(f64),
    /// Proportional dividend: percentage of spot (e.g., 0.02 for 2%)
    Proportional(f64),
}

impl Dividend {
    /// Compute dividend amount given current spot price.
    pub fn amount(&self, spot: f64) -> f64 {
        match self {
            Dividend::Cash(d) => *d,
            Dividend::Proportional(pct) => spot * pct,
        }
    }

    /// Apply dividend to spot price (returns adjusted spot).
    pub fn apply(&self, spot: f64) -> f64 {
        match self {
            Dividend::Cash(d) => (spot - d).max(0.0),
            Dividend::Proportional(pct) => spot * (1.0 - pct).max(0.0),
        }
    }
}

/// GBM process with discrete dividends.
#[derive(Debug, Clone)]
pub struct GbmWithDividends {
    /// Base GBM parameters (note: q should typically be 0 since dividends are explicit)
    params: GbmParams,
    /// Dividend schedule: (time, dividend)
    /// Must be sorted by time in ascending order
    dividends: Vec<(f64, Dividend)>,
}

impl GbmWithDividends {
    /// Create a new GBM process with discrete dividends.
    ///
    /// # Arguments
    ///
    /// * `params` - Base GBM parameters (set q=0 if all dividends are discrete)
    /// * `dividends` - Dividend schedule (time, dividend)
    ///
    /// # Errors
    ///
    /// Returns an error if any dividend time is non-finite, or two dividends
    /// share the same time after sorting.
    pub fn new(
        params: GbmParams,
        mut dividends: Vec<(f64, Dividend)>,
    ) -> finstack_quant_core::Result<Self> {
        for (i, (t, _)) in dividends.iter().enumerate() {
            if !t.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Dividend time at index {i} must be finite, got {t}"
                )));
            }
        }

        // Sort dividends by time using total_cmp for safe float comparison
        dividends.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Validate strictly increasing order
        for i in 1..dividends.len() {
            if dividends[i].0 <= dividends[i - 1].0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Dividend times must be strictly increasing; \
                     dividend[{}].time = {} <= dividend[{}].time = {}",
                    i,
                    dividends[i].0,
                    i - 1,
                    dividends[i - 1].0
                )));
            }
        }

        Ok(Self { params, dividends })
    }

    /// Create with explicit parameters and dividends.
    ///
    /// # Errors
    ///
    /// Returns an error if any parameter is invalid (see [`GbmParams::new`]) or
    /// if dividend times are non-finite or not strictly increasing.
    pub fn with_params(
        r: f64,
        q: f64,
        sigma: f64,
        dividends: Vec<(f64, Dividend)>,
    ) -> finstack_quant_core::Result<Self> {
        Self::new(GbmParams::new(r, q, sigma)?, dividends)
    }

    /// Get the base GBM parameters.
    pub fn params(&self) -> &GbmParams {
        &self.params
    }

    /// Get the dividend schedule.
    pub fn dividends(&self) -> &[(f64, Dividend)] {
        &self.dividends
    }

    /// Find dividends that occur in the time interval (t, t+dt].
    ///
    /// Returns vector of (dividend_time, dividend) for dividends in the interval.
    pub fn dividends_in_interval(&self, t: f64, dt: f64) -> Vec<(f64, &Dividend)> {
        let t_end = t + dt;
        self.dividends
            .iter()
            .filter(|(div_time, _)| *div_time > t && *div_time <= t_end)
            .map(|(div_time, div)| (*div_time, div))
            .collect()
    }

    /// Apply all dividends in an interval to the spot price.
    ///
    /// Dividends are applied in chronological order.
    pub fn apply_dividends(&self, mut spot: f64, t: f64, dt: f64) -> f64 {
        let divs = self.dividends_in_interval(t, dt);
        for (_, div) in divs {
            spot = div.apply(spot);
        }
        spot
    }
}

impl StochasticProcess for GbmWithDividends {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        // Each `[t, t+Δt]` step splits into at most `dividends_in_interval + 1` exact GBM
        // sub-intervals; independent Brownian increments per sub-interval require
        // one standard normal per segment (see `ExactGbmWithDividends`).
        self.dividends.len().saturating_add(1)
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // Drift is same as standard GBM between dividend dates
        // μ(S) = (r - q) S
        out[0] = (self.params.r - self.params.q) * x[0];
    }

    fn diffusion(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // Diffusion is same as standard GBM
        // σ(S) = σ S
        out[0] = self.params.sigma * x[0];
    }

    /// Only [`ExactGbmWithDividends`](crate::discretization::exact_gbm_dividends::ExactGbmWithDividends)
    /// applies the discrete dividend jumps; any generic scheme would silently
    /// simulate plain GBM and bias the spot high by the PV of the dividend
    /// stream. (This is also why the type deliberately does not implement
    /// `ProportionalDiffusion`: Milstein-family schemes must not accept it.)
    fn dedicated_scheme(&self) -> Option<&'static str> {
        Some("exact_gbm_with_dividends")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::StochasticProcess;

    #[test]
    fn test_num_factors_one_per_gbm_subinterval_budget() {
        let empty = GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), vec![]).unwrap();
        assert_eq!(empty.num_factors(), 1);

        let one_div = GbmWithDividends::new(
            GbmParams::new(0.05, 0.0, 0.2).unwrap(),
            vec![(0.5, Dividend::Cash(1.0))],
        )
        .unwrap();
        assert_eq!(one_div.num_factors(), 2);

        let two_div = GbmWithDividends::new(
            GbmParams::new(0.05, 0.0, 0.2).unwrap(),
            vec![(0.25, Dividend::Cash(0.5)), (0.75, Dividend::Cash(0.5))],
        )
        .unwrap();
        assert_eq!(two_div.num_factors(), 3);
    }

    #[test]
    fn test_cash_dividend() {
        let div = Dividend::Cash(1.0);
        assert_eq!(div.amount(100.0), 1.0);
        assert_eq!(div.amount(50.0), 1.0); // Fixed amount

        assert_eq!(div.apply(100.0), 99.0);
        assert_eq!(div.apply(50.0), 49.0);
    }

    #[test]
    fn test_proportional_dividend() {
        let div = Dividend::Proportional(0.02); // 2%
        assert_eq!(div.amount(100.0), 2.0);
        assert_eq!(div.amount(50.0), 1.0); // Scales with spot

        assert_eq!(div.apply(100.0), 98.0);
        assert_eq!(div.apply(50.0), 49.0);
    }

    #[test]
    fn test_dividend_prevents_negative_spot() {
        let div = Dividend::Cash(150.0);
        assert_eq!(div.apply(100.0), 0.0); // Can't go negative

        let div_pct = Dividend::Proportional(1.5); // 150%
        assert_eq!(div_pct.apply(100.0), 0.0);
    }

    #[test]
    fn test_gbm_with_dividends_creation() {
        let dividends = vec![(0.25, Dividend::Cash(0.50)), (0.50, Dividend::Cash(0.50))];

        let gbm_div =
            GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), dividends).unwrap();

        assert_eq!(gbm_div.dividends().len(), 2);
        assert_eq!(gbm_div.dim(), 1);
    }

    #[test]
    fn test_dividends_in_interval() {
        let dividends = vec![
            (0.25, Dividend::Cash(0.50)),
            (0.50, Dividend::Cash(0.75)),
            (0.75, Dividend::Cash(1.00)),
        ];

        let gbm_div =
            GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), dividends).unwrap();

        // Interval (0.2, 0.6] should capture dividends at 0.25 and 0.50
        let divs = gbm_div.dividends_in_interval(0.2, 0.4);
        assert_eq!(divs.len(), 2);
        assert_eq!(divs[0].0, 0.25);
        assert_eq!(divs[1].0, 0.50);

        // Interval (0.5, 0.8] should capture dividend at 0.75
        let divs = gbm_div.dividends_in_interval(0.5, 0.3);
        assert_eq!(divs.len(), 1);
        assert_eq!(divs[0].0, 0.75);

        // Interval (0.8, 1.0] should be empty
        let divs = gbm_div.dividends_in_interval(0.8, 0.2);
        assert_eq!(divs.len(), 0);
    }

    #[test]
    fn test_apply_dividends() {
        let dividends = vec![
            (0.25, Dividend::Cash(1.0)),
            (0.50, Dividend::Proportional(0.02)), // 2%
        ];

        let gbm_div =
            GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), dividends).unwrap();

        let spot = 100.0;

        // Apply dividends in (0.0, 0.6]
        let adjusted = gbm_div.apply_dividends(spot, 0.0, 0.6);

        // First: S = 100 - 1 = 99
        // Then: S = 99 * 0.98 = 97.02
        assert!((adjusted - 97.02).abs() < 1e-10);
    }

    #[test]
    fn test_dividend_sorting() {
        // Provide unsorted dividends
        let dividends = vec![
            (0.75, Dividend::Cash(1.0)),
            (0.25, Dividend::Cash(0.5)),
            (0.50, Dividend::Cash(0.75)),
        ];

        let gbm_div =
            GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), dividends).unwrap();

        // Should be sorted
        assert_eq!(gbm_div.dividends()[0].0, 0.25);
        assert_eq!(gbm_div.dividends()[1].0, 0.50);
        assert_eq!(gbm_div.dividends()[2].0, 0.75);
    }

    #[test]
    fn test_duplicate_dividend_times_returns_error() {
        let dividends = vec![
            (0.25, Dividend::Cash(0.5)),
            (0.25, Dividend::Cash(0.5)), // Duplicate time
        ];

        let result = GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), dividends);
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("strictly increasing"));
    }

    #[test]
    fn test_non_finite_dividend_time_returns_error() {
        let dividends = vec![(f64::NAN, Dividend::Cash(0.5))];
        let result = GbmWithDividends::new(GbmParams::new(0.05, 0.0, 0.2).unwrap(), dividends);
        assert!(result.is_err());
    }
}
