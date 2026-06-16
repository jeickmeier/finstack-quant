#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
use finstack_quant_statements::checks::{CheckSuite, Severity};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

#[test]
fn evaluator_with_checks_attaches_report() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "total_assets",
            &[
                (q(1), AmountOrScalar::scalar(1000.0)),
                (q(2), AmountOrScalar::scalar(1100.0)),
            ],
        )
        .value(
            "total_liabilities",
            &[
                (q(1), AmountOrScalar::scalar(600.0)),
                (q(2), AmountOrScalar::scalar(700.0)),
            ],
        )
        .value(
            "total_equity",
            &[
                (q(1), AmountOrScalar::scalar(300.0)), // imbalance: 1000 - (600+300) = 100
                (q(2), AmountOrScalar::scalar(400.0)),
            ],
        )
        .build()
        .unwrap();

    let suite = CheckSuite::builder("inline")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("total_assets")],
            liabilities_nodes: vec![NodeId::new("total_liabilities")],
            equity_nodes: vec![NodeId::new("total_equity")],
            tolerance: None,
        })
        .build();

    let mut evaluator = Evaluator::new().with_checks(suite);
    let result = evaluator.evaluate(&model).unwrap();

    let report = result
        .check_report
        .as_ref()
        .expect("report should be present");
    assert!(report.has_errors());
    assert_eq!(report.summary.total_checks, 1);
    assert_eq!(report.summary.failed, 1);

    let error_findings = report.findings_by_severity(Severity::Error);
    assert!(!error_findings.is_empty());
    assert!(error_findings.iter().any(|f| f.period == Some(q(1))));
}

#[test]
fn evaluator_without_checks_has_no_report() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("revenue", &[(q(1), AmountOrScalar::scalar(100.0))])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let result = evaluator.evaluate(&model).unwrap();

    assert!(result.check_report.is_none());
}

#[test]
fn evaluator_with_checks_balanced_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "total_assets",
            &[
                (q(1), AmountOrScalar::scalar(1000.0)),
                (q(2), AmountOrScalar::scalar(1100.0)),
            ],
        )
        .value(
            "total_liabilities",
            &[
                (q(1), AmountOrScalar::scalar(600.0)),
                (q(2), AmountOrScalar::scalar(700.0)),
            ],
        )
        .value(
            "total_equity",
            &[
                (q(1), AmountOrScalar::scalar(400.0)),
                (q(2), AmountOrScalar::scalar(400.0)),
            ],
        )
        .build()
        .unwrap();

    let suite = CheckSuite::builder("inline")
        .add_check(BalanceSheetArticulation {
            assets_nodes: vec![NodeId::new("total_assets")],
            liabilities_nodes: vec![NodeId::new("total_liabilities")],
            equity_nodes: vec![NodeId::new("total_equity")],
            tolerance: None,
        })
        .build();

    let mut evaluator = Evaluator::new().with_checks(suite);
    let result = evaluator.evaluate(&model).unwrap();

    let report = result
        .check_report
        .as_ref()
        .expect("report should be present");
    assert!(!report.has_errors());
    assert_eq!(report.summary.passed, 1);
    assert_eq!(report.summary.failed, 0);
}
