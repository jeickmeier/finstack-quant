//! Scenario configuration for stochastic structured-credit pricing.

use finstack_quant_models::correlation::latent_factor::LatentFactorSpec;
use finstack_quant_models::correlation::recovery::RecoverySpec;
use finstack_quant_models::credit::pool::{StochasticDefaultSpec, StochasticPrepaySpec};

/// Internal inputs shared by structured-credit setup and stochastic pricing.
pub(crate) struct ScenarioTreeConfig {
    /// Number of time periods (typically monthly); sizes the Monte Carlo
    /// horizon.
    pub num_periods: usize,

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

    /// Balance-weighted pool seasoning (loan age) at the valuation date, in
    /// months.
    pub seasoning_months: u32,

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
    /// Create a scenario configuration over `num_periods` periods (at least
    /// one) with default factor, prepayment, default and recovery specs.
    pub(crate) fn new(num_periods: usize) -> Self {
        Self {
            num_periods: num_periods.max(1),
            factor_spec: LatentFactorSpec::default(),
            prepay_spec: StochasticPrepaySpec::default(),
            default_spec: StochasticDefaultSpec::default(),
            recovery_spec: RecoverySpec::default(),
            seed: 42,
            seasoning_months: 0,
            market_refi_rate: 0.045,
            asset_correlation_override: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_periods_is_clamped_to_at_least_one() {
        assert_eq!(ScenarioTreeConfig::new(0).num_periods, 1);
        assert_eq!(ScenarioTreeConfig::new(5).num_periods, 5);
    }
}
