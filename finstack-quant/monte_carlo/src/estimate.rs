//! Numeric Monte Carlo estimation results without currency tagging.
//!
//! [`Estimate`] is the engine's internal numeric summary for discounted path
//! values. Pricing-facing APIs usually convert it into
//! [`crate::results::MoneyEstimate`] once the output currency is known.

use serde::{Deserialize, Serialize};

/// Numeric Monte Carlo estimate for discounted path values.
///
/// All fields are unitless `f64` values in the same numeric unit as the
/// simulated discounted payoff. Pricing APIs usually wrap the mean and
/// confidence interval in [`finstack_quant_core::money::Money`] after choosing a
/// currency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Estimate {
    /// Mean of the discounted path values.
    pub mean: f64,
    /// Standard error of the discounted mean.
    pub stderr: f64,
    /// 95% confidence interval for the discounted mean.
    pub ci_95: (f64, f64),
    /// Number of independent path estimators contributing to the estimate.
    ///
    /// With variance reduction disabled (`antithetic = false`), this equals the
    /// number of simulated sample paths. With antithetic variates enabled each
    /// estimator is the mean of an antithetic pair, so
    /// `num_simulated_paths == 2 * num_paths`. The engine records both so
    /// downstream consumers can distinguish statistical sample size from
    /// simulation work.
    pub num_paths: usize,
    /// Total number of simulated sample paths driving the estimator.
    ///
    /// Equal to `num_paths` without variance reduction. When
    /// [`crate::engine::McEngineConfig::antithetic`] is enabled this is twice
    /// `num_paths` because each estimator averages an antithetic pair of
    /// paths. `#[serde(default)]` allows older payloads to round-trip with
    /// `num_simulated_paths == 0` which [`Self::new`] back-fills with
    /// `num_paths` for convenience.
    #[serde(default)]
    pub num_simulated_paths: usize,
    /// Optional sample standard deviation of discounted path values.
    pub std_dev: Option<f64>,
    /// Optional median of captured discounted path values.
    #[serde(default)]
    pub median: Option<f64>,
    /// Optional 25th percentile of captured discounted path values.
    #[serde(default)]
    pub percentile_25: Option<f64>,
    /// Optional 75th percentile of captured discounted path values.
    #[serde(default)]
    pub percentile_75: Option<f64>,
    /// Optional minimum of captured discounted path values.
    #[serde(default)]
    pub min: Option<f64>,
    /// Optional maximum of captured discounted path values.
    #[serde(default)]
    pub max: Option<f64>,
    /// Legacy count of skipped paths from older survivor-pricing runs.
    ///
    /// Current engine loops reject non-finite discounted payoffs instead of
    /// censoring paths, so new engine-created estimates should leave this at
    /// zero.
    #[serde(default)]
    pub num_skipped: usize,
}

impl Estimate {
    /// Create a new estimate from aggregate simulation statistics.
    ///
    /// # Arguments
    ///
    /// * `mean` - Discounted sample mean.
    /// * `stderr` - Standard error of the discounted mean.
    /// * `ci_95` - Lower and upper bounds of the 95% confidence interval.
    /// * `num_paths` - Number of simulated paths used to compute the estimate.
    ///
    /// # Returns
    ///
    /// An estimate without optional distribution diagnostics populated.
    pub fn new(mean: f64, stderr: f64, ci_95: (f64, f64), num_paths: usize) -> Self {
        Self {
            mean,
            stderr,
            ci_95,
            num_paths,
            num_simulated_paths: num_paths,
            std_dev: None,
            median: None,
            percentile_25: None,
            percentile_75: None,
            min: None,
            max: None,
            num_skipped: 0,
        }
    }

    /// Record the total number of simulated sample paths.
    ///
    /// Callers should set this to `2 * num_paths` when antithetic variates
    /// are enabled, and leave it at `num_paths` (the default) otherwise.
    pub fn with_num_simulated_paths(mut self, num_simulated_paths: usize) -> Self {
        self.num_simulated_paths = num_simulated_paths;
        self
    }

    /// Attach the sample standard deviation of discounted path values.
    pub fn with_std_dev(mut self, std_dev: f64) -> Self {
        self.std_dev = Some(std_dev);
        self
    }

    /// Attach the median of captured discounted path values.
    pub fn with_median(mut self, median: f64) -> Self {
        self.median = Some(median);
        self
    }

    /// Attach the 25th and 75th percentiles of captured discounted path values.
    pub fn with_percentiles(mut self, p25: f64, p75: f64) -> Self {
        self.percentile_25 = Some(p25);
        self.percentile_75 = Some(p75);
        self
    }

    /// Attach the minimum and maximum of captured discounted path values.
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Attach a legacy skipped-path count.
    ///
    /// Retained for backward-compatible deserialization/display of older
    /// survivor-pricing estimates.
    pub fn with_num_skipped(mut self, num_skipped: usize) -> Self {
        self.num_skipped = num_skipped;
        self
    }

    /// Return the interquartile range when captured percentiles are available.
    pub fn iqr(&self) -> Option<f64> {
        match (self.percentile_25, self.percentile_75) {
            (Some(p25), Some(p75)) => Some(p75 - p25),
            _ => None,
        }
    }

    /// Return `max - min` when captured extrema are available.
    pub fn range(&self) -> Option<f64> {
        match (self.min, self.max) {
            (Some(min), Some(max)) => Some(max - min),
            _ => None,
        }
    }

    /// Return `stderr / abs(mean)`.
    ///
    /// Returns `f64::INFINITY` when the estimate is numerically close to zero.
    pub fn relative_stderr(&self) -> f64 {
        if self.mean.abs() < 1e-10 {
            f64::INFINITY
        } else {
            self.stderr.abs() / self.mean.abs()
        }
    }

    /// Return the coefficient of variation `std_dev / abs(mean)` when available.
    pub fn cv(&self) -> Option<f64> {
        self.std_dev.map(|sd| {
            if self.mean.abs() < 1e-10 {
                f64::INFINITY
            } else {
                sd.abs() / self.mean.abs()
            }
        })
    }

    /// Return half the width of `ci_95`.
    pub fn ci_half_width(&self) -> f64 {
        (self.ci_95.1 - self.ci_95.0) / 2.0
    }
}

impl std::fmt::Display for Estimate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.6} ± {:.6} [{:.6}, {:.6}] (n={}",
            self.mean, self.stderr, self.ci_95.0, self.ci_95.1, self.num_paths
        )?;
        if self.num_skipped > 0 {
            write!(f, ", skipped={}", self.num_skipped)?;
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_creation() {
        let est = Estimate::new(100.0, 1.0, (98.0, 102.0), 10000);
        assert_eq!(est.mean, 100.0);
        assert_eq!(est.stderr, 1.0);
        assert_eq!(est.num_paths, 10000);
        assert_eq!(est.ci_half_width(), 2.0);
    }

    #[test]
    fn test_relative_stderr() {
        let est = Estimate::new(100.0, 1.0, (98.0, 102.0), 10000);
        assert_eq!(est.relative_stderr(), 0.01); // 1%
    }

    // MoneyEstimate tests moved with pricing module

    #[test]
    fn test_display() {
        let est = Estimate::new(100.0, 1.0, (98.0, 102.0), 10000);
        let s = format!("{}", est);
        assert!(s.contains("100."));
        assert!(s.contains("n=10000"));
    }

    // MonteCarloResult tests moved with pricing module
}
