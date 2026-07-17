//! Tests for the surrounding crate component and its documented behavior.
//!
#![allow(clippy::unwrap_used)]

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::{Check, CheckContext};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};
use finstack_quant_statements_analytics::analysis::checks::{
    CapexReconciliation, DepreciationReconciliation, DividendReconciliation,
    InterestExpenseReconciliation,
};

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter)
}

fn s(v: f64) -> AmountOrScalar {
    AmountOrScalar::scalar(v)
}

// ============================================================================
// DepreciationReconciliation
// ============================================================================

#[test]
fn depreciation_reconciles_passes() {
    // PPE(t) = PPE(t-1) + Capex - D&A - Disposals
    // Q1: 1000 (seed)
    // Q2: 1000 + 200 - 50 - 10 = 1140
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("ppe", &[(q(1), s(1000.0)), (q(2), s(1140.0))])
        .value("capex", &[(q(1), s(0.0)), (q(2), s(200.0))])
        .value("da", &[(q(1), s(0.0)), (q(2), s(50.0))])
        .value("disposals", &[(q(1), s(0.0)), (q(2), s(10.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = DepreciationReconciliation {
        depreciation_expense_node: NodeId::new("da"),
        ppe_node: NodeId::new("ppe"),
        capex_node: NodeId::new("capex"),
        disposals_node: Some(NodeId::new("disposals")),
        tolerance: None,
        sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn depreciation_mismatch_flags_warning() {
    // Q2 actual PPE = 1200, expected = 1000 + 200 - 50 = 1150 → diff = 50
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("ppe", &[(q(1), s(1000.0)), (q(2), s(1200.0))])
        .value("capex", &[(q(1), s(0.0)), (q(2), s(200.0))])
        .value("da", &[(q(1), s(0.0)), (q(2), s(50.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = DepreciationReconciliation {
        depreciation_expense_node: NodeId::new("da"),
        ppe_node: NodeId::new("ppe"),
        capex_node: NodeId::new("capex"),
        disposals_node: None,
        tolerance: None,
        sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert_eq!(result.findings.len(), 1);
    let f = &result.findings[0];
    assert_eq!(f.period, Some(q(2)));
    let mat = f.materiality.as_ref().unwrap();
    assert!((mat.absolute - 50.0).abs() < 0.01);
}

// ============================================================================
// InterestExpenseReconciliation
// ============================================================================

#[test]
fn interest_cs_matches_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("interest", &[(q(1), s(25.0)), (q(2), s(30.0))])
        .value("cs_interest", &[(q(1), s(25.0)), (q(2), s(30.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = InterestExpenseReconciliation {
        interest_expense_node: NodeId::new("interest"),
        debt_balance_nodes: vec![],
        cs_interest_node: Some(NodeId::new("cs_interest")),
        tolerance_pct: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.findings.is_empty());
}

#[test]
fn interest_cs_mismatch_flags_warning() {
    // Interest = 30, CS interest = 25 → 20% diff > 5% default tolerance
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("interest", &[(q(1), s(30.0))])
        .value("cs_interest", &[(q(1), s(25.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = InterestExpenseReconciliation {
        interest_expense_node: NodeId::new("interest"),
        debt_balance_nodes: vec![],
        cs_interest_node: Some(NodeId::new("cs_interest")),
        tolerance_pct: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].period, Some(q(1)));
}

#[test]
fn interest_implied_rate_reasonable_passes() {
    // Debt = 1000, rate = 0.05, implied interest = 50, IS interest = 50
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("interest", &[(q(1), s(50.0))])
        .value("debt", &[(q(1), s(1000.0))])
        .value("rate", &[(q(1), s(0.05))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = InterestExpenseReconciliation {
        interest_expense_node: NodeId::new("interest"),
        debt_balance_nodes: vec![(NodeId::new("debt"), Some(NodeId::new("rate")))],
        cs_interest_node: None,
        tolerance_pct: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.findings.is_empty());
}

/// Implied interest must use the AVERAGE balance
/// `(B_{t-1} + B_t) / 2`, not the end-of-period balance. This test
/// constructs a period where the debt balance grew from 800 to 1200;
/// average is 1000; at a 5 % rate implied interest = 50. The interest
/// expense reported as 50 must pass — it would fail under an EOP
/// convention (which would expect 60 = 1200 × 0.05, a 20 % miss).
#[test]
fn interest_implied_rate_uses_average_balance() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("interest", &[(q(1), s(40.0)), (q(2), s(50.0))])
        .value("debt", &[(q(1), s(800.0)), (q(2), s(1200.0))])
        .value("rate", &[(q(1), s(0.05)), (q(2), s(0.05))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = InterestExpenseReconciliation {
        interest_expense_node: NodeId::new("interest"),
        debt_balance_nodes: vec![(NodeId::new("debt"), Some(NodeId::new("rate")))],
        cs_interest_node: None,
        tolerance_pct: None,
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    // Q2: avg balance = (800 + 1200) / 2 = 1000; implied = 1000 × 0.05
    // = 50; actual = 50 → no finding. Under the old EOP convention
    // implied would have been 1200 × 0.05 = 60, a 20 % gap that would
    // have flagged.
    let q2_findings: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.period == Some(q(2)))
        .collect();
    assert!(
        q2_findings.is_empty(),
        "Q2 should pass with average balance convention but got findings: {q2_findings:?}"
    );
}

// ============================================================================
// CapexReconciliation
// ============================================================================

#[test]
fn capex_reconciles_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("capex_cf", &[(q(1), s(100.0)), (q(2), s(150.0))])
        .value("ppe_add", &[(q(1), s(80.0)), (q(2), s(120.0))])
        .value("intangible_add", &[(q(1), s(20.0)), (q(2), s(30.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = CapexReconciliation {
        capex_cf_node: NodeId::new("capex_cf"),
        ppe_additions_node: Some(NodeId::new("ppe_add")),
        intangible_additions_node: Some(NodeId::new("intangible_add")),
        tolerance: None,
        sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn capex_mismatch_flags_warning() {
    // CF capex = 100, but PPE add = 60, intangible add = 20 → expected = 80, diff = 20
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("capex_cf", &[(q(1), s(100.0))])
        .value("ppe_add", &[(q(1), s(60.0))])
        .value("intangible_add", &[(q(1), s(20.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = CapexReconciliation {
        capex_cf_node: NodeId::new("capex_cf"),
        ppe_additions_node: Some(NodeId::new("ppe_add")),
        intangible_additions_node: Some(NodeId::new("intangible_add")),
        tolerance: None,
        sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert_eq!(result.findings.len(), 1);
    let mat = result.findings[0].materiality.as_ref().unwrap();
    assert!((mat.absolute - 20.0).abs() < 0.01);
}

#[test]
fn capex_no_components_passes_trivially() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("capex_cf", &[(q(1), s(100.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = CapexReconciliation {
        capex_cf_node: NodeId::new("capex_cf"),
        ppe_additions_node: None,
        intangible_additions_node: None,
        tolerance: None,
        sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
}

// ============================================================================
// DividendReconciliation
// ============================================================================

#[test]
fn dividends_reconcile_passes() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value("div_cf", &[(q(1), s(10.0)), (q(2), s(12.0))])
        .value("div_eq", &[(q(1), s(10.0)), (q(2), s(12.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = DividendReconciliation {
        dividends_cf_node: NodeId::new("div_cf"),
        dividends_equity_node: NodeId::new("div_eq"),
        tolerance: None,
        sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert!(result.passed);
    assert!(result.findings.is_empty());
}

#[test]
fn dividends_mismatch_flags_warning() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..Q1", None)
        .unwrap()
        .value("div_cf", &[(q(1), s(15.0))])
        .value("div_eq", &[(q(1), s(10.0))])
        .build()
        .unwrap();

    let mut ev = Evaluator::new();
    let results = ev.evaluate(&model).unwrap();

    let check = DividendReconciliation {
        dividends_cf_node: NodeId::new("div_cf"),
        dividends_equity_node: NodeId::new("div_eq"),
        tolerance: None,
        sign_convention: Default::default(),
    };

    let ctx = CheckContext::new(&model, &results);
    let result = check.execute(&ctx).unwrap();

    assert_eq!(result.findings.len(), 1);
    let mat = result.findings[0].materiality.as_ref().unwrap();
    assert!((mat.absolute - 5.0).abs() < 0.01);
}
