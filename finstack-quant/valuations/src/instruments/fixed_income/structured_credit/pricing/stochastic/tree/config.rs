//! Scenario tree configuration.
//!
//! Defines the parameters for scenario tree generation including
//! time horizon, branching, and model specifications.

#![allow(dead_code)]

use super::super::{
    correlation::CorrelationStructure, default::StochasticDefaultSpec,
    prepayment::StochasticPrepaySpec,
};
use crate::correlation::factor_model::LatentFactorSpec;
use crate::correlation::recovery::RecoverySpec;

const MAX_NODE_CAPACITY: usize = 50_000_000;

/// Branching specification for scenario tree generation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", deny_unknown_fields)]
#[non_exhaustive]
pub(crate) enum BranchingSpec {
    /// Fixed branching factor at each node.
    ///
    /// Each non-terminal node has exactly `branches` children.
    /// Total nodes = 1 + branches + branches² + ... + branches^periods
    Fixed {
        /// Number of branches at each node
        branches: usize,
    },

    /// Adaptive branching based on variance.
    ///
    /// Branches are added where uncertainty is highest.
    Adaptive {
        /// Minimum branches per node
        min: usize,
        /// Maximum branches per node
        max: usize,
        /// Variance threshold for adding branches
        variance_threshold: f64,
    },

    /// Sparse branching for large trees.
    ///
    /// Uses stratified sampling to reduce tree size.
    Stratified {
        /// Total number of terminal paths
        num_paths: usize,
        /// Resampling method ("antithetic", "importance", "latin_hypercube")
        method: String,
    },
}

impl Default for BranchingSpec {
    fn default() -> Self {
        BranchingSpec::Fixed { branches: 3 }
    }
}

impl BranchingSpec {
    /// Create fixed branching specification.
    #[allow(clippy::expect_used)] // Builder with valid inputs should not fail
    pub(crate) fn fixed(branches: usize) -> Self {
        BranchingSpec::Fixed {
            branches: branches.max(2),
        }
    }

    /// Create adaptive branching specification.
    pub(crate) fn adaptive(min: usize, max: usize, variance_threshold: f64) -> Self {
        BranchingSpec::Adaptive {
            min: min.max(2),
            max: max.max(min),
            variance_threshold: variance_threshold.clamp(0.0, 1.0),
        }
    }

    /// Create stratified branching specification.
    pub(crate) fn stratified(num_paths: usize) -> Self {
        BranchingSpec::Stratified {
            num_paths: num_paths.max(100),
            method: "antithetic".to_string(),
        }
    }

    /// Get the number of branches for a given node.
    ///
    /// For fixed branching, always returns the fixed number.
    /// For adaptive/stratified, returns the base or calculated number.
    pub(crate) fn branches_at_node(&self, variance: f64) -> usize {
        match self {
            BranchingSpec::Fixed { branches } => *branches,
            BranchingSpec::Adaptive {
                min,
                max,
                variance_threshold,
            } => {
                if variance > *variance_threshold {
                    *max
                } else {
                    *min
                }
            }
            BranchingSpec::Stratified { num_paths, .. } => *num_paths,
        }
    }

    /// Estimate total number of terminal nodes.
    pub(crate) fn estimate_terminal_nodes(&self, num_periods: usize) -> usize {
        match self {
            BranchingSpec::Fixed { branches } => {
                saturating_pow(*branches, num_periods).min(MAX_NODE_CAPACITY)
            }
            BranchingSpec::Adaptive { max, .. } => {
                saturating_pow(*max, num_periods).min(MAX_NODE_CAPACITY)
            }
            BranchingSpec::Stratified { num_paths, .. } => (*num_paths).min(MAX_NODE_CAPACITY),
        }
    }
}

