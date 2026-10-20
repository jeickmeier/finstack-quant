//! Stochastic pricer configuration and the scenario inputs it carries.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations_core::results::structured_credit::StructuredCreditPricingMode;

use finstack_quant_models::correlation::latent_factor::LatentFactorSpec;
use finstack_quant_models::correlation::recovery::RecoverySpec;
use finstack_quant_models::credit::pool::PoolGranularity;
use finstack_quant_models::credit::pool::{StochasticDefaultSpec, StochasticPrepaySpec};
use std::sync::Arc;

/// Configuration for stochastic pricer.
pub(crate) struct StochasticPricerConfig {
    /// Valuation date
    pub valuation_date: Date,

    /// Discount curve for present value calculations
    pub discount_curve: Arc<DiscountCurve>,

    /// Monte Carlo estimator count and antithetic pairing.
    pub pricing_mode: StructuredCreditPricingMode,

    /// Scenario configuration: horizon, factor/prepay/default/recovery specs
    /// and the path seed.
    pub tree_config: ScenarioTreeConfig,

    /// Expected Shortfall confidence level (e.g., 0.95 for 95% ES)
    pub es_confidence: f64,

    /// AssetPool-granularity policy for copula-based default models.
    ///
    /// [`PoolGranularity::PerName`] (the default) realizes each pool asset's
    /// default individually — the correct treatment for concentrated CLOs
    /// where name-level lumpiness dominates mezzanine/equity risk.
    /// [`PoolGranularity::LargeHomogeneous`] is an explicit opt-in fast-path
    /// that applies the closed-form large-homogeneous-pool limit, acceptable
    /// only for genuinely granular pools. Ignored by non-copula default
    /// models.
    pub pool_granularity: PoolGranularity,
}

impl StochasticPricerConfig {
    /// Create a new pricer configuration.
    pub(crate) fn new(
        valuation_date: Date,
        discount_curve: Arc<DiscountCurve>,
        tree_config: ScenarioTreeConfig,
    ) -> Self {
        Self {
            valuation_date,
            discount_curve,
            pricing_mode: StructuredCreditPricingMode::default(),
            tree_config,
            es_confidence: 0.95,
            pool_granularity: PoolGranularity::default(),
        }
    }

    /// Select the pool-granularity policy for copula-based default models.
    ///
    /// Defaults to [`PoolGranularity::PerName`]; pass
    /// [`PoolGranularity::LargeHomogeneous`] to opt into the closed-form LHP
    /// fast-path for genuinely granular pools.
    pub(crate) fn with_pool_granularity(mut self, granularity: PoolGranularity) -> Self {
        self.pool_granularity = granularity;
        self
    }

    /// Set pricing mode.
    pub(crate) fn with_pricing_mode(mut self, mode: StructuredCreditPricingMode) -> Self {
        self.pricing_mode = mode;
        self
    }
}

impl std::fmt::Debug for StochasticPricerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StochasticPricerConfig")
            .field("valuation_date", &self.valuation_date)
            .field("pricing_mode", &self.pricing_mode)
            .field("es_confidence", &self.es_confidence)
            .finish()
    }
}

// Scenario configuration for stochastic structured-credit pricing.

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
mod scenario_tree_tests {
    use super::*;

    #[test]
    fn num_periods_is_clamped_to_at_least_one() {
        assert_eq!(ScenarioTreeConfig::new(0).num_periods, 1);
        assert_eq!(ScenarioTreeConfig::new(5).num_periods, 5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use time::Month;

    fn test_discount_curve() -> Arc<DiscountCurve> {
        Arc::new(
            DiscountCurve::builder("USD-OIS")
                .base_date(Date::from_calendar_date(2024, Month::January, 15).expect("Valid date"))
                .knots([
                    (0.0, 1.0),
                    (0.5, 0.975),
                    (1.0, 0.95),
                    (2.0, 0.90),
                    (5.0, 0.78),
                ])
                .interp(InterpStyle::LogLinear)
                .build()
                .expect("Valid curve"),
        )
    }

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 15).expect("Valid date")
    }

    /// The standalone default must agree with what
    /// `default_stochastic_pricing_mode` selects, so it is not a second
    /// opinion on the estimator count.
    #[test]
    fn test_pricing_mode_default_is_monte_carlo() {
        assert_eq!(
            StructuredCreditPricingMode::default(),
            StructuredCreditPricingMode::MonteCarlo {
                num_paths: 5_000,
                antithetic: true
            },
            "the default must match the public entry point's choice"
        );
    }

    #[test]
    fn test_config_creation() {
        let today = test_date();
        let curve = test_discount_curve();
        let tree_config = ScenarioTreeConfig::new(12);

        let config = StochasticPricerConfig::new(today, curve, tree_config);

        assert_eq!(config.valuation_date, today);
        assert!(matches!(
            config.pricing_mode,
            StructuredCreditPricingMode::MonteCarlo { .. }
        ));
    }

    #[test]
    fn test_builder_pattern() {
        let today = test_date();
        let curve = test_discount_curve();
        let tree_config = ScenarioTreeConfig::new(12);

        let config = StochasticPricerConfig::new(today, curve, tree_config)
            .with_pricing_mode(StructuredCreditPricingMode::monte_carlo(5_000));

        assert!(matches!(
            config.pricing_mode,
            StructuredCreditPricingMode::MonteCarlo { .. }
        ));
    }
}
