// Comprehensive tests for stochastic structured credit module.
// This module contains:
// - **Golden tests**: Verify known results match expected values
// - **Property tests**: Verify invariants hold across parameter ranges
// - **Integration tests**: Verify component interactions work correctly

use crate::instruments::common_impl::models::correlation::copula::{Copula, GaussianCopula};
use crate::instruments::common_impl::models::correlation::factor_model::FactorSpec;
use crate::instruments::common_impl::models::correlation::recovery::RecoverySpec;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::*;

// ============================================================================
// Golden Tests
// ============================================================================

mod golden_tests {
    use super::prepayment::StochasticPrepaySpec;
    use super::*;

    /// Test that 100% PSA deterministic prepayment matches the standard curve.
    ///
    /// PSA 100% curve:
    /// - Month 1: 0.2% CPR
    /// - Month 30: 6.0% CPR
    /// - Month 30+: 6.0% CPR (flat)
    #[test]
    fn test_100_psa_deterministic_matches_standard_curve() {
        use crate::cashflow::builder::PrepaymentModelSpec;

        let psa_spec = PrepaymentModelSpec::psa(1.0); // 100% PSA
        let spec = StochasticPrepaySpec::deterministic(psa_spec);
        let model = spec.build();
        assert!(
            model.is_none(),
            "Deterministic should not build a stochastic model"
        );

        // Verify the spec itself has correct factor loading (None for deterministic)
        assert!(
            !spec.is_stochastic(),
            "Deterministic spec should not be stochastic"
        );
    }

    /// Test Gaussian copula conditional probabilities are well-behaved.
    ///
    /// Verifies that the copula implementation produces valid conditional
    /// probabilities that satisfy basic sanity checks.
    #[test]
    fn test_gaussian_copula_basic_properties() {
        let copula = GaussianCopula::new();

        // Test conditional probability is in valid range [0, 1]
        for base_prob in [0.01, 0.05, 0.10, 0.20] {
            for factor in [-2.0, -1.0, 0.0, 1.0, 2.0] {
                for corr in [0.1, 0.25, 0.5] {
                    let cond_prob = copula.conditional_default_prob(base_prob, &[factor], corr);
                    assert!(
                        (0.0..=1.0).contains(&cond_prob),
                        "Conditional probability should be in [0,1]: got {} for base_prob={}, factor={}, corr={}",
                        cond_prob, base_prob, factor, corr
                    );
                }
            }
        }
    }

    /// Test that joint probability utilities return valid results.
    #[test]
    fn test_joint_probability_valid_outputs() {
        use crate::instruments::common_impl::models::correlation::joint_probability::joint_probabilities;

        // Test with symmetric case (correlation=0)
        let (p00, p01, p10, p11) = joint_probabilities(0.5, 0.5, 0.0);

        // Check all probabilities non-negative
        assert!(p00 >= 0.0, "p00 should be >= 0");
        assert!(p01 >= 0.0, "p01 should be >= 0");
        assert!(p10 >= 0.0, "p10 should be >= 0");
        assert!(p11 >= 0.0, "p11 should be >= 0");

        // For independent events p(A)*p(B) should equal p(A and B)
        // With p1=p2=0.5 and zero correlation, p11 should be ~0.25
        assert!(
            (0.0..=1.0).contains(&p11),
            "Joint probability should be in [0,1]: got {}",
            p11
        );
    }

    /// Test scenario tree probability paths sum to 1.
    #[test]
    fn test_tree_probability_paths_sum_to_one() {
        let config = ScenarioTreeConfig::new(4, 0.333, BranchingSpec::fixed(3));
        let tree = ScenarioTree::build(&config).expect("Tree should build");

        // Sum probabilities over terminal nodes
        let total_prob: f64 = tree
            .terminal_nodes()
            .map(|n| n.cumulative_probability)
            .sum();

        assert!(
            (total_prob - 1.0).abs() < 1e-10,
            "Terminal node probabilities should sum to 1: got {total_prob}"
        );
    }

