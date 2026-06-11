//! Comprehensive extension execution tests with valid configurations.
//!
//! These tests provide full coverage of the analytics extensions by executing
//! them with properly configured parameters via their inherent methods.

use finstack_statements::prelude::*;
use finstack_statements_analytics::extensions::{
    AccountType, CorkscrewAccount, CorkscrewConfig, CorkscrewExtension, CorkscrewStatus,
    CreditScorecardExtension, ScorecardConfig, ScorecardMetric, ScorecardStatus,
};

// ============================================================================
// Corkscrew Extension Full Execution Tests
// ============================================================================

#[test]
fn test_corkscrew_extension_with_valid_config() {
    // Build a model with balance sheet accounts that roll forward
    let model = ModelBuilder::new("balance_sheet")
        .periods("2025Q1..2025Q3", None)
        .unwrap()
        // Cash account
        .value(
            "cash",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(115_000.0),
                ),
            ],
        )
        // Cash changes
        .value(
            "cash_inflows",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(15_000.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(10_000.0)),
            ],
        )
        .value(
            "cash_outflows",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(-5_000.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(-5_000.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Configure corkscrew extension
    let config = CorkscrewConfig {
        accounts: vec![CorkscrewAccount {
            node_id: "cash".into(),
            account_type: AccountType::Asset,
            changes: vec!["cash_inflows".into(), "cash_outflows".into()],
            beginning_balance_node: None,
        }],
        tolerance: 0.01,
        fail_on_error: false,
    };

    let mut extension = CorkscrewExtension::with_config(config);

    // Execute extension via inherent method
    let report = extension.execute(&model, &results).unwrap();

    assert_eq!(report.status, CorkscrewStatus::Success);
    assert!(report.data.contains_key("validations"));
}

#[test]
fn test_corkscrew_with_multiple_accounts() {
    let model = ModelBuilder::new("multi_account")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "cash",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(50_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(55_000.0)),
            ],
        )
        .value(
            "debt",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(190_000.0),
                ),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let config = CorkscrewConfig {
        accounts: vec![
            CorkscrewAccount {
                node_id: "cash".into(),
                account_type: AccountType::Asset,
                changes: vec![],
                beginning_balance_node: None,
            },
            CorkscrewAccount {
                node_id: "debt".into(),
                account_type: AccountType::Liability,
                changes: vec![],
                beginning_balance_node: None,
            },
        ],
        tolerance: 0.01,
        fail_on_error: false,
    };

    let mut extension = CorkscrewExtension::with_config(config);

    let report = extension.execute(&model, &results).unwrap();
    assert_eq!(report.status, CorkscrewStatus::Success);
}

#[test]
fn test_corkscrew_set_config() {
    let mut extension = CorkscrewExtension::new();
    assert!(extension.config().is_none());

    let config = CorkscrewConfig {
        accounts: vec![CorkscrewAccount {
            node_id: "test".into(),
            account_type: AccountType::Asset,
            changes: vec![],
            beginning_balance_node: None,
        }],
        tolerance: 0.01,
        fail_on_error: false,
    };

    extension.set_config(config.clone());
    assert!(extension.config().is_some());
    assert_eq!(extension.config().unwrap().tolerance, 0.01);
}

// ============================================================================
// Credit Scorecard Extension Full Execution Tests
// ============================================================================

