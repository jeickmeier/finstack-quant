//! Scenario-tree configuration for stochastic structured-credit pricing.

use finstack_quant_models::correlation::latent_factor::LatentFactorSpec;
use finstack_quant_models::correlation::recovery::RecoverySpec;
use finstack_quant_models::credit::pool::{StochasticDefaultSpec, StochasticPrepaySpec};

const MAX_TERMINAL_PATHS: usize = 50_000_000;

/// Internal inputs shared by structured-credit setup and stochastic pricing.
pub(crate) struct ScenarioTreeConfig {
    /// Number of time periods (typically monthly).
    pub num_periods: usize,

    /// Fixed number of children per tree node.
    pub branch_count: usize,

    /// Factor model specification.
    pub factor_spec: LatentFactorSpec,

    /// Stochastic prepayment model specification.
    pub prepay_spec: StochasticPrepaySpec,

    /// Stochastic default model specification.
    pub default_spec: StochasticDefaultSpec,

    /// Recovery model specification.
    pub recovery_spec: RecoverySpec,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Initial pool seasoning in months.
    pub initial_seasoning: u32,

    /// Asset correlation from an explicit deal
    /// [`CorrelationStructure`](finstack_quant_models::credit::pool::CorrelationStructure).
    ///
    /// When `Some`, takes precedence over the copula spec's scalar. `None`
    /// keeps the correlation on `StochasticDefaultSpec::Copula`.
    pub asset_correlation_override: Option<f64>,

    /// Prevailing market refinancing rate for the Richard-Roll incentive,
    /// sourced from `StructuredCredit::market_conditions.refi_rate`.
    pub market_refi_rate: f64,
}

impl ScenarioTreeConfig {
    /// Create a fixed-branching scenario-tree configuration.
    pub(crate) fn new(num_periods: usize, branch_count: usize) -> Self {
        Self {
            num_periods: num_periods.max(1),
            branch_count: branch_count.max(2),
            factor_spec: LatentFactorSpec::default(),
            prepay_spec: StochasticPrepaySpec::default(),
            default_spec: StochasticDefaultSpec::default(),
            recovery_spec: RecoverySpec::default(),
            seed: 42,
            initial_seasoning: 0,
            market_refi_rate: 0.045,
            asset_correlation_override: None,
        }
    }

    /// Return the capped number of terminal paths for `num_periods`.
    pub(crate) fn terminal_path_count(&self, num_periods: usize) -> usize {
        capped_pow(self.branch_count, num_periods)
    }
}

fn capped_pow(base: usize, exp: usize) -> usize {
    if base == 0 {
        return 0;
    }
    let mut result = 1usize;
    for _ in 0..exp {
        match result.checked_mul(base) {
            Some(value) => {
                result = value;
                if result >= MAX_TERMINAL_PATHS {
                    return MAX_TERMINAL_PATHS;
                }
            }
            None => return MAX_TERMINAL_PATHS,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_branching_is_clamped_and_counted() {
        let config = ScenarioTreeConfig::new(5, 1);

        assert_eq!(config.branch_count, 2);
        assert_eq!(config.terminal_path_count(5), 32);
    }

    #[test]
    fn terminal_path_count_saturates_at_the_capacity_guard() {
        let config = ScenarioTreeConfig::new(usize::MAX, 3);

        assert_eq!(
            config.terminal_path_count(config.num_periods),
            MAX_TERMINAL_PATHS
        );
    }
}
