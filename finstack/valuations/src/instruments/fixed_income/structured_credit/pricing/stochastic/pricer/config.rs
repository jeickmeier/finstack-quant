//! Stochastic pricer configuration.

use finstack_core::dates::Date;
use finstack_core::market_data::term_structures::DiscountCurve;

use crate::instruments::fixed_income::structured_credit::pricing::stochastic::default::PoolGranularity;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;
use std::sync::Arc;

/// Pricing mode selection.
///
/// All three modes are first-class supported pricers. Test coverage:
/// - **Tree**: exercised by `tests/instruments/structured_credit/unit/{stochastic_pricing_tests,stochastic_tranche_pv_tests}` and `tests/support/structured_credit_stochastic.rs`. Default mode.
/// - **MonteCarlo**: exercised by the same test suites and the convergence tests.
/// - **Hybrid**: exercised by structured-credit pricer integration tests.
///
/// Choose based on horizon × dimensionality: `Tree` for short-horizon
/// non-recombining stochastic CLO/RMBS deals (deterministic, low variance),
/// `MonteCarlo` for long-horizon or high-dimensional pools, `Hybrid` to
/// front-load tree precision and tail with MC.
#[derive(
    Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum PricingMode {
    /// Tree-based pricing (exact, non-recombining)
    #[default]
    Tree,
    /// Monte Carlo pricing with specified number of paths
    MonteCarlo {
        /// Number of simulation paths
        num_paths: usize,
        /// Use antithetic variates for variance reduction
        antithetic: bool,
    },
    /// Hybrid: tree for short horizons, MC for long
    Hybrid {
        /// Tree periods before switching to MC
        tree_periods: usize,
        /// MC paths for tail
        mc_paths: usize,
    },
}

impl PricingMode {
    /// Create tree pricing mode.
    pub fn tree() -> Self {
        PricingMode::Tree
    }

    /// Create Monte Carlo pricing mode.
    pub fn monte_carlo(num_paths: usize) -> Self {
        PricingMode::MonteCarlo {
            num_paths: num_paths.max(100),
            antithetic: true,
        }
    }

    /// Create hybrid pricing mode.
    pub fn hybrid(tree_periods: usize, mc_paths: usize) -> Self {
        PricingMode::Hybrid {
            tree_periods: tree_periods.max(6),
            mc_paths: mc_paths.max(100),
        }
    }
}

/// Configuration for stochastic pricer.
#[derive(Clone)]
pub(crate) struct StochasticPricerConfig {
    /// Valuation date
    pub valuation_date: Date,

    /// Discount curve for present value calculations
    pub discount_curve: Arc<DiscountCurve>,

    /// Pricing mode (tree, MC, or hybrid)
    pub pricing_mode: PricingMode,

    /// Scenario tree configuration
    pub tree_config: ScenarioTreeConfig,

    /// Whether to compute risk metrics (EL, UL, ES)
    pub compute_risk_metrics: bool,

    /// Expected Shortfall confidence level (e.g., 0.95 for 95% ES)
    pub es_confidence: f64,

    /// Random seed for Monte Carlo
    pub seed: u64,

    /// Maximum terminal paths allowed for explicit path-preserving tree mode.
    pub max_tree_paths: usize,

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
            pricing_mode: PricingMode::default(),
            tree_config,
            compute_risk_metrics: true,
            es_confidence: 0.95,
            seed: 42,
            max_tree_paths: 100_000,
            pool_granularity: PoolGranularity::default(),
        }
    }

    /// Select the pool-granularity policy for copula-based default models.
    ///
    /// Defaults to [`PoolGranularity::PerName`]; pass
    /// [`PoolGranularity::LargeHomogeneous`] to opt into the closed-form LHP
    /// fast-path for genuinely granular pools.
    #[allow(dead_code)] // builder for the LHP fast-path; exercised by tests.
    pub(crate) fn with_pool_granularity(mut self, granularity: PoolGranularity) -> Self {
        self.pool_granularity = granularity;
        self
    }

    /// Set pricing mode.
    pub(crate) fn with_pricing_mode(mut self, mode: PricingMode) -> Self {
        self.pricing_mode = mode;
        self
    }
}

impl std::fmt::Debug for StochasticPricerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StochasticPricerConfig")
            .field("valuation_date", &self.valuation_date)
            .field("pricing_mode", &self.pricing_mode)
            .field("compute_risk_metrics", &self.compute_risk_metrics)
            .field("es_confidence", &self.es_confidence)
            .field("max_tree_paths", &self.max_tree_paths)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
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

    #[test]
    fn test_pricing_mode_default() {
        let mode = PricingMode::default();
        assert!(matches!(mode, PricingMode::Tree));
    }

    #[test]
    fn test_config_creation() {
        let today = test_date();
        let curve = test_discount_curve();
        let tree_config = ScenarioTreeConfig::new(
            12,
            1.0,
            crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::BranchingSpec::fixed(
                3,
            ),
        );

        let config = StochasticPricerConfig::new(today, curve, tree_config);

        assert_eq!(config.valuation_date, today);
        assert!(matches!(config.pricing_mode, PricingMode::Tree));
        assert!(config.compute_risk_metrics);
    }

    #[test]
    fn test_builder_pattern() {
        let today = test_date();
        let curve = test_discount_curve();
        let tree_config = ScenarioTreeConfig::new(
            12,
            1.0,
            crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::BranchingSpec::fixed(
                3,
            ),
        );

        let config = StochasticPricerConfig::new(today, curve, tree_config)
            .with_pricing_mode(PricingMode::monte_carlo(10000));

        assert!(matches!(
            config.pricing_mode,
            PricingMode::MonteCarlo { .. }
        ));
    }
}