#[test]
fn test_scorecard_extension_with_valid_config() {
    // Build a model with leverage metrics
    let model = ModelBuilder::new("credit_model")
        .periods("2024Q1..2025Q2", None)
        .unwrap()
        .value(
            "total_debt",
            &[
                (
                    PeriodId::quarter(2024, 1),
                    AmountOrScalar::scalar(300_000.0),
                ),
                (
                    PeriodId::quarter(2024, 2),
                    AmountOrScalar::scalar(310_000.0),
                ),
                (
                    PeriodId::quarter(2024, 3),
                    AmountOrScalar::scalar(320_000.0),
                ),
                (
                    PeriodId::quarter(2024, 4),
                    AmountOrScalar::scalar(330_000.0),
                ),
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(340_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(350_000.0),
                ),
            ],
        )
        .value(
            "ebitda",
            &[
                (PeriodId::quarter(2024, 1), AmountOrScalar::scalar(25_000.0)),
                (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(26_000.0)),
                (PeriodId::quarter(2024, 3), AmountOrScalar::scalar(27_000.0)),
                (PeriodId::quarter(2024, 4), AmountOrScalar::scalar(28_000.0)),
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(29_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(30_000.0)),
            ],
        )
        .value(
            "interest_expense",
            &[
                (PeriodId::quarter(2024, 1), AmountOrScalar::scalar(5_000.0)),
                (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(5_100.0)),
                (PeriodId::quarter(2024, 3), AmountOrScalar::scalar(5_200.0)),
                (PeriodId::quarter(2024, 4), AmountOrScalar::scalar(5_300.0)),
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(5_400.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(5_500.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Configure scorecard with S&P thresholds
    let mut thresholds_debt_ebitda = indexmap::IndexMap::new();
    thresholds_debt_ebitda.insert("AAA".into(), (0.0, 1.0));
    thresholds_debt_ebitda.insert("AA".into(), (1.0, 2.0));
    thresholds_debt_ebitda.insert("A".into(), (2.0, 3.0));
    thresholds_debt_ebitda.insert("BBB".into(), (3.0, 4.0));
    thresholds_debt_ebitda.insert("BB".into(), (4.0, 6.0));
    thresholds_debt_ebitda.insert("B".into(), (6.0, 999.0));

    let mut thresholds_interest_cov = indexmap::IndexMap::new();
    thresholds_interest_cov.insert("AAA".into(), (8.0, 999.0));
    thresholds_interest_cov.insert("AA".into(), (6.0, 8.0));
    thresholds_interest_cov.insert("A".into(), (4.5, 6.0));
    thresholds_interest_cov.insert("BBB".into(), (3.0, 4.5));
    thresholds_interest_cov.insert("BB".into(), (2.0, 3.0));
    thresholds_interest_cov.insert("B".into(), (0.0, 2.0));

    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![
            ScorecardMetric {
                name: "debt_to_ebitda".into(),
                formula: "total_debt / ttm(ebitda)".into(),
                weight: 0.5,
                thresholds: thresholds_debt_ebitda,
                description: Some("Leverage ratio".into()),
            },
            ScorecardMetric {
                name: "interest_coverage".into(),
                formula: "ttm(ebitda) / ttm(interest_expense)".into(),
                weight: 0.5,
                thresholds: thresholds_interest_cov,
                description: Some("Coverage ratio".into()),
            },
        ],
        min_rating: Some("BBB".into()),
        period: None,
    };

    let mut extension = CreditScorecardExtension::with_config(config);

    // Execute extension via inherent method
    let report = extension.execute(&model, &results).unwrap();

    assert_eq!(report.status, ScorecardStatus::Success);
    assert!(report.data.contains_key("rating"));
    assert!(report.data.contains_key("total_score"));
    assert!(report.data.contains_key("metric_scores"));
}

#[test]
fn test_scorecard_set_config() {
    let mut extension = CreditScorecardExtension::new();
    assert!(extension.config().is_none());

    let config = ScorecardConfig {
        rating_scale: "Moody's".into(),
        metrics: vec![ScorecardMetric {
            name: "leverage".into(),
            formula: "debt / ebitda".into(),
            weight: 1.0,
            thresholds: indexmap::IndexMap::new(),
            description: None,
        }],
        min_rating: None,
        period: None,
    };

    extension.set_config(config.clone());
    assert!(extension.config().is_some());
    assert_eq!(extension.config().unwrap().rating_scale, "Moody's");
}

#[test]
fn test_scorecard_moodys_rating_scale() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "debt",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(100.0)),
            ],
        )
        .value(
            "equity",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(50.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(50.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(50.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(50.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let mut thresholds = indexmap::IndexMap::new();
    thresholds.insert("Aaa".into(), (0.0, 1.0));
    thresholds.insert("Aa1".into(), (1.0, 2.0));
    thresholds.insert("A1".into(), (2.0, 3.0));
    thresholds.insert("Baa1".into(), (3.0, 999.0));

    let config = ScorecardConfig {
        rating_scale: "Moody's".into(),
        metrics: vec![ScorecardMetric {
            name: "debt_to_equity".into(),
            formula: "debt / equity".into(),
            weight: 1.0,
            thresholds,
            description: None,
        }],
        min_rating: None,
        period: None,
    };

    let mut extension = CreditScorecardExtension::with_config(config);

    let report = extension.execute(&model, &results).unwrap();

    assert_eq!(report.status, ScorecardStatus::Success);
    assert!(report.data.contains_key("rating"));
}

#[test]
fn test_scorecard_fitch_rating_scale() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "value",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let mut thresholds = indexmap::IndexMap::new();
    thresholds.insert("AAA".into(), (0.0, 999.0));

    let config = ScorecardConfig {
        rating_scale: "Fitch".into(),
        metrics: vec![ScorecardMetric {
            name: "simple".into(),
            formula: "value".into(),
            weight: 1.0,
            thresholds,
            description: None,
        }],
        min_rating: None,
        period: None,
    };

    let mut extension = CreditScorecardExtension::with_config(config);

    let report = extension.execute(&model, &results).unwrap();
    assert_eq!(report.status, ScorecardStatus::Success);
}

#[test]
fn test_scorecard_with_minimum_rating_warning() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "debt",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(600.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(600.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(600.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(600.0)),
            ],
        )
        .value(
            "ebitda",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(100.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Debt/EBITDA = 600/400 = 1.5 (should be A rating range)
    let mut thresholds = indexmap::IndexMap::new();
    thresholds.insert("AAA".into(), (0.0, 1.0));
    thresholds.insert("AA".into(), (1.0, 2.0));
    thresholds.insert("A".into(), (2.0, 3.0));
    thresholds.insert("BBB".into(), (3.0, 999.0));

    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![ScorecardMetric {
            name: "leverage".into(),
            formula: "debt / ttm(ebitda)".into(),
            weight: 1.0,
            thresholds,
            description: None,
        }],
        min_rating: Some("AAA".into()), // Higher than expected
        period: None,
    };

    let mut extension = CreditScorecardExtension::with_config(config);

    let report = extension.execute(&model, &results).unwrap();

    // Should have warnings about not meeting minimum rating
    assert!(
        !report.warnings.is_empty(),
        "Should warn about not meeting minimum rating"
    );
}

#[test]
fn test_scorecard_multiple_metrics_weighted() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "metric1",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1.5)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(1.5)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(1.5)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(1.5)),
            ],
        )
        .value(
            "metric2",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(2.5)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(2.5)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(2.5)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(2.5)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let mut thresholds1 = indexmap::IndexMap::new();
    thresholds1.insert("AAA".into(), (0.0, 1.0));
    thresholds1.insert("AA".into(), (1.0, 2.0));
    thresholds1.insert("A".into(), (2.0, 3.0));

    let mut thresholds2 = indexmap::IndexMap::new();
    thresholds2.insert("AAA".into(), (0.0, 2.0));
    thresholds2.insert("AA".into(), (2.0, 3.0));
    thresholds2.insert("A".into(), (3.0, 4.0));

    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![
            ScorecardMetric {
                name: "metric1".into(),
                formula: "metric1".into(),
                weight: 0.3,
                thresholds: thresholds1,
                description: None,
            },
            ScorecardMetric {
                name: "metric2".into(),
                formula: "metric2".into(),
                weight: 0.7,
                thresholds: thresholds2,
                description: None,
            },
        ],
        min_rating: None,
        period: None,
    };

    let mut extension = CreditScorecardExtension::with_config(config);

    let report = extension.execute(&model, &results).unwrap();

    assert_eq!(report.status, ScorecardStatus::Success);
    assert!(report.data.contains_key("metric_scores"));
    let metric_scores = report.data.get("metric_scores").unwrap();
    assert!(metric_scores.is_array());
    assert_eq!(metric_scores.as_array().unwrap().len(), 2);
}