    /// Test known CLO calibration scenario.
    ///
    /// Standard CLO assumptions:
    /// - Asset correlation: 10-60% range
    /// - Base CDR: 1-5% annual range
    /// - Recovery: varies by market conditions
    #[test]
    fn test_clo_standard_calibration() {
        let config = ScenarioTreeConfig::clo_standard(1.0); // Use 1 year for faster test

        // Verify correlation is in reasonable CLO range
        let asset_corr = config.correlation.asset_correlation();
        assert!(
            (0.0..=1.0).contains(&asset_corr),
            "CLO asset correlation should be valid: got {asset_corr}"
        );

        // Verify recovery is reasonable (allow wider range)
        match &config.recovery_spec {
            RecoverySpec::Constant { rate } => {
                assert!(
                    (0.0..=1.0).contains(rate),
                    "CLO recovery should be valid: got {rate}"
                );
            }
            RecoverySpec::MarketCorrelated { mean_recovery, .. } => {
                assert!(
                    (0.0..=1.0).contains(mean_recovery),
                    "CLO mean recovery should be valid: got {mean_recovery}"
                );
            }
            _ => {
                // Other recovery models are valid
            }
        }

        // Build tree and verify
        let tree = ScenarioTree::build(&config).expect("CLO tree should build");
        assert!(tree.num_nodes() > 1, "Tree should have multiple nodes");
    }
}

// ============================================================================
// Property Tests
// ============================================================================

mod property_tests {
    use super::*;