/// Configuration for scenario tree generation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct ScenarioTreeConfig {
    /// Number of time periods (typically monthly)
    pub num_periods: usize,

    /// Horizon in years
    pub horizon_years: f64,

    /// Branching specification
    pub branching: BranchingSpec,

    /// Factor model specification
    pub factor_spec: LatentFactorSpec,

    /// Stochastic prepayment model specification
    pub prepay_spec: StochasticPrepaySpec,

    /// Stochastic default model specification
    pub default_spec: StochasticDefaultSpec,

    /// Recovery model specification
    pub recovery_spec: RecoverySpec,

    /// Correlation structure
    pub correlation: CorrelationStructure,

    /// Random seed for reproducibility
    pub seed: u64,

    /// Initial pool balance
    pub initial_balance: f64,

    /// Initial pool seasoning (months)
    pub initial_seasoning: u32,

    /// AssetPool coupon rate (the collateral WAC).
    pub pool_coupon: f64,

    /// Asset correlation from an explicit deal [`CorrelationStructure`].
    ///
    /// When `Some`, takes precedence over the copula spec's scalar. `None`
    /// keeps the correlation on `StochasticDefaultSpec::Copula`.
    pub asset_correlation_override: Option<f64>,

    /// Prevailing market refinancing rate for the Richard-Roll incentive
    /// `pool_coupon − market_rate`, sourced from
    /// `StructuredCredit::market_conditions.refi_rate`.
    pub market_refi_rate: f64,
}

impl ScenarioTreeConfig {
    /// Create a new scenario tree configuration.
    pub(crate) fn new(num_periods: usize, horizon_years: f64, branching: BranchingSpec) -> Self {
        Self {
            num_periods: num_periods.max(1),
            horizon_years: horizon_years.max(0.1),
            branching,
            factor_spec: LatentFactorSpec::default(),
            prepay_spec: StochasticPrepaySpec::default(),
            default_spec: StochasticDefaultSpec::default(),
            recovery_spec: RecoverySpec::default(),
            correlation: CorrelationStructure::default(),
            seed: 42,
            initial_balance: 1_000_000.0,
            initial_seasoning: 0,
            pool_coupon: 0.05,
            market_refi_rate: 0.045,
            asset_correlation_override: None,
        }
    }

    /// Create RMBS-standard configuration.
    pub(crate) fn rmbs_standard(horizon_years: f64, pool_coupon: f64) -> Self {
        let num_periods = (horizon_years * 12.0).ceil() as usize;
        Self {
            num_periods,
            horizon_years,
            branching: BranchingSpec::fixed(3),
            factor_spec: LatentFactorSpec::two_factor(0.20, 0.25, -0.30),
            prepay_spec: StochasticPrepaySpec::rmbs_agency(pool_coupon),
            default_spec: StochasticDefaultSpec::rmbs_standard(),
            recovery_spec: RecoverySpec::market_standard_stochastic(),
            correlation: CorrelationStructure::rmbs_standard(),
            seed: 42,
            initial_balance: 1_000_000.0,
            initial_seasoning: 0,
            pool_coupon,
            market_refi_rate: 0.045,
            asset_correlation_override: None,
        }
    }

    /// Create CLO-standard configuration.
    pub(crate) fn clo_standard(horizon_years: f64) -> Self {
        let num_periods = (horizon_years * 12.0).ceil() as usize;
        Self {
            num_periods,
            horizon_years,
            branching: BranchingSpec::fixed(3),
            factor_spec: LatentFactorSpec::two_factor(0.15, 0.30, -0.20),
            prepay_spec: StochasticPrepaySpec::clo_standard(),
            default_spec: StochasticDefaultSpec::clo_standard(),
            recovery_spec: RecoverySpec::MarketCorrelated {
                mean_recovery: 0.40,
                recovery_volatility: 0.30,
                // Canonical low-factor-stress convention: positive
                // correlation makes recovery fall when the factor falls.
                factor_correlation: 0.50,
            },
            correlation: CorrelationStructure::clo_standard(),
            seed: 42,
            initial_balance: 1_000_000.0,
            initial_seasoning: 0,
            pool_coupon: 0.06,
            market_refi_rate: 0.045,
            asset_correlation_override: None,
        }
    }

    /// Set the random seed.
    pub(crate) fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Set the initial pool balance.
    pub(crate) fn with_initial_balance(mut self, balance: f64) -> Self {
        self.initial_balance = balance;
        self
    }

    /// Set the initial seasoning.
    pub(crate) fn with_initial_seasoning(mut self, months: u32) -> Self {
        self.initial_seasoning = months;
        self
    }

    /// Set the pool coupon.
    pub(crate) fn with_pool_coupon(mut self, coupon: f64) -> Self {
        self.pool_coupon = coupon;
        self
    }

