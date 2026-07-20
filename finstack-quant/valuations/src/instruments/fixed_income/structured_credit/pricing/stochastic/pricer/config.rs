//! Stochastic pricer configuration.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;

use crate::instruments::fixed_income::structured_credit::pricing::stochastic::default::PoolGranularity;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;
use std::sync::Arc;

/// Pricing mode selection.
///
/// Choose based on horizon × dimensionality: `Tree` for SHORT-horizon
/// non-recombining stochastic deals (deterministic, low variance),
/// `MonteCarlo` for long-horizon or high-dimensional pools, `Hybrid` to
/// front-load tree precision and tail with MC.
///
/// # Tree mode is bounded by construction — read this before selecting it
///
/// SC-M25: path-preserving tree pricing keeps `3^n` terminal nodes for `n`
/// periods, checked against `max_tree_paths` (default 100,000). `3^11 =
/// 177,147`, so **Tree hard-errors for any deal with more than ten periods
/// remaining** — which is essentially every real deal, since
/// `build_scenario_tree_config` sets `num_periods` to months-to-maturity.
///
/// This type previously documented Tree as "Default mode" and all three modes
/// as "first-class supported pricers". That was not true of Tree at any
/// realistic horizon, and it only escaped notice because the public
/// `price_stochastic` entry point overrides the default to Monte Carlo before
/// the mode is ever used.
///
/// The default is therefore [`PricingMode::MonteCarlo`] — the mode that can
/// actually price the deals this module is built for. Tree remains available
/// and correct for genuinely short horizons; select it explicitly.
///
/// Test coverage:
/// - **Tree**: `tests/instruments/structured_credit/unit/{stochastic_pricing_tests,stochastic_tranche_pv_tests}`, at horizons within the node bound.
/// - **MonteCarlo**: the same suites plus the convergence tests.
/// - **Hybrid**: structured-credit pricer integration tests.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum PricingMode {
    /// Tree-based pricing (exact, non-recombining).
    ///
    /// Bounded to roughly ten periods by the `3^n` node count — see the type
    /// docs. Not the default for that reason.
    Tree,
    /// Monte Carlo pricing with specified number of paths.
    ///
    /// The default, because it is the only mode that can price a deal at a
    /// realistic horizon.
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

impl Default for PricingMode {
    /// SC-M25: Monte Carlo, not Tree.
    ///
    /// Tree is bounded to roughly ten periods by its `3^n` node count, so it
    /// cannot price a deal at any realistic horizon. Defaulting to it made the
    /// type's own documentation wrong; the only reason nothing broke is that
    /// `price_stochastic` overrode the default before it was used.
    ///
    /// 10,000 paths with antithetic variates matches what
    /// `default_stochastic_pricing_mode` already selects, so this changes no
    /// behaviour on the public entry point — it makes the standalone default
    /// agree with it.
    fn default() -> Self {
        PricingMode::MonteCarlo {
            num_paths: 10_000,
            antithetic: true,
        }
    }
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
        // Adopt the scenario-tree seed (per deal / valuation date via
        // `derive_seed`) so portfolio Monte Carlo error diversifies.
        let seed = tree_config.seed;
        Self {
            valuation_date,
            discount_curve,
            pricing_mode: PricingMode::default(),
            tree_config,
            compute_risk_metrics: true,
            es_confidence: 0.95,
            seed,
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

    /// Pricer config adopts the scenario-tree seed (not a hardcoded constant).
    #[test]
    fn pricer_config_adopts_the_scenario_tree_seed() {
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::BranchingSpec;
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;

        let seeded =
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(3)).with_seed(987_654_321);
        let config = StochasticPricerConfig::new(test_date(), test_discount_curve(), seeded);
        assert_eq!(
            config.seed, 987_654_321,
            "pricer seed must come from the scenario-tree config"
        );

        // Two distinct tree seeds must yield two distinct pricer seeds.
        let a = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(3)).with_seed(1),
        );
        let b = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(3)).with_seed(2),
        );
        assert_ne!(
            a.seed, b.seed,
            "distinct tree seeds must produce distinct pricer seeds"
        );
    }

    /// SC-M25 — the default must be a mode that can actually price a deal.
    ///
    /// Tree keeps `3^n` terminal nodes against a 100,000 cap, so it hard-errors
    /// past ten periods — essentially every real deal. Defaulting to it made
    /// this type's own docs wrong; nothing broke only because
    /// `price_stochastic` overrode the default before it was used.
    #[test]
    fn test_pricing_mode_default_is_monte_carlo() {
        let mode = PricingMode::default();
        assert!(
            matches!(mode, PricingMode::MonteCarlo { .. }),
            "the default pricing mode must be Monte Carlo, not Tree — Tree \
             cannot price a deal at a realistic horizon (SC-M25)"
        );
        // And it must agree with what `default_stochastic_pricing_mode`
        // already selects, so the standalone default is not a second opinion.
        assert!(
            matches!(
                mode,
                PricingMode::MonteCarlo {
                    num_paths: 10_000,
                    antithetic: true
                }
            ),
            "the default must match the public entry point's choice"
        );
    }

    /// Tree remains selectable and correct within its node bound.
    #[test]
    fn tree_mode_is_still_available_explicitly() {
        assert!(matches!(PricingMode::tree(), PricingMode::Tree));
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
        assert!(matches!(
            config.pricing_mode,
            PricingMode::MonteCarlo { .. }
        ));
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
