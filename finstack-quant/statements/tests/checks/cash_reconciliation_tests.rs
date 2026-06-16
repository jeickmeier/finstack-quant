#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::CashReconciliation;
use finstack_quant_statements::checks::{Check, CheckContext};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

#[test]
fn cash_reconciliation_passes() {
    // Cash(Q2) = Cash(Q1) + TotalCF(Q2) = 100 + 50 = 150 ✓
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "cash",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(150.0)),
            ],
        )
        .value(
            "total_cf",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(50.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = CashReconciliation {
        cash_balance_node: NodeId::new("cash"),
        total_cash_flow_node: NodeId::new("total_cf"),
        cfo_node: None,
        cfi_node: None,
        cff_node: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn cash_reconciliation_fails() {
    // Expected Cash(Q2) = 100 + 50 = 150, actual = 160 → diff = 10
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "cash",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(160.0)),
            ],
        )
        .value(
            "total_cf",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(50.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = CashReconciliation {
        cash_balance_node: NodeId::new("cash"),
        total_cash_flow_node: NodeId::new("total_cf"),
        cfo_node: None,
        cfi_node: None,
        cff_node: None,
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    assert_eq!(result.findings.len(), 1);

    let mat = result.findings[0].materiality.as_ref().unwrap();
    assert!((mat.absolute - 10.0).abs() < 0.01);
}

#[test]
fn component_check_passes() {
    // Cash(Q2)=150, Cash(Q1)=100, TotalCF(Q2)=50,
    // CFO=80, CFI=-20, CFF=-10 → 80-20-10=50=TotalCF ✓
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "cash",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(150.0)),
            ],
        )
        .value(
            "total_cf",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(50.0)),
            ],
        )
        .value(
            "cfo",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(80.0)),
            ],
        )
        .value(
            "cfi",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(-20.0)),
            ],
        )
        .value(
            "cff",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(-10.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = CashReconciliation {
        cash_balance_node: NodeId::new("cash"),
        total_cash_flow_node: NodeId::new("total_cf"),
        cfo_node: Some(NodeId::new("cfo")),
        cfi_node: Some(NodeId::new("cfi")),
        cff_node: Some(NodeId::new("cff")),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn component_check_fails() {
    // TotalCF(Q2)=50, but CFO=80+CFI=-20+CFF=-5=55 → diff=5
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "cash",
            &[
                (q(1), AmountOrScalar::scalar(100.0)),
                (q(2), AmountOrScalar::scalar(150.0)),
            ],
        )
        .value(
            "total_cf",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(50.0)),
            ],
        )
        .value(
            "cfo",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(80.0)),
            ],
        )
        .value(
            "cfi",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(-20.0)),
            ],
        )
        .value(
            "cff",
            &[
                (q(1), AmountOrScalar::scalar(0.0)),
                (q(2), AmountOrScalar::scalar(-5.0)), // mismatch: 80-20-5=55≠50
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = CashReconciliation {
        cash_balance_node: NodeId::new("cash"),
        total_cash_flow_node: NodeId::new("total_cf"),
        cfo_node: Some(NodeId::new("cfo")),
        cfi_node: Some(NodeId::new("cfi")),
        cff_node: Some(NodeId::new("cff")),
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(!result.passed);
    // Should have the component mismatch finding
    let component_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.message.contains("components"))
        .collect();
    assert_eq!(component_findings.len(), 1);

    let mat = component_findings[0].materiality.as_ref().unwrap();
    assert!((mat.absolute - 5.0).abs() < 0.01);
}
