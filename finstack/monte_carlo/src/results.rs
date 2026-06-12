//! Pricing-side Monte Carlo result types with explicit currency tags.
//!
//! This module wraps the numeric estimates from [`crate::estimate`] into
//! currency-aware result types for pricing APIs. All amounts here refer to
//! discounted path values unless a field explicitly says otherwise.

use crate::paths::PathDataset;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use serde::{Deserialize, Serialize};

/// Discounted Monte Carlo estimate tagged with a currency.
///
/// The engine computes these values from discounted path outcomes. `mean` and
/// `ci_95` are stored as [`Money`], while the auxiliary statistics remain raw
/// `f64` values in the same currency unit as `mean.amount()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyEstimate {
    /// Discounted mean present value.
    pub mean: Money,
    /// Standard error of the discounted mean, in `mean.amount()` units.
    pub stderr: f64,
    /// 95% confidence interval for the discounted mean present value.
    pub ci_95: (Money, Money),
    /// Number of independent path estimators contributing to the estimate.
    ///
    /// See [`crate::estimate::Estimate::num_paths`] for the full semantics,
    /// including how antithetic variates split simulated work across
    /// estimators.
    pub num_paths: usize,
    /// Total number of simulated sample paths driving the estimator.
    ///
    /// See [`crate::estimate::Estimate::num_simulated_paths`]. Equal to
    /// `num_paths` without variance reduction, or `2 * num_paths` with
    /// antithetic variates.
    #[serde(default)]
    pub num_simulated_paths: usize,
    /// Optional sample standard deviation of discounted path values.
    #[serde(default)]
    pub std_dev: Option<f64>,
    /// Optional median of captured discounted path values.
    ///
    /// This is populated only when captured-path diagnostics are available.
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
    /// censoring paths.
    #[serde(default)]
    pub num_skipped: usize,
}

impl MoneyEstimate {
    /// Convert a numeric estimate into a currency-aware pricing estimate.
    ///
    /// # Arguments
    ///
    /// * `estimate` - Numeric Monte Carlo estimate on discounted path values.
    /// * `currency` - Currency tag to attach to `mean` and `ci_95`.
    ///
    /// # Returns
    ///
    /// A [`MoneyEstimate`] whose raw `f64` diagnostics remain in the same
    /// currency unit as `mean.amount()`.
    pub fn from_estimate(estimate: crate::estimate::Estimate, currency: Currency) -> Self {
        Self {
            mean: Money::new(estimate.mean, currency),
            stderr: estimate.stderr,
            ci_95: (
                Money::new(estimate.ci_95.0, currency),
                Money::new(estimate.ci_95.1, currency),
            ),
            num_paths: estimate.num_paths,
            num_simulated_paths: estimate.num_simulated_paths,
            std_dev: estimate.std_dev,
            median: estimate.median,
            percentile_25: estimate.percentile_25,
            percentile_75: estimate.percentile_75,
            min: estimate.min,
            max: estimate.max,
            num_skipped: estimate.num_skipped,
        }
    }

    /// Return `stderr / abs(mean.amount())`.
    ///
    /// Returns `f64::INFINITY` when the estimate is numerically close to zero.
    pub fn relative_stderr(&self) -> f64 {
        if self.mean.amount().abs() < 1e-10 {
            f64::INFINITY
        } else {
            self.stderr.abs() / self.mean.amount().abs()
        }
    }
}

impl std::fmt::Display for MoneyEstimate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ± {:.6} [{}, {}] (n={})",
            self.mean, self.stderr, self.ci_95.0, self.ci_95.1, self.num_paths
        )
    }
}

/// Reproducibility metadata stamped on a Monte Carlo pricing run.
///
/// Records the execution policy that produced an estimate so results are
/// auditable and replayable: the same `(seed, num_paths, num_steps,
/// chunk_size)` reproduces the run bit-for-bit regardless of thread count or
/// host (see the determinism notes on [`crate::engine::McEngine::price`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunMetadata {
    /// Root RNG seed, when the calling pricer derived the stream from one.
    ///
    /// `None` when the engine was driven by an externally constructed
    /// [`crate::traits::RandomStream`] whose seed the engine cannot observe.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Whether the run used the parallel execution path.
    #[serde(default)]
    pub use_parallel: bool,
    /// Whether antithetic variates were enabled.
    #[serde(default)]
    pub antithetic: bool,
    /// Effective chunk size of the deterministic reduction tree.
    #[serde(default)]
    pub chunk_size: usize,
    /// Number of time-grid steps.
    #[serde(default)]
    pub num_steps: usize,
}

/// Monte Carlo pricing result with optional captured paths.
///
/// The estimate always reflects all simulated discounted path values used by the
/// pricing run. When `paths` is present, it contains the captured subset only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResult {
    /// Discounted pricing estimate for the full simulation.
    pub estimate: MoneyEstimate,
    /// Optional captured-path subset for diagnostics and visualization.
    pub paths: Option<PathDataset>,
    /// Reproducibility metadata for the run (execution policy and seed).
    #[serde(default)]
    pub run: Option<RunMetadata>,
}

impl MonteCarloResult {
    /// Create a result that only contains the aggregate estimate.
    pub fn new(estimate: MoneyEstimate) -> Self {
        Self {
            estimate,
            paths: None,
            run: None,
        }
    }

    /// Create a result that includes captured-path diagnostics.
    pub fn with_paths(estimate: MoneyEstimate, paths: PathDataset) -> Self {
        Self {
            estimate,
            paths: Some(paths),
            run: None,
        }
    }

    /// Attach reproducibility metadata for the run.
    #[must_use]
    pub fn with_run_metadata(mut self, run: RunMetadata) -> Self {
        self.run = Some(run);
        self
    }

    /// Return `true` when captured-path diagnostics are present.
    pub fn has_paths(&self) -> bool {
        self.paths.is_some()
    }

    /// Borrow the captured-path subset if available.
    pub fn paths(&self) -> Option<&PathDataset> {
        self.paths.as_ref()
    }

    /// Return the number of captured paths, or `0` when none were retained.
    pub fn num_captured_paths(&self) -> usize {
        self.paths.as_ref().map(|p| p.num_captured()).unwrap_or(0)
    }

    /// Borrow the aggregate discounted estimate.
    pub fn estimate(&self) -> &MoneyEstimate {
        &self.estimate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estimate::Estimate;

    #[test]
    fn test_from_estimate_preserves_optional_diagnostics() {
        let estimate = Estimate::new(100.0, 1.0, (98.0, 102.0), 10_000)
            .with_std_dev(10.0)
            .with_median(99.0)
            .with_percentiles(95.0, 105.0)
            .with_range(80.0, 120.0);

        let money_estimate = MoneyEstimate::from_estimate(estimate, Currency::USD);

        assert_eq!(money_estimate.std_dev, Some(10.0));
        assert_eq!(money_estimate.median, Some(99.0));
        assert_eq!(money_estimate.percentile_25, Some(95.0));
        assert_eq!(money_estimate.percentile_75, Some(105.0));
        assert_eq!(money_estimate.min, Some(80.0));
        assert_eq!(money_estimate.max, Some(120.0));
    }
}
