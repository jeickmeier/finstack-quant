//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::RetainedEarningsReconciliation;
use finstack_quant_statements::checks::{Check, CheckContext};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

#[test]
fn reconciliation_passes() {
    // RE(Q2) = RE(Q1) + NI(Q2) - Div(Q2) = 500 + 120 - 20 = 600 ✓
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "retained_earnings",
            &[
                (q(1), AmountOrScalar::scalar(500.0)),
                (q(2), AmountOrScalar::scalar(600.0)),
            ],
        )
        .value(
            "net_income",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(120.0)),
            ],
        )
        .value(
            "dividends",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(20.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = RetainedEarningsReconciliation {
        retained_earnings_node: NodeId::new("retained_earnings"),
        net_income_node: NodeId::new("net_income"),
        dividends_node: Some(NodeId::new("dividends")),
        other_adjustments: vec![],
        tolerance: None,
        dividends_sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn reconciliation_fails() {
    // Expected RE(Q2) = 500 + 120 - 20 = 600, actual = 650 → diff = 50
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "retained_earnings",
            &[
                (q(1), AmountOrScalar::scalar(500.0)),
                (q(2), AmountOrScalar::scalar(650.0)),
            ],
        )
        .value(
            "net_income",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(120.0)),
            ],
        )
        .value(
            "dividends",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(20.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = RetainedEarningsReconciliation {
        retained_earnings_node: NodeId::new("retained_earnings"),
        net_income_node: NodeId::new("net_income"),
        dividends_node: Some(NodeId::new("dividends")),
        other_adjustments: vec![],
        tolerance: None,
        dividends_sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].period, Some(q(2)));

    let mat = result.findings[0].materiality.as_ref().unwrap();
    assert!((mat.absolute - 50.0).abs() < 0.01);
}

#[test]
fn skips_first_period() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q3", None)
        .unwrap()
        .value(
            "retained_earnings",
            &[
                (q(1), AmountOrScalar::scalar(500.0)),
                (q(2), AmountOrScalar::scalar(600.0)),
                (q(3), AmountOrScalar::scalar(700.0)),
            ],
        )
        .value(
            "net_income",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(100.0)),
                (q(3), AmountOrScalar::scalar(100.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = RetainedEarningsReconciliation {
        retained_earnings_node: NodeId::new("retained_earnings"),
        net_income_node: NodeId::new("net_income"),
        dividends_node: None,
        other_adjustments: vec![],
        tolerance: None,
        dividends_sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    // No finding should reference Q1 (the first period is skipped)
    assert!(result.findings.iter().all(|f| f.period != Some(q(1))));
}

#[test]
fn with_other_adjustments() {
    // RE(Q2) = RE(Q1) + NI(Q2) - Div(Q2) + Adj(Q2) = 500 + 120 - 20 + 10 = 610 ✓
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "retained_earnings",
            &[
                (q(1), AmountOrScalar::scalar(500.0)),
                (q(2), AmountOrScalar::scalar(610.0)),
            ],
        )
        .value(
            "net_income",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(120.0)),
            ],
        )
        .value(
            "dividends",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(20.0)),
            ],
        )
        .value(
            "aoci_adjustment",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(10.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = RetainedEarningsReconciliation {
        retained_earnings_node: NodeId::new("retained_earnings"),
        net_income_node: NodeId::new("net_income"),
        dividends_node: Some(NodeId::new("dividends")),
        other_adjustments: vec![NodeId::new("aoci_adjustment")],
        tolerance: None,
        dividends_sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}
