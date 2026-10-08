//! Stochastic pricer configuration.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;

use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;
use finstack_quant_models::credit::pool::PoolGranularity;
use std::sync::Arc;

/// Stochastic pricing mode.
///
/// Monte Carlo is the only engine: each scenario path draws its monthly
/// systematic factors from a seeded Philox substream and runs the full deal
/// waterfall. The mode is echoed on
/// [`StochasticPricingResult::pricing_mode`](super::StochasticPricingResult)
/// so a result records the estimator count it was produced with.
///
/// Test coverage:
/// `tests/instruments/structured_credit/unit/{stochastic_pricing_tests,stochastic_tranche_pv_tests}`
/// plus the convergence tests.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum StructuredCreditPricingMode {
    /// Monte Carlo pricing with a specified number of independent estimators.
    MonteCarlo {
        /// Number of independent estimators; must be at least two to estimate
        /// sampling uncertainty. With `antithetic` each estimator averages a
        /// `(Z, -Z)` pair, so the engine prices `2 × num_paths` scenario paths.
        /// Sample standard error and the Student-t interval use this estimator
        /// count, with `num_paths - 1` degrees of freedom.
        #[cfg_attr(feature = "json-schema", schemars(range(min = 2)))]
        num_paths: usize,
        /// Pair each estimator's path with its sign-flipped mirror.
        antithetic: bool,
    },
}

impl Default for StructuredCreditPricingMode {
    /// 5,000 antithetic estimators (10,000 simulated paths), matching what
    /// `default_stochastic_pricing_mode` selects, so the standalone default
    /// agrees with the public `price_stochastic` entry point.
    fn default() -> Self {
        StructuredCreditPricingMode::MonteCarlo {
            num_paths: 5_000,
            antithetic: true,
        }
    }
}

impl StructuredCreditPricingMode {
    /// Create an antithetic Monte Carlo pricing mode.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Number of independent antithetic estimators, clamped to
    ///   at least 50 (100 simulated paths); the engine simulates
    ///   `2 × num_paths` scenario paths.
    pub fn monte_carlo(num_paths: usize) -> Self {
        StructuredCreditPricingMode::MonteCarlo {
            num_paths: num_paths.max(50),
            antithetic: true,
        }
    }
}

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