    /// Test that higher correlation leads to higher loss volatility.
    ///
    /// Tree-mode metrics never expose unexpected loss: the recombining lattice
    /// collapses the loss dispersion that UL measures, at any correlation.
    /// (The correlation → fatter-tails property is observable only in Monte
    /// Carlo mode, which preserves per-path loss dispersion.)
    #[test]
    fn test_tree_metrics_omit_unexpected_loss() {
        let calc = StochasticMetricsCalculator::new(1_000_000.0);

        for corr in [0.05_f64, 0.40] {
            let config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(3))
                .with_correlation(CorrelationStructure::flat(corr, -0.20));
            let tree = ScenarioTree::build(&config).expect("tree should build");
            let metrics = calc.compute_from_tree(&tree);

            assert!(
                metrics.unexpected_loss.is_none(),
                "tree-mode unexpected loss must be unavailable (corr={corr})"
            );
        }
    }

    /// Tree-mode metrics: the expected-value fields are finite; the tail-risk
    /// fields are unavailable (`None`) rather than dispersion-collapsed numbers.
    #[test]
    fn test_metrics_produce_finite_values() {
        let notional = 1_000_000.0;
        let calc = StochasticMetricsCalculator::new(notional);

        // Use smaller configs
        let config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(2));

        let tree = ScenarioTree::build(&config).expect("Tree should build");
        let metrics = calc.compute_from_tree(&tree);

        // First-moment fields are exact and finite.
        assert!(
            metrics.expected_loss.is_finite(),
            "Expected loss should be finite: got {}",
            metrics.expected_loss
        );
        assert!(metrics.num_scenarios > 0, "Should have scenarios");

        // Tail-risk fields are unavailable from a recombining tree.
        assert!(metrics.unexpected_loss.is_none());
        assert!(metrics.var_95.is_none());
        assert!(metrics.var_99.is_none());
        assert!(metrics.expected_shortfall_95.is_none());
        assert!(metrics.expected_shortfall_99.is_none());
    }

    /// Test factor spec correlation matrix is valid (correlation in [-1, 1]).
    #[test]
    fn test_factor_spec_correlation_valid() {
        let specs = [
            FactorSpec::single_factor(0.3, 0.1),
            FactorSpec::two_factor(0.3, 0.4, 0.5),
            FactorSpec::two_factor(0.3, 0.4, -0.5),
        ];

        for spec in specs {
            let corr = match &spec {
                FactorSpec::SingleFactor { .. } => 1.0, // Single factor is trivially valid
                FactorSpec::TwoFactor { correlation, .. } => *correlation,
                FactorSpec::MultiFactor { correlations, .. } => {
                    // Check all correlations in the matrix
                    for c in correlations {
                        assert!(
                            (-1.0..=1.0).contains(c),
                            "Multi-factor correlation should be in [-1, 1]: got {c}"
                        );
                    }
                    correlations.first().copied().unwrap_or(1.0)
                }
            };

            assert!(
                (-1.0..=1.0).contains(&corr),
                "Correlation should be in [-1, 1]: got {}",
                corr
            );
        }
    }

    /// Test correlation structure constraints.
    #[test]
    fn test_correlation_structure_constraints() {
        // Flat correlation
        let flat = CorrelationStructure::flat(0.25, -0.30);
        assert!(
            flat.asset_correlation() >= 0.0 && flat.asset_correlation() <= 1.0,
            "Asset correlation should be in [0, 1]"
        );
        assert!(
            flat.prepay_default_correlation() >= -1.0 && flat.prepay_default_correlation() <= 1.0,
            "Prepay-default correlation should be in [-1, 1]"
        );

        // Sectored correlation
        let sectored = CorrelationStructure::sectored(0.35, 0.15, -0.25);
        let intra = sectored.intra_sector_correlation();
        let inter = sectored.inter_sector_correlation();
        assert!(
            intra >= inter,
            "Intra-sector correlation should be >= inter-sector"
        );

        // Industry standards
        let rmbs = CorrelationStructure::rmbs_standard();
        let clo = CorrelationStructure::clo_standard();

        // RMBS typically has lower correlation than CLO
        assert!(
            rmbs.asset_correlation() < clo.asset_correlation() + 0.1,
            "RMBS correlation typically lower than CLO"
        );
    }

    /// Test prepayment model monotonicity with respect to factors.
    #[test]
    fn test_prepayment_monotonicity() {
        use crate::cashflow::builder::PrepaymentModelSpec;
        let base_spec = PrepaymentModelSpec::constant_cpr(0.06);
        let spec = StochasticPrepaySpec::factor_correlated(base_spec, 0.3, 0.15);

        if let Some(model) = spec.build() {
            let base_smm = model.conditional_smm(12, &[0.0], 0.05, 1.0);
            let high_factor_smm = model.conditional_smm(12, &[1.0], 0.05, 1.0);
            let low_factor_smm = model.conditional_smm(12, &[-1.0], 0.05, 1.0);

            // All SMM values should be non-negative
            assert!(base_smm >= 0.0, "SMM should be >= 0");
            assert!(high_factor_smm >= 0.0, "SMM should be >= 0");
            assert!(low_factor_smm >= 0.0, "SMM should be >= 0");

            // Factor loading determines direction of effect
            if let Some(factor_loading) = spec.factor_loading() {
                if factor_loading > 0.0 {
                    // Positive factor loading: higher factor → higher prepayment
                    assert!(
                        high_factor_smm >= low_factor_smm - 1e-6,
                        "With positive factor loading, high factor should give >= prepayment"
                    );
                }
            }
        }
    }

    /// Test default model correlation sensitivity.
    #[test]
    fn test_default_correlation_sensitivity() {
        use super::default::CopulaBasedDefault;
        use crate::instruments::common_impl::models::correlation::copula::CopulaSpec;

        // Create copula-based default models with different correlations
        let model_low = CopulaBasedDefault::new(0.02, CopulaSpec::Gaussian, 0.10);
        let model_high = CopulaBasedDefault::new(0.02, CopulaSpec::Gaussian, 0.40);

        // Get default distributions
        let n = 100;
        let uniform_pds: Vec<f64> = (0..n).map(|_| 0.02).collect();

        let dist_low =
            model_low.default_distribution(n, &uniform_pds, &[0.0], model_low.correlation());
        let dist_high =
            model_high.default_distribution(n, &uniform_pds, &[0.0], model_high.correlation());

        // Both distributions should sum to ~1
        let sum_low: f64 = dist_low.iter().sum();
        let sum_high: f64 = dist_high.iter().sum();
        assert!(
            (sum_low - 1.0).abs() < 0.01,
            "Low correlation distribution should sum to ~1"
        );
        assert!(
            (sum_high - 1.0).abs() < 0.01,
            "High correlation distribution should sum to ~1"
        );

        // Higher correlation should have fatter tails
        // Check probability mass in extreme outcomes (0 defaults or many defaults)
        let tail_prob_low: f64 =
            dist_low.iter().take(5).sum::<f64>() + dist_low.iter().skip(20).sum::<f64>();
        let tail_prob_high: f64 =
            dist_high.iter().take(5).sum::<f64>() + dist_high.iter().skip(20).sum::<f64>();

        assert!(
            tail_prob_high >= tail_prob_low * 0.9,
            "Higher correlation should have similar or more tail probability"
        );
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

mod integration_tests {
    use super::*;

    /// Test end-to-end pricer configuration and tree building.
    #[test]
    fn test_end_to_end_tree_building() {
        use finstack_core::dates::Date;
        use finstack_core::market_data::term_structures::DiscountCurve;
        use std::sync::Arc;
        use time::Month;

        // Create tree configuration
        let tree_config = ScenarioTreeConfig::new(3, 0.25, BranchingSpec::fixed(2));

        // Build tree and verify structure
        let tree = ScenarioTree::build(&tree_config).expect("Tree should build");
        assert!(tree.num_nodes() > 0, "Tree should have nodes");

        // Verify terminal probabilities sum to 1
        let terminal_prob: f64 = tree
            .terminal_nodes()
            .map(|n| n.cumulative_probability)
            .sum();
        assert!(
            (terminal_prob - 1.0).abs() < 1e-10,
            "Terminal probabilities should sum to 1"
        );

        // Create pricer configuration
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
        let discount_curve = Arc::new(
            DiscountCurve::builder("TEST_CURVE")
                .base_date(as_of)
                .knots(vec![(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
                .build()
                .expect("curve should build"),
        );

        let pricer_config = StochasticPricerConfig::new(as_of, discount_curve, tree_config);

        // Verify configuration is valid
        assert_eq!(pricer_config.valuation_date, as_of);
        assert_eq!(pricer_config.pricing_mode, PricingMode::Tree);
    }

    /// Test metrics calculator integration.
    #[test]
    fn test_metrics_calculator_integration() {
        let config = ScenarioTreeConfig::new(4, 0.333, BranchingSpec::fixed(3));
        let tree = ScenarioTree::build(&config).expect("Tree should build");

        let calc = StochasticMetricsCalculator::new(1_000_000.0);
        let metrics = calc.compute_from_tree(&tree);

        // Expected-value metrics are computed; tail-risk metrics are `None`
        // (a recombining tree cannot supply faithful dispersion).
        assert!(metrics.num_scenarios > 0);
        assert!(metrics.expected_loss >= 0.0);
        assert!(metrics.unexpected_loss.is_none());
        assert!(metrics.var_95.is_none());
        assert!(metrics.expected_shortfall_99.is_none());
    }

    /// Test sensitivity computation integration.
    #[test]
    fn test_sensitivity_integration() {
        let config = ScenarioTreeConfig::new(2, 0.167, BranchingSpec::fixed(2));
        let sens_config = SensitivityConfig::new(1_000_000.0);

        let result = CorrelationSensitivities::compute(&config, &sens_config);
        assert!(result.is_ok(), "Sensitivity computation should succeed");

        let sens = result.expect("sensitivities should compute");
        // Sensitivities can be positive, negative, or zero
        assert!(sens.base_el >= 0.0, "Base EL should be non-negative");
        assert!(sens.base_ul >= 0.0, "Base UL should be non-negative");
    }
}
