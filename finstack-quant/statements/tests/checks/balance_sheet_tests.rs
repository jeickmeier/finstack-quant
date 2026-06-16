#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
use finstack_quant_statements::checks::{Check, CheckContext};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

#[test]
fn balanced_passes() {
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

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn imbalanced_fails_with_materiality() {
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
                (q(2), AmountOrScalar::scalar(400.0)), // balanced
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);

    let q1_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.period == Some(q(1)))
        .collect();
    assert_eq!(q1_findings.len(), 1);

    let mat = q1_findings[0].materiality.as_ref().unwrap();
    assert!((mat.absolute - 100.0).abs() < 0.01);
    assert!((mat.relative_pct - 10.0).abs() < 0.01); // 100/1000 * 100 = 10%
    assert!((mat.reference_value - 1000.0).abs() < 0.01);
    assert_eq!(mat.reference_label, "total_assets");
}

#[test]
fn within_tolerance_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "total_assets",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(100.0)),
            ],
        )
        .value(
            "total_liabilities",
            &[
                (q(1), AmountOrScalar::scalar(60.0)),
                (q(2), AmountOrScalar::scalar(60.0)),
            ],
        )
        .value(
            "total_equity",
            &[
                (q(1), AmountOrScalar::scalar(39.995)), // imbalance = 0.005
                (q(2), AmountOrScalar::scalar(39.995)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: Some(0.01), // 0.005 < 0.01 => passes
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}