    /// Set the factor specification.
    pub(crate) fn with_factor_spec(mut self, spec: LatentFactorSpec) -> Self {
        self.factor_spec = spec;
        self
    }

    /// Set the prepayment specification.
    pub(crate) fn with_prepay_spec(mut self, spec: StochasticPrepaySpec) -> Self {
        self.prepay_spec = spec;
        self
    }

    /// Set the default specification.
    pub(crate) fn with_default_spec(mut self, spec: StochasticDefaultSpec) -> Self {
        self.default_spec = spec;
        self
    }

    /// Set the correlation structure.
    pub(crate) fn with_correlation(mut self, corr: CorrelationStructure) -> Self {
        self.correlation = corr;
        self
    }

    /// Get the time step size in years.
    pub(crate) fn dt(&self) -> f64 {
        self.horizon_years / self.num_periods as f64
    }

    /// Estimate total number of nodes in the tree.
    pub(crate) fn estimate_total_nodes(&self) -> usize {
        let levels = self.num_periods.saturating_add(1);
        levels
            .saturating_mul(levels)
            .min(MAX_NODE_CAPACITY)
            .saturating_add(levels.saturating_mul(2))
            .min(MAX_NODE_CAPACITY)
    }
}

fn saturating_pow(base: usize, exp: usize) -> usize {
    if base == 0 {
        return 0;
    }
    let mut result = 1usize;
    for _ in 0..exp {
        match result.checked_mul(base) {
            Some(value) => {
                result = value;
                if result >= MAX_NODE_CAPACITY {
                    return MAX_NODE_CAPACITY;
                }
            }
            None => return MAX_NODE_CAPACITY,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branching_spec_fixed() {
        let spec = BranchingSpec::fixed(3);
        assert_eq!(spec.branches_at_node(0.5), 3);

        // 3^5 = 243 terminal nodes
        assert_eq!(spec.estimate_terminal_nodes(5), 243);
    }

    #[test]
    fn test_branching_spec_adaptive() {
        let spec = BranchingSpec::adaptive(2, 5, 0.3);

        // Low variance: minimum branches
        assert_eq!(spec.branches_at_node(0.1), 2);

        // High variance: maximum branches
        assert_eq!(spec.branches_at_node(0.5), 5);
    }

    #[test]
    fn test_branching_spec_stratified_uses_terminal_path_count() {
        let spec = BranchingSpec::stratified(250);

        assert_eq!(spec.branches_at_node(0.5), 250);
        assert_eq!(spec.estimate_terminal_nodes(12), 250);
    }

    #[test]
    fn test_config_creation() {
        let config = ScenarioTreeConfig::new(60, 5.0, BranchingSpec::fixed(3));

        assert_eq!(config.num_periods, 60);
        assert!((config.horizon_years - 5.0).abs() < 1e-10);
        assert!((config.dt() - 5.0 / 60.0).abs() < 1e-10);
    }

    #[test]
    fn test_rmbs_standard_config() {
        let config = ScenarioTreeConfig::rmbs_standard(5.0, 0.045);

        assert_eq!(config.num_periods, 60);
        assert!((config.pool_coupon - 0.045).abs() < 1e-10);
        assert!(config.prepay_spec.is_stochastic());
        assert!(config.default_spec.is_stochastic());
    }

    #[test]
    fn test_clo_standard_config() {
        let config = ScenarioTreeConfig::clo_standard(7.0);

        assert_eq!(config.num_periods, 84);
        assert!(config.correlation.is_sectored());
    }

    #[test]
    fn test_estimate_nodes() {
        let config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(3));

        // Recombining lattice => (n + 1)^2 with buffer
        assert_eq!(config.estimate_total_nodes(), 24);
    }

    #[test]
    fn test_builder_pattern() {
        let config = ScenarioTreeConfig::new(60, 5.0, BranchingSpec::fixed(3))
            .with_seed(12345)
            .with_initial_balance(5_000_000.0)
            .with_initial_seasoning(24);

        assert_eq!(config.seed, 12345);
        assert!((config.initial_balance - 5_000_000.0).abs() < 1e-10);
        assert_eq!(config.initial_seasoning, 24);
    }
}
