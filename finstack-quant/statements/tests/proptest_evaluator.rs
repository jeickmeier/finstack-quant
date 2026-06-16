//! Property-based tests for evaluator determinism.
//!
//! Verifies that evaluation produces consistent results.

use finstack_quant_statements::prelude::*;
use proptest::prelude::*;

proptest! {
    /// Test that evaluating the same model multiple times produces identical results
    #[test]
    fn evaluation_is_deterministic(
        revenue_q1 in 100_000.0..1_000_000.0,
        revenue_q2 in 100_000.0..1_000_000.0,
        cogs_multiplier in 0.4..0.8,
    ) {
        let model = ModelBuilder::new("proptest")
            .periods("2025Q1..Q2", None)
            .unwrap()
            .value("revenue", &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(revenue_q1)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(revenue_q2)),
            ])
            .compute("cogs", format!("revenue * {}", cogs_multiplier))
            .unwrap()
            .compute("gross_profit", "revenue - cogs")
            .unwrap()
            .build()
            .unwrap();

        // Evaluate 10 times
        let mut results_vec = Vec::new();
        for _ in 0..10 {
            let mut evaluator = Evaluator::new();
            let results = evaluator.evaluate(&model).unwrap();
            results_vec.push(results);
        }

        // All results should be identical
        for i in 1..results_vec.len() {
            let period_q1 = PeriodId::quarter(2025, 1);
            let period_q2 = PeriodId::quarter(2025, 2);

            prop_assert_eq!(
                results_vec[0].get("revenue", &period_q1),
                results_vec[i].get("revenue", &period_q1),
                "Revenue Q1 not deterministic on iteration {}", i
            );

            prop_assert_eq!(
                results_vec[0].get("gross_profit", &period_q1),
                results_vec[i].get("gross_profit", &period_q1),
                "Gross profit Q1 not deterministic on iteration {}", i
            );

            prop_assert_eq!(
                results_vec[0].get("gross_profit", &period_q2),
                results_vec[i].get("gross_profit", &period_q2),
                "Gross profit Q2 not deterministic on iteration {}", i
            );
        }
    }

    /// Test that DAG construction is deterministic
    #[test]
    fn dag_construction_deterministic(
        num_nodes in 3usize..10usize,
    ) {
        // Build a model with a chain of dependent nodes
        let mut builder = ModelBuilder::new("dag_test")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("base", &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))]);

        for i in 1..num_nodes {
            let prev = if i == 1 { "base".to_string() } else { format!("node_{}", i - 1) };
            builder = builder.compute(format!("node_{}", i), format!("{} * 1.1", prev)).unwrap();
        }

        let model = builder.build().unwrap();

        // Build DAG multiple times
        let dag1 = finstack_quant_statements::evaluator::DependencyGraph::from_model(&model).unwrap();
        let dag2 = finstack_quant_statements::evaluator::DependencyGraph::from_model(&model).unwrap();

        // Dependencies should be identical
        prop_assert_eq!(dag1.dependencies.len(), dag2.dependencies.len());

        for (node_id, deps1) in &dag1.dependencies {
            let deps2 = dag2.dependencies.get(node_id).unwrap();
            prop_assert_eq!(deps1, deps2, "Dependencies differ for node {}", node_id);
        }
    }

    /// Test that seeded forecast methods are deterministic
    #[test]
    fn seeded_forecast_deterministic(
        seed in 1u64..1000u64,
        mean in 0.01..0.10,
        std_dev in 0.01..0.05,
    ) {
        let model = ModelBuilder::new("forecast_test")
            .periods("2025Q1..Q4", Some("2025Q1"))
            .unwrap()
            .value("revenue", &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100_000.0))])
            .forecast("revenue", ForecastSpec::lognormal(mean, std_dev, seed))
            .build()
            .unwrap();

        // Evaluate twice with same seed
        let mut eval1 = Evaluator::new();
        let results1 = eval1.evaluate(&model).unwrap();

        let mut eval2 = Evaluator::new();
        let results2 = eval2.evaluate(&model).unwrap();

        // Results should be identical
        for q in 2..=4 {
            let period = PeriodId::quarter(2025, q);
            prop_assert_eq!(
                results1.get("revenue", &period),
                results2.get("revenue", &period),
                "Forecast not deterministic for Q{}", q
            );
        }
    }

    /// Test that growth forecast is consistent with parameters
    #[test]
    fn growth_forecast_consistent(
        base_value in 50_000.0..200_000.0,
        growth_rate in 0.01..0.20,
    ) {
        let model = ModelBuilder::new("growth_test")
            .periods("2025Q1..Q4", Some("2025Q1"))
            .unwrap()
            .value("revenue", &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(base_value))])
            .forecast("revenue", ForecastSpec::growth(growth_rate))
            .build()
            .unwrap();

        let mut evaluator = Evaluator::new();
        let results = evaluator.evaluate(&model).unwrap();

        // Verify growth rate is applied correctly
        let q1_value = results.get("revenue", &PeriodId::quarter(2025, 1)).unwrap();
        let q2_value = results.get("revenue", &PeriodId::quarter(2025, 2)).unwrap();

        let expected_q2 = q1_value * (1.0 + growth_rate);
        let diff = (q2_value - expected_q2).abs();

        prop_assert!(diff < 0.01, "Growth calculation incorrect: expected {}, got {}", expected_q2, q2_value);
    }
}

#[test]
fn test_proptest_evaluator_infrastructure_works() {
    // Smoke test to ensure proptest is properly configured
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(model.id, "test");
}