#[test]
fn test_scorecard_metric_evaluation_error_handling() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q1", None)
        .unwrap()
        .value(
            "value",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let config = ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![ScorecardMetric {
            name: "invalid_metric".into(),
            formula: "nonexistent_node".into(), // References non-existent node
            weight: 1.0,
            thresholds: indexmap::IndexMap::new(),
            description: None,
        }],
        min_rating: None,
        period: None,
    };

    let mut extension = CreditScorecardExtension::with_config(config);

    let report = extension.execute(&model, &results).unwrap();

    // Should fail due to invalid formula
    assert_eq!(report.status, ScorecardStatus::Failed);
    assert!(!report.errors.is_empty());
}

// ============================================================================
// Scorecard NM-exclusion, period selection, and partial stamping
// ============================================================================

fn two_metric_model() -> (
    finstack_statements::types::FinancialModelSpec,
    finstack_statements::evaluator::StatementResult,
) {
    let model = ModelBuilder::new("nm_test")
        .periods("2025Q1..2025Q2", None)
        .unwrap()
        .value(
            "debt",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(150.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(150.0)),
            ],
        )
        .value(
            "ebitda",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                // EBITDA collapses to zero → debt/ebitda = inf (non-finite).
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(0.0)),
            ],
        )
        .value(
            "coverage",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(5.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(5.0)),
            ],
        )
        .build()
        .unwrap();
    let mut evaluator = finstack_statements::evaluator::Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();
    (model, results)
}

