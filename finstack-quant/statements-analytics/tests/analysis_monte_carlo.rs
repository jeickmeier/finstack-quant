//! Monte Carlo analysis integration tests.
#![allow(clippy::expect_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::MonteCarloConfig;
use finstack_quant_statements::types::{AmountOrScalar, ForecastSpec};

#[test]
fn evaluate_monte_carlo_produces_deterministic_results() {
    let model = ModelBuilder::new("mc-test")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .expect("valid periods")
        .mixed("revenue")
        .values(&[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(110_000.0),
            ),
        ])
        .forecast(ForecastSpec::normal(120_000.0, 10_000.0, 42))
        .build()
        .expect("valid mixed node")
        .build()
        .expect("valid model");

    let config = MonteCarloConfig::new(32, 7);

    let mut eval1 = finstack_quant_statements::evaluator::Evaluator::new();
    let mut eval2 = finstack_quant_statements::evaluator::Evaluator::new();

    let res1 = eval1
        .evaluate_monte_carlo(&model, &config)
        .expect("mc eval 1");
    let res2 = eval2
        .evaluate_monte_carlo(&model, &config)
        .expect("mc eval 2");

    assert_eq!(res1.n_paths, res2.n_paths);
    assert_eq!(res1.percentiles, res2.percentiles);
    assert_eq!(res1.percentile_results.len(), res2.percentile_results.len());

    let p95_series_1 = res1
        .percentile_by_period("revenue", 0.95)
        .expect("p95 series");
    let p95_series_2 = res2
        .percentile_by_period("revenue", 0.95)
        .expect("p95 series");
    assert_eq!(p95_series_1, p95_series_2);
}

#[test]
fn evaluate_monte_carlo_correlated_normals_constant_spread() {
    let mut fp_b = ForecastSpec::normal(200.0, 10.0, 42);
    fp_b.params
        .insert("correlation_with".into(), serde_json::json!("a"));
    fp_b.params
        .insert("correlation".into(), serde_json::json!(1.0));

    let model = ModelBuilder::new("mc-corr")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .expect("valid periods")
        .mixed("a")
        .values(&[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(110_000.0),
            ),
        ])
        .forecast(ForecastSpec::normal(100.0, 10.0, 42))
        .build()
        .expect("valid mixed node")
        .mixed("b")
        .values(&[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(110_000.0),
            ),
        ])
        .forecast(fp_b)
        .formula("a")
        .expect("valid formula")
        .build()
        .expect("valid mixed node")
        .build()
        .expect("valid model");

    let config = MonteCarloConfig::new(4, 1);
    let mut eval = finstack_quant_statements::evaluator::Evaluator::new();
    let res = eval.evaluate_monte_carlo(&model, &config).expect("mc eval");

    let q3 = PeriodId::quarter(2025, 3).expect("valid period fixture");
    let a = res
        .percentile_by_period("a", 0.5)
        .expect("a")
        .get(&q3)
        .copied()
        .expect("a q3");
    let b = res
        .percentile_by_period("b", 0.5)
        .expect("b")
        .get(&q3)
        .copied()
        .expect("b q3");
    assert!(
        (b - a - 100.0).abs() < 1e-9,
        "expected b = a + 100, got a={a} b={b}"
    );
}

#[test]
fn evaluate_monte_carlo_correlated_lognormals_preserve_ratio() {
    let mut fp_b = ForecastSpec::log_normal(4.0, 0.2, 42);
    fp_b.params
        .insert("correlation_with".into(), serde_json::json!("a"));
    fp_b.params
        .insert("correlation".into(), serde_json::json!(1.0));

    let model = ModelBuilder::new("mc-corr-lognormal")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .expect("valid periods")
        .mixed("a")
        .values(&[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(110_000.0),
            ),
        ])
        .forecast(ForecastSpec::log_normal(3.0, 0.2, 42))
        .build()
        .expect("valid mixed node")
        .mixed("b")
        .values(&[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(110_000.0),
            ),
        ])
        .forecast(fp_b)
        .formula("a")
        .expect("valid formula")
        .build()
        .expect("valid mixed node")
        .build()
        .expect("valid model");

    let config = MonteCarloConfig::new(4, 1);
    let mut eval = finstack_quant_statements::evaluator::Evaluator::new();
    let res = eval.evaluate_monte_carlo(&model, &config).expect("mc eval");

    let q3 = PeriodId::quarter(2025, 3).expect("valid period fixture");
    let a = res
        .percentile_by_period("a", 0.5)
        .expect("a")
        .get(&q3)
        .copied()
        .expect("a q3");
    let b = res
        .percentile_by_period("b", 0.5)
        .expect("b")
        .get(&q3)
        .copied()
        .expect("b q3");
    assert!(
        (b.ln() - a.ln() - 1.0).abs() < 1e-9,
        "expected ln(b) = ln(a) + 1, got a={a} b={b}"
    );
}

#[test]
fn evaluate_monte_carlo_correlated_forecast_rejects_unknown_peer() {
    let mut fp = ForecastSpec::normal(120_000.0, 10_000.0, 42);
    fp.params
        .insert("correlation_with".into(), serde_json::json!("missing_node"));
    fp.params
        .insert("correlation".into(), serde_json::json!(0.5));

    let model = ModelBuilder::new("mc-corr-missing-peer")
        .periods("2025Q1..Q4", Some("2025Q2"))
        .expect("valid periods")
        .mixed("revenue")
        .values(&[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100_000.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(110_000.0),
            ),
        ])
        .forecast(fp)
        .build()
        .expect("valid mixed node")
        .build()
        .expect("valid model");

    let config = MonteCarloConfig::new(16, 7);
    let mut eval = finstack_quant_statements::evaluator::Evaluator::new();
    let err = eval
        .evaluate_monte_carlo(&model, &config)
        .expect_err("unknown peer should fail");
    assert!(
        err.to_string().contains("unknown node 'missing_node'"),
        "unexpected error: {err}"
    );
}
