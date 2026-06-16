//! Scorecard extension integration tests.
#![allow(clippy::expect_used)]

use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements_analytics::extensions::{
    CreditScorecardExtension, ScorecardConfig, ScorecardMetric,
};

#[test]
fn test_scorecard_extension_creation() {
    let extension = CreditScorecardExtension::new();
    assert!(extension.config().is_none());
}

#[test]
fn test_scorecard_extension_with_config() {
    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![ScorecardMetric {
            name: "leverage".into(),
            formula: "debt / ebitda".into(),
            weight: 0.3,
            thresholds: indexmap::IndexMap::new(),
            description: Some("Leverage ratio".into()),
        }],
        min_rating: None,
        period: None,
    };

    let extension = CreditScorecardExtension::with_config(config);
    assert!(extension.config().is_some());
    assert_eq!(
        extension
            .config()
            .expect("test should succeed")
            .metrics
            .len(),
        1
    );
}

#[test]
fn test_scorecard_execute_requires_config() {
    let model = FinancialModelSpec::new("test", Vec::new());
    let results = StatementResult::new();

    let mut extension = CreditScorecardExtension::new();
    let result = extension.execute(&model, &results);

    assert!(result.is_err());
    assert!(result
        .expect_err("should fail")
        .to_string()
        .contains("requires configuration"));
}

#[test]
fn test_scorecard_config_validation() {
    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![ScorecardMetric {
            name: "leverage".into(),
            formula: "debt / ebitda".into(),
            weight: 0.3,
            thresholds: {
                let mut t = indexmap::IndexMap::new();
                t.insert("AAA".into(), (0.0, 1.0));
                t.insert("AA".into(), (1.0, 2.0));
                t.insert("A".into(), (2.0, 3.0));
                t
            },
            description: None,
        }],
        min_rating: None,
        period: None,
    };

    assert!(CreditScorecardExtension::validate_config(&config).is_ok());
}

#[test]
fn test_scorecard_config_validation_invalid_weights() {
    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![ScorecardMetric {
            name: "leverage".into(),
            formula: "debt / ebitda".into(),
            weight: 150.0,
            thresholds: indexmap::IndexMap::new(),
            description: None,
        }],
        min_rating: None,
        period: None,
    };

    assert!(CreditScorecardExtension::validate_config(&config).is_err());
}

#[test]
fn test_scorecard_config_validation_invalid_scale() {
    let config = ScorecardConfig {
        rating_scale: "UnknownScale".into(),
        metrics: vec![],
        min_rating: None,
        period: None,
    };

    assert!(CreditScorecardExtension::validate_config(&config).is_err());
}

#[test]
fn test_scorecard_metric() {
    let metric = ScorecardMetric {
        name: "debt_to_ebitda".into(),
        formula: "total_debt / ttm(ebitda)".into(),
        weight: 0.3,
        thresholds: indexmap::IndexMap::new(),
        description: Some("Leverage ratio".into()),
    };

    assert_eq!(metric.name, "debt_to_ebitda");
    assert_eq!(metric.weight, 0.3);
}

#[test]
fn test_scorecard_config_with_thresholds() {
    let mut thresholds = indexmap::IndexMap::new();
    thresholds.insert("AAA".into(), (0.0, 1.0));
    thresholds.insert("AA".into(), (1.0, 2.0));
    thresholds.insert("A".into(), (2.0, 3.0));

    let metric = ScorecardMetric {
        name: "debt_to_ebitda".into(),
        formula: "total_debt / ttm(ebitda)".into(),
        weight: 0.3,
        thresholds,
        description: Some("Leverage ratio".into()),
    };

    assert_eq!(metric.thresholds.len(), 3);
    assert_eq!(metric.thresholds.get("AAA"), Some(&(0.0, 1.0)));
}

#[test]
fn test_scorecard_ttm_formula_uses_full_history() {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements::types::AmountOrScalar;

    let model = ModelBuilder::new("scorecard-ttm")
        .periods("2025Q1..Q4", None)
        .expect("valid periods")
        .value(
            "ebitda",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(120.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(130.0)),
            ],
        )
        .value(
            "total_debt",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(1000.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(1000.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(1000.0)),
            ],
        )
        .build()
        .expect("valid model");

    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");

    // Thresholds must cover the expected value (~2.17): an unmatched factor
    // is excluded from metric_scores under the NM-exclusion convention.
    let mut thresholds = indexmap::IndexMap::new();
    thresholds.insert("A".into(), (2.0, 3.0));

    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![ScorecardMetric {
            name: "leverage".into(),
            formula: "total_debt / ttm(ebitda)".into(),
            weight: 1.0,
            thresholds,
            description: None,
        }],
        min_rating: None,
        period: None,
    };
    let mut extension = CreditScorecardExtension::with_config(config);
    let report = extension
        .execute(&model, &results)
        .expect("scorecard should succeed");

    let score_data = report.data.get("total_score").expect("total_score");
    let leverage_data = report.data.get("metric_scores").expect("metric_scores");
    let leverage_value = leverage_data[0]["value"].as_f64().expect("leverage value");

    // ttm(ebitda) = 100 + 110 + 120 + 130 = 460
    // leverage = 1000 / 460 ≈ 2.174
    assert!(
        (leverage_value - 1000.0 / 460.0).abs() < 0.01,
        "leverage should be ~2.174, got {}",
        leverage_value
    );
    let _ = score_data;
}

#[test]
fn test_scorecard_warns_when_thresholds_do_not_cover_metric_value() {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements::types::AmountOrScalar;

    let model = ModelBuilder::new("scorecard-threshold-gap")
        .periods("2025Q1..Q1", None)
        .expect("valid periods")
        .value(
            "ebitda",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .value(
            "total_debt",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(250.0))],
        )
        .build()
        .expect("valid model");

    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate(&model)
        .expect("evaluation should succeed");

    let mut thresholds = indexmap::IndexMap::new();
    thresholds.insert("AAA".into(), (0.0, 1.0));

    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![ScorecardMetric {
            name: "leverage".into(),
            formula: "total_debt / ebitda".into(),
            weight: 1.0,
            thresholds,
            description: None,
        }],
        min_rating: None,
        period: None,
    };

    let mut extension = CreditScorecardExtension::with_config(config);
    let report = extension
        .execute(&model, &results)
        .expect("scorecard should succeed");

    // The unmatched factor is excluded (rating-agency "NM" treatment): no
    // fallback score, zero weight coverage, and the report is stamped partial.
    assert_eq!(
        report.data.get("total_score").and_then(|v| v.as_f64()),
        Some(0.0)
    );
    assert_eq!(
        report.data.get("partial").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        report.data.get("weight_coverage").and_then(|v| v.as_f64()),
        Some(0.0)
    );
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("thresholds did not match"));
    assert!(report.warnings[0].contains("excluding factor"));
}