fn leverage_coverage_config() -> ScorecardConfig {
    let mut leverage_thresholds = indexmap::IndexMap::new();
    leverage_thresholds.insert("AAA".into(), (0.0, 1.0));
    leverage_thresholds.insert("AA".into(), (1.0, 2.0));
    leverage_thresholds.insert("A".into(), (2.0, 3.0));

    let mut coverage_thresholds = indexmap::IndexMap::new();
    coverage_thresholds.insert("AAA".into(), (8.0, 999.0));
    coverage_thresholds.insert("AA".into(), (4.0, 8.0));
    coverage_thresholds.insert("A".into(), (2.0, 4.0));

    ScorecardConfig {
        rating_scale: "S&P".into(),
        metrics: vec![
            ScorecardMetric {
                name: "leverage".into(),
                formula: "debt / ebitda".into(),
                weight: 0.5,
                thresholds: leverage_thresholds,
                description: None,
            },
            ScorecardMetric {
                name: "coverage".into(),
                formula: "coverage".into(),
                weight: 0.5,
                thresholds: coverage_thresholds,
                description: None,
            },
        ],
        min_rating: None,
        period: None,
    }
}

#[test]
fn test_scorecard_non_finite_metric_excluded_and_weights_renormalized() {
    let (model, results) = two_metric_model();

    // Default period = last (Q2, no actuals): leverage = 150/0 = inf → NM.
    let mut extension = CreditScorecardExtension::with_config(leverage_coverage_config());
    let report = extension.execute(&model, &results).unwrap();

    assert_eq!(report.status, ScorecardStatus::Success);
    // Coverage = 5.0 → AA → score 90; leverage excluded, so the weighted
    // average renormalizes to coverage alone.
    assert_eq!(
        report.data.get("total_score").and_then(|v| v.as_f64()),
        Some(90.0),
        "excluded NM factor must renormalize weights, not drag the score to a default"
    );
    assert_eq!(
        report.data.get("partial").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        report.data.get("weight_coverage").and_then(|v| v.as_f64()),
        Some(0.5)
    );
    assert!(report
        .warnings
        .iter()
        .any(|w| w.contains("not finite") && w.contains("excluding factor")));
    // metric_scores only lists the included factor.
    let metric_scores = report
        .data
        .get("metric_scores")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(metric_scores.len(), 1);
}

#[test]
fn test_scorecard_period_selection() {
    let (model, results) = two_metric_model();

    // Explicitly rate Q1, where leverage = 150/100 = 1.5 → AA (90) and
    // coverage = 5.0 → AA (90).
    let mut config = leverage_coverage_config();
    config.period = Some("2025Q1".into());

    let mut extension = CreditScorecardExtension::with_config(config);
    let report = extension.execute(&model, &results).unwrap();

    assert_eq!(report.status, ScorecardStatus::Success);
    assert_eq!(
        report.data.get("period").and_then(|v| v.as_str()),
        Some("2025Q1")
    );
    assert_eq!(
        report.data.get("total_score").and_then(|v| v.as_f64()),
        Some(90.0)
    );
    assert_eq!(
        report.data.get("partial").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        report.data.get("weight_coverage").and_then(|v| v.as_f64()),
        Some(1.0)
    );
}

#[test]
fn test_scorecard_unknown_period_is_error() {
    let (model, results) = two_metric_model();

    let mut config = leverage_coverage_config();
    config.period = Some("2030Q4".into());

    let mut extension = CreditScorecardExtension::with_config(config);
    let err = extension
        .execute(&model, &results)
        .expect_err("period absent from the model must be an error");
    assert!(err.to_string().contains("not found in model periods"));
}
