//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::builtins::BalanceSheetArticulation;
use finstack_quant_statements::checks::{Check, CheckContext};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid period fixture")
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

// Fail-open guards: the identity must never pass because its operands are
// absent (missing nodes sum to zero on both sides → imbalance 0 → "pass").

#[test]
fn empty_node_group_is_rejected() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("total_assets", &[(q(1), AmountOrScalar::scalar(1000.0))])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![],
        equity_nodes: vec![],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let err = check
        .execute(&ctx)
        .expect_err("an empty node group makes the identity vacuous and must be rejected");
    assert!(
        err.to_string().contains("node group"),
        "error should name the empty node group, got: {err}"
    );
}

#[test]
fn misnamed_nodes_warn_instead_of_silently_passing() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("total_assets", &[(q(1), AmountOrScalar::scalar(1000.0))])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Every referenced node name is misspelled: the identity cannot be
    // evaluated, so the check must surface that — not report a clean pass.
    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assetz")],
        liabilities_nodes: vec![NodeId::new("total_liabilitiez")],
        equity_nodes: vec![NodeId::new("total_equityz")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(
        !result.findings.is_empty(),
        "missing operands must produce a finding, not a silent pass"
    );
    assert!(
        result.findings.iter().any(|f| f.severity
            == finstack_quant_statements::checks::Severity::Warning
            && f.message.contains("total_assetz")),
        "the warning should name the missing node, got: {:?}",
        result.findings
    );
}

#[test]
fn partially_missing_group_skips_period_with_warning() {
    // Assets are split across two subtotals but one is misspelled: summing the
    // resolvable one alone would fabricate an imbalance (or mask one), so the
    // period must be skipped with a warning instead of evaluated one-sided.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("current_assets", &[(q(1), AmountOrScalar::scalar(400.0))])
        .value(
            "total_liabilities",
            &[(q(1), AmountOrScalar::scalar(600.0))],
        )
        .value("total_equity", &[(q(1), AmountOrScalar::scalar(400.0))])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("current_assets"), NodeId::new("fixed_assetz")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    use finstack_quant_statements::checks::Severity;
    assert!(
        !result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error),
        "a period with unresolved operands must be skipped, not judged: {:?}",
        result.findings
    );
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Warning && f.message.contains("fixed_assetz")),
        "the warning should name the missing node, got: {:?}",
        result.findings
    );
}

#[test]
fn large_balance_sheet_within_relative_tolerance_passes() {
    // A $10B balance sheet with $0.05 of f64 accumulation drift must not fail
    // under the *default* config: the default tolerance is scale-aware
    // (relative component), not a flat absolute cent.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value(
            "total_assets",
            &[(q(1), AmountOrScalar::scalar(10_000_000_000.0))],
        )
        .value(
            "total_liabilities",
            &[(q(1), AmountOrScalar::scalar(6_000_000_000.0))],
        )
        .value(
            "total_equity",
            &[(q(1), AmountOrScalar::scalar(3_999_999_999.95))],
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

    assert!(
        result.passed,
        "a 5e-12 relative imbalance on a $10B sheet is f64 noise, not an accounting error: {:?}",
        result.findings
    );
}

#[test]
fn nan_operand_warns_instead_of_silently_passing() {
    // A NaN operand makes the imbalance NaN, and `NaN > tolerance` is false:
    // without a guard, a genuinely broken balance sheet silently passes. The
    // check must treat a non-finite operand like a missing one - warn and
    // skip the period.
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("total_assets", &[(q(1), AmountOrScalar::scalar(1000.0))])
        .value(
            "total_liabilities",
            &[(q(1), AmountOrScalar::scalar(600.0))],
        )
        .value("total_equity", &[(q(1), AmountOrScalar::scalar(400.0))])
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let mut results = evaluator.evaluate(&model).unwrap();
    results
        .nodes
        .entry("total_assets".to_string())
        .or_default()
        .insert(q(1), f64::NAN);

    let check = BalanceSheetArticulation {
        assets_nodes: vec![NodeId::new("total_assets")],
        liabilities_nodes: vec![NodeId::new("total_liabilities")],
        equity_nodes: vec![NodeId::new("total_equity")],
        tolerance: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(
        result.findings.iter().any(|f| f.severity
            == finstack_quant_statements::checks::Severity::Warning
            && f.message.contains("total_assets")),
        "a NaN operand must produce a warning naming the node, not a silent pass: {:?}",
        result.findings
    );
}
